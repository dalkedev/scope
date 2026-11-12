import { appendFileSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

// Never resolve executable code from the consumer's checkout or input paths.
const cli = fileURLToPath(new URL('../../../tools/har-compare/cli.mjs', import.meta.url));
const outputs = {
  'exit-code': '2', result: 'invalid', regressions: '', informational: '',
  'error-code': '', 'json-report': '', 'html-report': '', 'junit-report': '',
};
let code = 2;
let defer = false;
let diagnostic = '';

function fail(errorCode, message) {
  throw Object.assign(new Error(message), { safeCode: errorCode });
}
function pathInput(name, required = true) {
  const value = process.env[name] ?? '';
  if ((!value && required) || value.length > 4096 || /[\u0000-\u001f\u007f]/u.test(value)) {
    fail('ACTION_INPUT', 'Paths must be nonempty when required and contain no control characters (maximum 4096 code units).');
  }
  return value ? resolve(process.env.GITHUB_WORKSPACE, value) : '';
}
// CLI diagnostics deliberately exclude capture text and original paths. Escape
// runner commands even though they are fixed messages or validated ordinals.
const escapeCommand = value => value.replaceAll('%', '%25').replaceAll('\r', '%0D').replaceAll('\n', '%0A');

try {
  if (!process.env.GITHUB_OUTPUT) fail('ACTION_OUTPUT', 'GITHUB_OUTPUT is required to expose comparison results.');
  if (!process.env.GITHUB_WORKSPACE || /[\u0000-\u001f\u007f]/u.test(process.env.GITHUB_WORKSPACE)) {
    fail('ACTION_INPUT', 'GITHUB_WORKSPACE must identify the consumer workspace without control characters.');
  }
  const flag = process.env.SCOPE_FAIL_ON_RESULT ?? 'true';
  if (!['true', 'false'].includes(flag)) fail('ACTION_INPUT', 'fail-on-result must be true or false.');
  defer = flag === 'false';
  const labels = process.env.SCOPE_LABELS ?? 'ids';
  if (!['ids', 'readable'].includes(labels)) fail('ACTION_INPUT', 'labels must be ids or readable.');
  const baseline = pathInput('SCOPE_BASELINE');
  const current = pathInput('SCOPE_CURRENT');
  const config = pathInput('SCOPE_CONFIG', false);
  const directory = pathInput('SCOPE_OUTPUT');
  // Refuse stale directories, even empty ones. Never unlink caller files.
  try {
    mkdirSync(dirname(directory), { recursive: true });
    mkdirSync(directory, { mode: 0o700 });
  } catch {
    fail('ACTION_OUTPUT', 'Could not create a new report directory; use a fresh path in a trusted parent directory.');
  }
  const reports = { 'json-report': join(directory, 'analysis.json'), 'html-report': join(directory, 'report.html'), 'junit-report': join(directory, 'results.xml') };
  const args = [cli, 'compare', '--baseline', baseline, '--candidate', current,
    '--labels', labels, '--json', reports['json-report'], '--html', reports['html-report'], '--junit', reports['junit-report']];
  if (config) args.push('--config', config);
  const child = spawnSync(process.execPath, args, {
    cwd: process.env.GITHUB_WORKSPACE, encoding: 'utf8', shell: false,
    timeout: 120_000, maxBuffer: 32 * 1024 * 1024,
  });
  if (child.error || child.signal || ![0, 1, 2].includes(child.status)) {
    fail('ACTION_EXECUTION', 'Bundled CLI did not complete normally (120-second and 32-MiB output limits apply).');
  }
  let report;
  try { report = JSON.parse(child.stdout); } catch {
    fail('ACTION_EXECUTION', 'Bundled CLI did not return a valid JSON result.');
  }
  if (child.status === 2) {
    if (!/^[A-Z][A-Z_]{0,63}$/u.test(report.error?.code) || typeof report.error?.message !== 'string') {
      fail('ACTION_EXECUTION', 'Bundled CLI did not return a valid error result.');
    }
    fail(report.error.code, report.error.message);
  }
  if (!Number.isSafeInteger(report.summary?.regressions) || report.summary.regressions < 0
    || !Number.isSafeInteger(report.summary?.informational) || report.summary.informational < 0
    || child.status !== (report.summary.regressions > 0 ? 1 : 0)) {
    fail('ACTION_EXECUTION', 'Bundled CLI status and summary did not agree.');
  }
  code = child.status;
  Object.assign(outputs, reports, { 'exit-code': String(code), result: code ? 'regression' : 'clean',
    regressions: String(report.summary.regressions), informational: String(report.summary.informational) });
} catch (error) {
  outputs['error-code'] = error.safeCode ?? 'ACTION_EXECUTION';
  diagnostic = error.safeCode ? error.message : 'Action failed; no complete comparison is available.';
}

try {
  // No captured labels are written to logs or runner command files. Paths were
  // checked for CR/LF and NUL, so each output remains exactly one scalar line.
  appendFileSync(process.env.GITHUB_OUTPUT, Object.entries(outputs).map(([key, value]) => `${key}=${value}\n`).join(''));
} catch {
  process.stderr.write('::error::Snurpo HAR comparison invalid: ACTION_OUTPUT. Could not write result outputs.\n');
  process.exit(2);
}
if (code === 2) {
  process.stderr.write(`::error::Snurpo HAR comparison invalid: ${escapeCommand(outputs['error-code'])}. ${escapeCommand(diagnostic)}\n`);
} else if (code === 1) {
  process.stderr.write(`::warning::Snurpo HAR comparison: regression; ${outputs.regressions} regression candidates. Review local derived reports.\n`);
} else {
  process.stdout.write(`Snurpo HAR comparison: clean; ${outputs.informational} informational findings.\n`);
}
process.exitCode = defer ? 0 : code;
