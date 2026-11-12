#!/usr/bin/env node
// Public static fixtures; every expectation below is authored independently of the comparator.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { accessSync, constants, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const prototype = resolve(here, '../..');
const cli = join(prototype, 'cli.mjs');
const refresh = process.argv.slice(2).includes('--refresh');
assert.ok(process.argv.slice(2).every(argument => argument === '--refresh'), 'Only --refresh is supported.');
const [nodeMajor, nodeMinor, nodePatch] = process.versions.node.split('.').map(Number);
assert.ok(nodeMajor > 26 || (nodeMajor === 26 && (nodeMinor > 13 || (nodeMinor === 13 && nodePatch >= 1))), 'Use Node 26.13.1 or later.');
const fixtures = join(here, 'fixtures');
const configs = join(here, 'configs');
const goldens = join(here, 'goldens');
mkdirSync(join(prototype, 'out'), { recursive: true });
const out = mkdtempSync(join(prototype, 'out', 'sprint-'));
const origin = 'https://synthetic.invalid';
const forbidden = ['SPRINT_SYNTHETIC_QUERY_VALUE', 'SPRINT_SYNTHETIC_HEADER_VALUE', 'SPRINT_SYNTHETIC_BODY_VALUE'];
const absent = path => { try { accessSync(path, constants.F_OK); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } };

// Literal semantic expectations, not values obtained from a prior comparison.
const cases = [
  { name: 'inventory', inspect: 'mixed-baseline.har', exit: 0 },
  { name: 'unchanged-500-observed', baseline: 'unchanged-500.har', candidate: 'unchanged-500.har', exit: 0, regressions: 0, pairs: 1, findings: [] },
  { name: 'unchanged-500-asserted', baseline: 'unchanged-500.har', candidate: 'unchanged-500.har', config: 'status-200.json', exit: 1, regressions: 1, pairs: 1, contractChecks: 1,
    findings: [{ kind: 'contract-status', operationPath: '/users', entry: 1, status: 500 }] },
  { name: 'unchanged-500-explicitly-allowed', baseline: 'unchanged-500.har', candidate: 'unchanged-500.har', config: 'status-500.json', exit: 0, regressions: 0, pairs: 1, contractChecks: 1, findings: [] },
  { name: 'missing-from-both-observed', baseline: 'missing-from-both.har', candidate: 'missing-from-both.har', exit: 0, regressions: 0, pairs: 1, findings: [] },
  { name: 'missing-from-both-required', baseline: 'missing-from-both.har', candidate: 'missing-from-both.har', config: 'required-role.json', exit: 1, regressions: 1, pairs: 1, contractChecks: 1,
    findings: [{ kind: 'contract-json-missing', operationPath: '/users', entry: 1, path: '/user/role' }] },
  { name: 'missing-from-both-required-ids', baseline: 'missing-from-both.har', candidate: 'missing-from-both.har', config: 'required-role.json', labels: 'ids', exit: 1, regressions: 1, pairs: 1, contractChecks: 1,
    findings: [{ kind: 'contract-json-missing', operationPath: '/users', entry: 1, path: '/user/role' }] },
  { name: 'ignored-parent-still-required', baseline: 'missing-from-both.har', candidate: 'missing-from-both.har', config: 'ignored-parent-required-role.json', exit: 1, regressions: 1, pairs: 1, ignored: 1, contractChecks: 1,
    findings: [{ kind: 'contract-json-missing', operationPath: '/users', entry: 1, path: '/user/role' }] },
  { name: 'timestamps-strict-control', baseline: 'mixed-baseline.har', candidate: 'mixed-timestamps.har', config: 'strict.json', exit: 1, regressions: 2, pairs: 2, selection: true,
    findings: [{ kind: 'value-changed', operationPath: '/billing', entry: 3, path: '/timestamp' }, { kind: 'value-changed', operationPath: '/health', entry: 2, path: '/timestamp' }] },
  { name: 'timestamps-health-only', baseline: 'mixed-baseline.har', candidate: 'mixed-timestamps.har', config: 'scoped-health.json', exit: 1, regressions: 1, pairs: 2, ignored: 1, scopedIgnored: 1, selection: true,
    findings: [{ kind: 'value-changed', operationPath: '/billing', entry: 3, path: '/timestamp' }] },
  { name: 'timestamps-both-ignored', baseline: 'mixed-baseline.har', candidate: 'mixed-timestamps.har', config: 'scoped-both.json', exit: 0, regressions: 0, pairs: 2, ignored: 2, scopedIgnored: 2, selection: true, findings: [] },
  { name: 'duplicate-json-rejected', baseline: 'missing-from-both.har', candidate: 'duplicate-json.har', exit: 2, error: 'JSON' },
  { name: 'lossy-json-rejected', baseline: 'number-baseline.har', candidate: 'lossy-json.har', exit: 2, error: 'NUMBER' },
];

