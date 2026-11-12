import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const entrypoint = fileURLToPath(new URL('../../../.github/actions/har-compare/run.mjs', import.meta.url));
const fixtures = fileURLToPath(new URL('../examples/fixtures/', import.meta.url));
const baseline = readFileSync(join(fixtures, 'baseline.har'), 'utf8');

function setup(t, { current = baseline, config, inputs = {}, name = '' } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'scope-action-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const workspace = join(root, 'consumer workspace');
  mkdirSync(workspace);
  writeFileSync(join(workspace, `baseline${name}.har`), baseline);
  writeFileSync(join(workspace, `current${name}.har`), current);
  if (config !== undefined) writeFileSync(join(workspace, `config${name}.json`), JSON.stringify(config));
  const outputFile = join(root, 'github-output');
  writeFileSync(outputFile, '');
  const env = { ...process.env, GITHUB_WORKSPACE: workspace, GITHUB_OUTPUT: outputFile,
    SCOPE_BASELINE: `baseline${name}.har`, SCOPE_CURRENT: `current${name}.har`,
    SCOPE_CONFIG: config === undefined ? '' : `config${name}.json`, SCOPE_OUTPUT: `reports${name}`,
    SCOPE_LABELS: 'ids', SCOPE_FAIL_ON_RESULT: 'true', ...inputs };
  const invoke = overrides => {
    writeFileSync(outputFile, '');
    const child = spawnSync(process.execPath, [entrypoint], {
      cwd: root, env: { ...env, ...overrides }, encoding: 'utf8', timeout: 10000,
    });
    assert.equal(child.error, undefined);
    const outputs = Object.fromEntries(readFileSync(outputFile, 'utf8').trim().split('\n').filter(Boolean).map(line => {
      const split = line.indexOf('=');
      return [line.slice(0, split), line.slice(split + 1)];
    }));
    return { ...child, outputs };
  };
  return { root, workspace, env, invoke };
}

function complete(result, code, outcome, regressions) {
  assert.equal(result.status, code, result.stderr);
  assert.equal(result.outputs['exit-code'], String(code));
  assert.equal(result.outputs.result, outcome);
  assert.equal(result.outputs.regressions, String(regressions));
  for (const key of ['json-report', 'html-report', 'junit-report']) assert.ok(existsSync(result.outputs[key]), key);
  const report = JSON.parse(readFileSync(result.outputs['json-report'], 'utf8'));
  assert.equal(report.summary.regressions, regressions);
  return report;
}

test('clean subprocess resolves inputs from workspace and bundles its own CLI', t => {
  const s = setup(t);
  mkdirSync(join(s.workspace, 'tools', 'har-compare'), { recursive: true });
  writeFileSync(join(s.workspace, 'tools', 'har-compare', 'cli.mjs'), 'throw new Error("CONSUMER_CLI_EXECUTED")');
  const r = s.invoke();
  const report = complete(r, 0, 'clean', 0);
  assert.equal(report.privacy.labels, 'ids');
  for (const key of ['json-report', 'html-report', 'junit-report']) {
    assert.doesNotMatch(readFileSync(r.outputs[key], 'utf8'), /api\.example\.invalid|\/users\/7|\/user\/role|Synthetic/);
  }
  assert.doesNotMatch(r.stdout + r.stderr, /CONSUMER_CLI_EXECUTED|schemaVersion|api\.example|\/users\/7/);
  assert.equal(r.outputs['error-code'], '');
});

test('missing JSON field returns regression status and generated evidence', t => {
  const s = setup(t, { current: readFileSync(join(fixtures, 'missing-field.har'), 'utf8') });
  const report = complete(s.invoke(), 1, 'regression', 1);
  assert.equal(report.findings[0].kind, 'field-removed');
  assert.deepEqual(report.findings[0].evidence, { baselineEntry: 1, candidateEntry: 1 });
});

test('authored required status fails even for identical HTTP 200 responses', t => {
  const s = setup(t, { config: { contracts: [{ method: 'GET', origin: 'https://api.example.invalid', path: '/users/7', allowedStatuses: [201] }] } });
  const report = complete(s.invoke(), 1, 'regression', 1);
  assert.equal(report.findings[0].kind, 'contract-status');
});

test('strict timestamp ignore has a real negative control', t => {
  const current = readFileSync(join(fixtures, 'timestamp.har'), 'utf8');
  const ignored = setup(t, { current, config: { mode: 'strict', ignoreJsonPaths: ['/timestamp'] } });
  const report = complete(ignored.invoke(), 0, 'clean', 0);
  assert.equal(report.summary.ignoredNodes, 1);
  const strict = setup(t, { current, config: { mode: 'strict' } });
  assert.ok(complete(strict.invoke(), 1, 'regression', 1).findings.some(f => f.kind === 'value-changed'));
});

