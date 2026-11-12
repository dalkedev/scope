import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const cli = fileURLToPath(new URL('../cli.mjs', import.meta.url));
export function entry(body = { user: { name: 'Synthetic Alice', role: 'reader' } }, options = {}) {
  return {
    request: { method: 'GET', url: 'https://api.example.invalid/users/7', headers: [] },
    response: { status: 200, content: { mimeType: 'application/json', text: JSON.stringify(body) } },
    ...options,
  };
}
function runCase(t, baseline, candidate, config, extra = []) {
  const dir = mkdtempSync(join(tmpdir(), 'scope-har-test-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const a = join(dir, 'baseline.har'), b = join(dir, 'candidate.har');
  for (const [path, entries] of [[a, baseline], [b, candidate]]) {
    writeFileSync(path, JSON.stringify({ log: { version: '1.2', entries } }));
  }
  const args = ['compare', '--baseline', a, '--candidate', b];
  if (config !== undefined) {
    writeFileSync(join(dir, 'config.json'), JSON.stringify(config));
    args.push('--config', join(dir, 'config.json'));
  }
  const invoke = (tail = []) => spawnSync(process.execPath, [cli, ...args, ...tail], { encoding: 'utf8', cwd: '/tmp', timeout: 5000, maxBuffer: 64 * 1024 * 1024 });
  const result = invoke(extra.map(v => v.replaceAll('$DIR', dir)));
  return { ...result, data: result.stdout ? JSON.parse(result.stdout) : null, dir, a, b, invoke };
}
function kinds(r) { return r.data.findings.map(f => f.kind); }

test('missing nested observed field identifies both HAR entries and path', t => {
  const r = runCase(t, [entry()], [entry({ user: { name: 'Synthetic Alice' } })]);
  assert.equal(r.status, 1);
  assert.equal(r.data?.findings[0].kind, 'field-removed');
  assert.equal(r.data.findings[0].path, '/user/role');
  assert.deepEqual(r.data.findings[0].evidence, { baselineEntry: 1, candidateEntry: 1 });
});
test('200 to 500 is a regression even when JSON subtree is ignored', t => {
  const r = runCase(t, [entry()], [entry({}, { response: { status: 500, content: { mimeType: 'application/json', text: '{}' } } })], { ignoreJsonPaths: ['/user'] });
  assert.equal(r.status, 1); assert.ok(kinds(r).includes('http-failure'));
  assert.deepEqual(r.data.findings.find(f => f.kind === 'http-failure').status, { before: 200, after: 500 });
});
test('timestamp ignore has a strict value-aware negative control', t => {
  const a = [entry({ timestamp: '2026-11-01T00:00:00Z', ok: true })];
  const b = [entry({ timestamp: '2026-11-02T00:00:00Z', ok: true })];
  const ignored = runCase(t, a, b, { mode: 'strict', ignoreJsonPaths: ['/timestamp'] });
  assert.equal(ignored.status, 0); assert.equal(ignored.data.summary.ignoredNodes, 1);
  const strict = runCase(t, a, b, { mode: 'strict' });
  assert.equal(strict.status, 1); assert.deepEqual(kinds(strict), ['value-changed']);
});
test('per-occurrence comparison cannot hide a field behind a union', t => {
  const r = runCase(t, [entry(), entry()], [entry(), entry({ user: {} })]);
  assert.equal(r.status, 1);
  assert.equal(r.data.findings[0].evidence.candidateEntry, 2);
  assert.ok(r.data.notices.some(n => n.kind === 'ordinal-pairing'));
});
test('query values remain distinct matching keys and never appear in output', t => {
  const e = q => entry({}, { request: { method: 'GET', url: `https://api.example.invalid/x?token=${q}` } });
  const r = runCase(t, [e('SYNTHETIC_SECRET_A')], [e('SYNTHETIC_SECRET_B')]);
  assert.equal(r.status, 1); assert.deepEqual(kinds(r).sort(), ['operation-added', 'operation-removed']);
  assert.doesNotMatch(r.stdout, /SYNTHETIC_SECRET|token/);
});
test('scheme and non-default port stay separate', t => {
  const e = url => entry({}, { request: { method: 'GET', url } });
  const r = runCase(t, [e('http://api.example.invalid:8080/x')], [e('https://api.example.invalid:8081/x')]);
  assert.equal(r.status, 1); assert.deepEqual(kinds(r).sort(), ['operation-added', 'operation-removed']);
});
test('query order canonicalizes without collapsing duplicate parameter order', t => {
  const e = suffix => entry({}, { request: { method: 'GET', url: 'https://api.example.invalid/x?' + suffix } });
  assert.equal(runCase(t, [e('a=1&b=2')], [e('b=2&a=1')]).status, 0);
  assert.equal(runCase(t, [e('a=1&a=2')], [e('a=2&a=1')]).status, 1);
});
test('shape mode ignores scalars but strict mode catches type and array changes', t => {
  assert.equal(runCase(t, [entry({ x: 1 })], [entry({ x: 2 })]).status, 0);
  assert.deepEqual(kinds(runCase(t, [entry({ x: 1 })], [entry({ x: '1' })])), ['type-changed']);
  assert.ok(kinds(runCase(t, [entry([1, 2])], [entry([1])])).includes('array-length-changed'));
  assert.equal(runCase(t, [entry(null)], [entry({})]).status, 1);
});
test('new fields are informational; removed calls fail; new calls do not fail', t => {
  assert.equal(runCase(t, [entry({})], [entry({ extra: true })]).status, 0);
  assert.equal(runCase(t, [entry(), entry()], [entry()]).status, 1);
  assert.equal(runCase(t, [], [entry()]).status, 0);
});
test('base64 UTF-8 JSON and allowlisted Snurpo provenance are preserved', t => {
  const e = entry({});
  e.response.content.text = Buffer.from('{"word":"zażółć"}').toString('base64');
  e.response.content.encoding = 'base64';
  e._scope = { completed: true, provenance: 'mock', edited: true, error: null, originalResponse: 'PRIVATE_ORIGINAL', appliedRules: ['PRIVATE_RULE'] };
  const r = runCase(t, [e], [e]);
  assert.equal(r.status, 0); assert.equal(r.data.operations[0].baseline[0].provenance, 'mock');
  assert.doesNotMatch(r.stdout, /PRIVATE_|zażółć/);
});
for (const [name, mutate] of [
  ['missing JSON body', e => delete e.response.content.text],
  ['malformed JSON', e => e.response.content.text = '{SYNTHETIC_SECRET'],
  ['malformed base64', e => { e.response.content.encoding = 'base64'; e.response.content.text = '???'; }],
  ['unknown encoding', e => e.response.content.encoding = 'gzip'],
  ['invalid UTF-8 base64', e => { e.response.content.encoding = 'base64'; e.response.content.text = '/w=='; }],
  ['unsupported non-JSON response', e => { e.response.content.mimeType = 'text/html'; e.response.content.text = '<h1>hi</h1>'; }],
  ['incomplete Snurpo entry', e => e._scope = { completed: false }],
  ['Snurpo capture error', e => e._scope = { error: 'SYNTHETIC_SECRET' }],
  ['malformed Snurpo flags', e => e._scope = { completed: 'false' }],
  ['invalid status', e => e.response.status = 0],
  ['invalid method', e => e.request.method = 'GET\nSECRET'],
  ['non-HTTP URL', e => e.request.url = 'file:///etc/passwd'],
  ['oversize body', e => e.response.content.text = 'x'.repeat(1024 * 1024 + 1)],
  ['excessive body depth', e => e.response.content.text = '['.repeat(66) + '0' + ']'.repeat(66)],
]) test(name + ' fails closed without raw input in diagnostics', t => {
  const e = entry(); mutate(e); const r = runCase(t, [entry()], [e]);
  assert.equal(r.status, 2); assert.ok(r.data.error.code); assert.doesNotMatch(r.stdout + r.stderr, /SYNTHETIC_SECRET|PRIVATE_/);
});
for (const config of [{ typo: true }, { mode: 'value' }, { ignoreJsonPaths: ['timestamp'] }, { ignoreJsonPaths: ['/x~2'] }, { ignoreJsonPaths: ['/never-present'] }, { ignoreJsonPaths: [''] }, { ignoreJsonPaths: ['/x', '/x'] }, { ignoreJsonPaths: ['/user', '/user/name'] }]) {
  test('rejects mistaken ignore/config: ' + JSON.stringify(config), t => {
    assert.equal(runCase(t, [entry()], [entry()], config).status, 2);
  });
}
test('JSON pointers escape slash and tilde and cannot alter statuses', t => {
  const r = runCase(t, [entry({ 'a/b~c': 1 })], [entry({ 'a/b~c': 2 })], { mode: 'strict', ignoreJsonPaths: ['/a~1b~0c'] });
  assert.equal(r.status, 0);
});
test('declared empty responses are accepted without pretending JSON was parsed', t => {
  const e = entry({}, { response: { status: 204, content: { size: 0, mimeType: '' } } });
  assert.equal(runCase(t, [e], [e]).status, 0);
});
test('HTML and JSON omit secrets and HTML escapes hostile field/path data', t => {
  const payload = '</script><img src="https://leak.example.invalid/" onerror="globalThis.pwned=1">';
  const a = entry({ [payload]: 'BODY_SECRET_A' });
  a.request.url = 'https://USER_SECRET:PASS_SECRET@api.example.invalid/x?key=QUERY_SECRET#FRAGMENT_SECRET';
  a.request.headers = [{ name: 'Authorization', value: 'HEADER_SECRET' }];
  a.response.cookies = [{ name: 'session', value: 'COOKIE_SECRET' }];
  const b = structuredClone(a); b.response.content.text = '{}';
  const r = runCase(t, [a], [b], undefined, ['--html', '$DIR/report.html', '--json', '$DIR/report.json']);
  assert.equal(r.status, 1);
  const html = readFileSync(join(r.dir, 'report.html'), 'utf8');
  assert.doesNotMatch(html, /<script|<img|onerror="|\b(?:href|src)="https?:/i);
  assert.match(html, /&lt;~1script&gt;/);
  for (const secret of ['BODY_SECRET_A', 'USER_SECRET', 'PASS_SECRET', 'QUERY_SECRET', 'FRAGMENT_SECRET', 'HEADER_SECRET', 'COOKIE_SECRET']) assert.ok(!(html + r.stdout).includes(secret));
  assert.equal(readFileSync(join(r.dir, 'report.json'), 'utf8'), r.stdout);
  const second = r.invoke(['--html', join(r.dir, 'second.html')]);
  assert.equal(second.status, 1); assert.equal(second.stdout, r.stdout);
  assert.equal(readFileSync(join(r.dir, 'second.html'), 'utf8'), html);
});
test('CLI refuses output collisions and preserves existing files and inputs', t => {
  const r = runCase(t, [entry()], [entry()]);
  const result = r.invoke(['--html', r.a]);
  assert.equal(result.status, 2); assert.ok(JSON.parse(readFileSync(r.a)).log);
  const same = r.invoke(['--html', join(r.dir, 'same'), '--json', join(r.dir, 'same')]);
  assert.equal(same.status, 2);
});
test('malformed HAR/config and too many entries fail with bounded diagnostics', t => {
  const r = runCase(t, [entry()], [entry()]);
  writeFileSync(r.a, '{"log":'); assert.equal(r.invoke().status, 2);
  writeFileSync(r.a, JSON.stringify({ log: { entries: new Array(2001).fill(entry({})) } }));
  assert.equal(r.invoke().status, 2);
  writeFileSync(r.a, 'x'.repeat(10 * 1024 * 1024 + 1)); assert.equal(r.invoke().status, 2);
});
test('CLI help and offline doctor work outside the repository; invalid flags exit 2', () => {
  const call = args => spawnSync(process.execPath, [cli, ...args], { encoding: 'utf8', cwd: '/tmp', timeout: 5000, maxBuffer: 64 * 1024 * 1024 });
  assert.match(call(['--help']).stdout, /--baseline/);
  const doctor = call(['doctor']); assert.equal(doctor.status, 0); assert.equal(JSON.parse(doctor.stdout).offline, true);
  assert.equal(call(['compare', '--wat']).status, 2);
  assert.equal(call([]).status, 2);
});

test('null config fields are not silently replaced by defaults', t => {
  for (const config of [{ mode: null }, { ignoreJsonPaths: null }]) assert.equal(runCase(t, [entry()], [entry()], config).status, 2);
});
test('number precision, total node count and finding budgets fail closed', t => {
  const unsafe = entry(); unsafe.response.content.text = '{"n":9007199254740993}';
  assert.equal(runCase(t, [entry()], [unsafe]).status, 2);
  assert.equal(runCase(t, [entry(new Array(100001).fill(0))], [entry({})]).status, 2);
  const data = Object.fromEntries(Array.from({ length: 10001 }, (_, i) => ['field' + i, 1]));
  assert.equal(runCase(t, [entry(data)], [entry({})]).status, 2);
  let a = Object.fromEntries(Array.from({ length: 1500 }, (_, i) => ['field' + i, 1]));
  let b = {};
  for (let i = 0; i < 10; i++) { a = { ['k'.repeat(1000)]: a }; b = { ['k'.repeat(1000)]: b }; }
  assert.equal(runCase(t, [entry(a)], [entry(b)]).status, 2);
});

test('FIFO inputs do not block waiting for a writer', t => {
  const r = runCase(t, [entry()], [entry()]);
  const fifo = join(r.dir, 'pipe.har');
  assert.equal(spawnSync('mkfifo', [fifo]).status, 0);
  const result = spawnSync(process.execPath, [cli, 'compare', '--baseline', fifo, '--candidate', r.b], { encoding: 'utf8', timeout: 1000 });
  assert.equal(result.status, 2); assert.equal(result.error, undefined);
});

test('review: interim or upgrade statuses cannot substitute for final responses', t => {
  const e = status => entry({}, { request: { method: 'HEAD', url: 'https://api.example.invalid/x' }, response: { status, content: { mimeType: 'application/json', size: 0 } } });
  for (const status of [100, 101, 103, 199]) {
    const r = runCase(t, [e(200)], [e(status)]);
    assert.equal(r.status, 2); assert.equal(r.data.error.code, 'INCOMPLETE');
  }
});
test('review: known content size must agree with decoded captured bytes', t => {
  for (const encoding of [undefined, 'base64']) {
    const candidate = entry({}, { response: { status: 200, content: { mimeType: 'application/json', text: encoding ? 'e30=' : '{}', encoding, size: 200 } } });
    const r = runCase(t, [entry({})], [candidate]);
    assert.equal(r.status, 2); assert.equal(r.data.error.code, 'INCOMPLETE');
  }
});
test('review: valid UTF-8 sizes and omitted/unknown sizes remain supported', t => {
  const content = { mimeType: 'application/json', text: '{"word":"ż"}', size: 13 };
  const e = entry({}, { response: { status: 200, content } });
  assert.equal(runCase(t, [e], [e]).status, 0);
  content.text = Buffer.from(content.text).toString('base64'); content.encoding = 'base64';
  assert.equal(runCase(t, [e], [e]).status, 0);
  content.size = -1; assert.equal(runCase(t, [e], [e]).status, 0);
  delete content.size; assert.equal(runCase(t, [e], [e]).status, 0);
  content.size = -2; assert.equal(runCase(t, [e], [e]).status, 2);
});
test('review: HEAD representation size does not imply a missing response body', t => {
  const e = entry({}, { request: { method: 'HEAD', url: 'https://api.example.invalid/x' }, response: { status: 200, content: { size: 200, mimeType: 'application/json' } } });
  assert.equal(runCase(t, [e], [e]).status, 0);
});

const selectedApi = { method: 'GET', origin: 'https://api.example.invalid', path: '/users/7' };
const selectedConfig = { includeOperations: [selectedApi] };
const asset = () => entry({}, { request: { method: 'GET', url: 'https://static.example.invalid/private-ASSET_SECRET.png?token=QUERY_SECRET' }, response: { status: 200, content: { mimeType: 'image/png' } } });

test('selection: mixed HAR excludes assets and retains original entry evidence', t => {
  const r = runCase(t, [asset(), entry()], [asset(), entry({ user: { name: 'Synthetic Alice' } })], selectedConfig);
  assert.equal(r.status, 1); assert.equal(r.data.findings[0].path, '/user/role');
  assert.deepEqual(r.data.findings[0].evidence, { baselineEntry: 2, candidateEntry: 2 });
  assert.deepEqual(r.data.coverage, {
    baseline: { totalEntries: 2, selectedEntries: 1, excludedEntries: 1 },
    candidate: { totalEntries: 2, selectedEntries: 1, excludedEntries: 1 },
  });
  assert.doesNotMatch(r.stdout, /ASSET_SECRET|QUERY_SECRET|static\.example/);
});
test('selection: unchanged chosen API produces an explicit scoped clean report', t => {
  const r = runCase(t, [asset(), entry()], [asset(), asset(), entry()], selectedConfig, ['--html', '$DIR/report.html']);
  assert.equal(r.status, 0); assert.equal(r.data.coverage.candidate.excludedEntries, 2);
  const html = readFileSync(join(r.dir, 'report.html'), 'utf8');
  assert.match(html, /Selection scope/); assert.match(html, /Excluded responses were not analyzed/);
  assert.doesNotMatch(html, /ASSET_SECRET|QUERY_SECRET|static\.example/);
  assert.equal(r.invoke().stdout, r.stdout);
});
test('selection: missing or HTML response on a selected endpoint fails closed', t => {
  const missing = entry(); delete missing.response.content.text;
  const html = entry({}, { response: { status: 500, content: { mimeType: 'text/html', text: '<h1>SERVER_SECRET</h1>' } } });
  for (const item of [missing, html]) {
    const r = runCase(t, [asset(), entry()], [asset(), item], selectedConfig);
    assert.equal(r.status, 2); assert.equal(r.data.error.code, 'BODY');
    assert.doesNotMatch(r.stdout, /SERVER_SECRET/);
  }
});
test('selection: JSON ignore never hides selected HTTP failure', t => {
  const item = entry({}, { response: { status: 500, content: { mimeType: 'application/json', text: '{}' } } });
  const r = runCase(t, [asset(), entry()], [asset(), item], { ...selectedConfig, ignoreJsonPaths: ['/user'] });
  assert.equal(r.status, 1); assert.ok(kinds(r).includes('http-failure'));
});
test('selection: missing candidate remains an observed removal, not empty success', t => {
  const r = runCase(t, [asset(), entry()], [asset()], selectedConfig);
  assert.equal(r.status, 1); assert.equal(r.data.findings[0].kind, 'operation-removed');
  assert.equal(r.data.coverage.candidate.selectedEntries, 0);
});
test('selection: empty baseline or unmatched selector fails with no coverage claim', t => {
  const noBaseline = runCase(t, [asset()], [asset(), entry()], selectedConfig);
  assert.equal(noBaseline.status, 2); assert.equal(noBaseline.data.error.code, 'SELECTION_EMPTY');
  const typo = runCase(t, [entry()], [entry()], { includeOperations: [{ ...selectedApi, path: '/typo' }] });
  assert.equal(typo.status, 2); assert.equal(typo.data.error.code, 'SELECTION_UNUSED');
  assert.equal(runCase(t, [entry()], [entry()], { includeOperations: [selectedApi, { ...selectedApi, path: '/typo' }] }).status, 2);
});
test('selection: all query variants and repeated occurrences participate', t => {
  const a = entry(), b = entry();
  a.request.url += '?token=VARIANT_A'; b.request.url += '?token=VARIANT_B';
  const changed = structuredClone(b); changed.response.content.text = '{}';
  const r = runCase(t, [asset(), a, b, a], [asset(), a, changed, a], selectedConfig);
  assert.equal(r.status, 1); assert.equal(r.data.summary.pairedEntries, 3); assert.equal(r.data.summary.operations, 2);
  assert.equal(r.data.findings[0].evidence.candidateEntry, 3); assert.doesNotMatch(r.stdout, /VARIANT_/);
});
test('selection: strict timestamp ignore retains its negative control', t => {
  const a = [asset(), entry({ timestamp: 1 })], b = [asset(), entry({ timestamp: 2 })];
  assert.equal(runCase(t, a, b, { ...selectedConfig, mode: 'strict', ignoreJsonPaths: ['/timestamp'] }).status, 0);
  assert.equal(runCase(t, a, b, { ...selectedConfig, mode: 'strict' }).status, 1);
});
test('selection: method normalization and canonical selector ordering are deterministic', t => {
  const post = entry({}, { request: { method: 'POST', url: 'https://api.example.invalid/users/7' } });
  const postSelector = { ...selectedApi, method: 'post' };
  const a = runCase(t, [entry(), post], [entry(), post], { includeOperations: [postSelector, selectedApi] });
  const b = runCase(t, [entry(), post], [entry(), post], { includeOperations: [selectedApi, { ...postSelector, method: 'POST' }] });
  assert.equal(a.status, 0); assert.equal(a.stdout, b.stdout);
});
for (const list of [null, [], [selectedApi, selectedApi], [{ ...selectedApi, method: '*' }], [{ ...selectedApi, method: 'GET\n' }], [{ ...selectedApi, origin: 'https://api.example.invalid/' }], [{ ...selectedApi, origin: 'https://user:password@api.example.invalid' }], [{ ...selectedApi, origin: 'file:///etc' }], [{ ...selectedApi, origin: 'https://api.example.invalid:443' }], [{ ...selectedApi, path: '/users/../users/7' }], [{ ...selectedApi, path: '/users/7?q=secret' }], [{ ...selectedApi, path: '/users/7#x' }], [{ ...selectedApi, path: '*' }], [{ ...selectedApi, typo: true }], Array.from({ length: 101 }, (_, i) => ({ ...selectedApi, path: '/x/' + i }))]) {
  test('selection: invalid or misleading selectors are rejected: ' + JSON.stringify(list).slice(0, 140), t => {
    assert.equal(runCase(t, [entry()], [entry()], { includeOperations: list }).status, 2);
  });
}
test('selection: malformed request identity is not hidden by exclusion', t => {
  const bad = asset(); bad.request.url = 'not a URL';
  const r = runCase(t, [entry(), bad], [entry()], selectedConfig);
  assert.equal(r.status, 2); assert.equal(r.data.error.code, 'HAR');
});

test('selection: literal double-slash path stays on its configured origin', t => {
  const e = entry({}, { request: { method: 'GET', url: 'https://api.example.invalid//users/7' } });
  const r = runCase(t, [e], [e], { includeOperations: [{ ...selectedApi, path: '//users/7' }] });
  assert.equal(r.status, 0); assert.equal(r.data.operations[0].path, '//users/7');
});
test('HAR envelope nesting is bounded even in excluded response metadata', t => {
  const r = runCase(t, [entry()], [entry()], selectedConfig);
  const raw = JSON.parse(readFileSync(r.a)); let extra = 0;
  for (let i = 0; i < 130; i++) extra = [extra];
  raw.log.extra = extra;
  writeFileSync(r.a, JSON.stringify(raw));
  const result = r.invoke(); assert.equal(result.status, 2); assert.equal(JSON.parse(result.stdout).error.code, 'LIMIT');
});

test('envelope guard ignores bracket and escaped-quote text inside strings', t => {
  const e = entry({ text: '['.repeat(200) + '\\"' + ']'.repeat(200) });
  const r = runCase(t, [e], [e]); assert.equal(r.status, 0);
});
test('selection: excluded response availability is explicitly outside coverage', t => {
  const e = asset(); delete e.response;
  const r = runCase(t, [e, entry()], [entry()], selectedConfig);
  assert.equal(r.status, 0); assert.equal(r.data.coverage.baseline.excludedEntries, 1);
});

test('selection: unused JSON ignore remains an error when all selected candidates disappear', t => {
  const r = runCase(t, [asset(), entry({ timestamp: 1 })], [asset()], { ...selectedConfig, ignoreJsonPaths: ['/timestamp'] });
  assert.equal(r.status, 2); assert.equal(r.data.error.code, 'IGNORE_UNUSED');
});