function invoke(scenario, suffix) {
  const paths = Object.fromEntries(['json', 'html', 'junit'].map(format => [format, join(out, `${scenario.name}-${suffix}.${format === 'junit' ? 'xml' : format}`)]));
  const arguments_ = scenario.inspect
    ? ['inspect', '--input', join(fixtures, scenario.inspect), '--json', paths.json]
    : ['compare', '--baseline', join(fixtures, scenario.baseline), '--candidate', join(fixtures, scenario.candidate),
      ...(scenario.config ? ['--config', join(configs, scenario.config)] : []),
      ...(scenario.labels ? ['--labels', scenario.labels] : []),
      '--json', paths.json, '--html', paths.html, '--junit', paths.junit];
  const result = spawnSync(process.execPath, [cli, ...arguments_], { encoding: 'utf8', timeout: 10000 });
  assert.equal(result.error, undefined, `${scenario.name}: failed process`);
  assert.equal(result.status, scenario.exit, `${scenario.name}: unexpected exit; ${result.stdout} ${result.stderr}`);
  assert.equal(result.stderr, '', `${scenario.name}: unexpected stderr`);
  const parsed = JSON.parse(result.stdout);
  const files = {};
  if (scenario.error) {
    assert.equal(parsed.error.code, scenario.error);
    for (const path of Object.values(paths)) assert.ok(absent(path), `${scenario.name}: invalid analysis created ${path}`);
    files[`${scenario.name}.error.json`] = result.stdout;
  } else {
    files[`${scenario.name}.json`] = readFileSync(paths.json, 'utf8');
    assert.equal(files[`${scenario.name}.json`], result.stdout, 'Saved JSON must equal stdout');
    if (!scenario.inspect) {
      files[`${scenario.name}.html`] = readFileSync(paths.html, 'utf8');
      files[`${scenario.name}.xml`] = readFileSync(paths.junit, 'utf8');
    }
  }
  for (const content of Object.values(files)) for (const marker of forbidden) assert.ok(!content.includes(marker), `${scenario.name}: fixture value leaked`);
  return { parsed, files, exit: result.status };
}
function assertInventory(report) {
  assert.equal(report.kind, 'inventory');
  assert.equal(report.analysisPerformed, false);
  assert.deepEqual(report.summary, { entries: 3, operations: 3 });
  assert.deepEqual(report.operations.map(operation => ({ path: operation.selector.path, entries: operation.entries, json: operation.jsonResponses, other: operation.otherResponses, queryVariants: operation.queryVariants })), [
    { path: '/', entries: [1], json: 0, other: 1, queryVariants: 1 },
    { path: '/billing', entries: [3], json: 1, other: 0, queryVariants: 1 },
    { path: '/health', entries: [2], json: 1, other: 0, queryVariants: 1 },
  ]);
  assert.deepEqual(report.configTemplate, { includeOperations: [
    { method: 'GET', origin, path: '/billing' }, { method: 'GET', origin, path: '/health' },
  ] });
}
function assertComparison(scenario, report, files) {
  assert.equal(report.summary.regressions, scenario.regressions);
  assert.equal(report.summary.informational, 0);
  assert.equal(report.summary.pairedEntries, scenario.pairs);
  assert.equal(report.summary.operations, scenario.pairs);
  assert.equal(report.summary.ignoredNodes, scenario.ignored ?? 0);
  if (scenario.scopedIgnored !== undefined) assert.equal(report.summary.scopedIgnoredNodes, scenario.scopedIgnored);
  if (scenario.contractChecks !== undefined) {
    assert.deepEqual(report.contracts, { rules: 1, checks: scenario.contractChecks, regressions: scenario.regressions });
  } else assert.equal(report.contracts, undefined);
  if (scenario.selection) {
    for (const side of ['baseline', 'candidate']) {
      assert.equal(report.coverage[side].totalEntries, 3);
      assert.equal(report.coverage[side].selectedEntries, 2);
      assert.equal(report.coverage[side].excludedEntries, 1);
    }
  }
  assert.equal(report.findings.length, scenario.findings.length);
  for (const [index, expected] of scenario.findings.entries()) {
    const finding = report.findings[index];
    assert.equal(finding.kind, expected.kind);
    assert.equal(finding.severity, 'regression');
    assert.equal(finding.path, scenario.labels === 'ids' ? undefined : expected.path);
    assert.equal(finding.occurrence, 1);
    assert.deepEqual(finding.evidence, { baselineEntry: expected.entry, candidateEntry: expected.entry });
    const operation = report.operations.find(operation => operation.id === finding.operation);
    assert.ok(operation, 'Finding must reference a reported operation');
    assert.equal(operation.path, scenario.labels === 'ids' ? undefined : expected.operationPath);
    if (expected.status !== undefined) assert.equal(finding.status.after, expected.status);
    if (expected.kind.startsWith('contract-')) assert.equal(finding.source, 'authored-contract');
  }
  const xml = files[`${scenario.name}.xml`];
  const counts = `tests="${scenario.findings.length || 1}" failures="${scenario.regressions}" errors="0" skipped="0"`;
  assert.ok(xml.includes(`<testsuites ${counts}>`));
  assert.ok(xml.includes(`<testsuite name="scope-har-compare" ${counts}>`));
  assert.equal((xml.match(/<testcase\b/gu) ?? []).length, scenario.findings.length || 1);
  assert.equal((xml.match(/<failure\b/gu) ?? []).length, scenario.regressions);
  assert.equal((xml.match(/<skipped\b/gu) ?? []).length, 0);
  assert.ok(!/<!DOCTYPE|<!ENTITY/iu.test(xml));
  for (const finding of scenario.findings) {
    assert.ok(xml.includes(`type="${finding.kind}"`));
    if (finding.path && scenario.labels !== 'ids') assert.ok(xml.includes(`JSON pointer: ${finding.path}`));
  }
  const html = files[`${scenario.name}.html`];
  assert.ok(/^<!doctype html>/iu.test(html));
  assert.ok(html.includes('Content-Security-Policy'));
  assert.ok(!/<script\b/iu.test(html));
  if (scenario.labels === 'ids') {
    assert.deepEqual(report.privacy, { labels: 'ids', capturedLabelsOmitted: true });
    assert.deepEqual(report.config, { mode: 'shape', redacted: true, ignorePointerCount: 0, selectionRuleCount: null, contractRuleCount: 1 });
    assert.deepEqual(report.operations, [{ id: 'operation-0001', queryValuesOmitted: true, baseline: [{ entry: 1 }], candidate: [{ entry: 1 }] }]);
    assert.deepEqual(report.findings, [{ id: 'contract-finding-00001', operation: 'operation-0001', occurrence: 1, kind: 'contract-json-missing', severity: 'regression', source: 'authored-contract', rule: 1, requirement: 1, evidence: { baselineEntry: 1, candidateEntry: 1 } }]);
    for (const content of Object.values(files)) {
      for (const label of [origin, 'GET', '/users', '/user/role', 'requiredJson', 'allowedStatuses']) assert.ok(!content.includes(label), `ID-only projection leaked ${label}`);
    }
    assert.ok(html.includes('Captured labels omitted'));
    assert.ok(xml.includes('operation-0001'));
    assert.ok(!xml.includes('JSON pointer:'));
  }
}