test('malformed input returns invalid=2 with visible safe diagnostic and no report paths', t => {
  const s = setup(t, { current: '{PRIVATE_MALFORMED_BODY' });
  const r = s.invoke();
  assert.equal(r.status, 2);
  assert.equal(r.outputs['exit-code'], '2');
  assert.equal(r.outputs.result, 'invalid');
  assert.equal(r.outputs['error-code'], 'JSON');
  assert.match(r.stderr, /JSON.*malformed JSON/);
  assert.doesNotMatch(r.stdout + r.stderr, /PRIVATE_MALFORMED_BODY|consumer workspace/);
  for (const key of ['json-report', 'html-report', 'junit-report', 'regressions', 'informational']) assert.equal(r.outputs[key], '');
});

test('spaces, quotes, equals, dollar substitution, backticks and semicolons stay literal', t => {
  const name = ' space = ; $(touch INJECTED) `touch BACKTICK` \' " &';
  const s = setup(t, { name, config: { mode: 'shape' } });
  const r = s.invoke();
  complete(r, 0, 'clean', 0);
  assert.equal(r.outputs['json-report'], join(s.workspace, `reports${name}`, 'analysis.json'));
  for (const dir of [s.root, s.workspace]) for (const marker of ['INJECTED', 'BACKTICK']) assert.equal(existsSync(join(dir, marker)), false);
});

test('readable labels are an explicit opt-in, never dumped into logs', t => {
  const s = setup(t, { inputs: { SCOPE_LABELS: 'readable' } });
  const r = s.invoke();
  complete(r, 0, 'clean', 0);
  assert.match(readFileSync(r.outputs['json-report'], 'utf8'), /api\.example\.invalid/);
  assert.doesNotMatch(r.stdout + r.stderr, /api\.example\.invalid/);
});

for (const [current, code, outcome] of [['missing-field.har', 1, 'regression'], ['malformed', 2, 'invalid']]) {
  test(`deferred gating preserves ${outcome} classification without failing the subprocess`, t => {
    const s = setup(t, { current: current === 'malformed' ? '{' : readFileSync(join(fixtures, current), 'utf8'), inputs: { SCOPE_FAIL_ON_RESULT: 'false' } });
    const r = s.invoke();
    assert.equal(r.status, 0);
    assert.equal(r.outputs['exit-code'], String(code));
    assert.equal(r.outputs.result, outcome);
    assert.match(r.stderr, /Snurpo HAR comparison/);
  });
}

for (const [label, inputs, expected] of [
  ['missing baseline', { SCOPE_BASELINE: '' }, 'ACTION_INPUT'],
  ['missing current', { SCOPE_CURRENT: '' }, 'ACTION_INPUT'],
  ['missing output', { SCOPE_OUTPUT: '' }, 'ACTION_INPUT'],
  ['bad labels', { SCOPE_LABELS: 'unknown' }, 'ACTION_INPUT'],
  ['bad gating flag', { SCOPE_FAIL_ON_RESULT: 'yes' }, 'ACTION_INPUT'],
  ['newline injection', { SCOPE_OUTPUT: 'reports\nforged=value' }, 'ACTION_INPUT'],
  ['CR injection', { SCOPE_BASELINE: 'x\rforged=value' }, 'ACTION_INPUT'],
  ['missing file', { SCOPE_CURRENT: 'absent.har' }, 'INPUT'],
  ['missing config', { SCOPE_CONFIG: 'absent.json' }, 'INPUT'],
]) {
  test(`invalid ${label} is visible and fails closed`, t => {
    const r = setup(t, { inputs }).invoke();
    assert.equal(r.status, 2);
    assert.equal(r.outputs.result, 'invalid');
    assert.equal(r.outputs['exit-code'], '2');
    assert.equal(r.outputs['error-code'], expected);
    assert.equal(r.outputs.forged, undefined);
    assert.match(r.stderr, new RegExp(expected));
    assert.equal(r.outputs['json-report'], '');
  });
}

test('existing output directory is refused without overwriting or advertising stale reports', t => {
  const s = setup(t);
  complete(s.invoke(), 0, 'clean', 0);
  const path = join(s.workspace, 'reports', 'analysis.json');
  const before = readFileSync(path, 'utf8');
  const r = s.invoke();
  assert.equal(r.status, 2);
  assert.equal(r.outputs['error-code'], 'ACTION_OUTPUT');
  assert.equal(r.outputs['json-report'], '');
  assert.equal(readFileSync(path, 'utf8'), before);
});

test('missing runner output file fails visibly instead of reporting success', t => {
  const r = setup(t).invoke({ GITHUB_OUTPUT: '' });
  assert.equal(r.status, 2);
  assert.match(r.stderr, /ACTION_OUTPUT/);
});