const pendingGoldens = new Map();
const results = [];
for (const scenario of cases) {
  const first = invoke(scenario, 'first');
  const second = invoke(scenario, 'repeat');
  assert.deepEqual(first.files, second.files, `${scenario.name}: reports must be byte-identical on rerun`);
  if (scenario.inspect) assertInventory(first.parsed);
  else if (!scenario.error) assertComparison(scenario, first.parsed, first.files);
  for (const [name, content] of Object.entries(first.files)) {
    if (refresh) pendingGoldens.set(name, content);
    else assert.equal(content, readFileSync(join(goldens, name), 'utf8'), `${name}: golden changed; inspect before --refresh`);
  }
  results.push({ name: scenario.name, exit: scenario.exit, repeated: true, outputFiles: scenario.error ? 0 : scenario.inspect ? 1 : 3 });
  console.log(`${scenario.name}: exit ${scenario.exit}; repeated bytes match`);
}
let refreshedFiles = 0;
if (refresh) {
  mkdirSync(goldens, { recursive: true });
  for (const [name, content] of pendingGoldens) {
    const path = join(goldens, name);
    if (!absent(path) && readFileSync(path, 'utf8') === content) continue;
    writeFileSync(path, content, { encoding: 'utf8' });
    refreshedFiles += 1;
  }
}
const receipt = { ok: true, node: process.versions.node, staticSyntheticFixtures: true, browserCapture: false, refreshedGoldens: refresh, cases: results,
  limitations: ['Static synthetic HAR; no capture or replay occurred.', 'JUnit counts and evidence are checked here; full XML parser validation is covered by the separate lab.', 'HTML checks are static; no browser rendering or CSP runtime check is claimed.'] };
writeFileSync(join(out, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx' });
console.log(`Receipt: ${join(out, 'receipt.json')}`);
console.log(refresh ? `Refreshed ${refreshedFiles} changed/new golden files after semantic checks.` : 'All checked-in golden bytes matched.');
