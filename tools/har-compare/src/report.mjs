import { createHash } from 'node:crypto';
import { mkdir, open, unlink } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fail } from './config.mjs';

const escape = value => String(value).replace(/[&<>"']/gu, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
const css = `:root{font-family:system-ui,sans-serif;color:#183244;background:#f7fafc;font-size:16px}*{box-sizing:border-box}body{max-width:1120px;margin:36px auto;padding:0 24px 48px}header{border-bottom:3px solid #176589;padding-bottom:20px}h1{font-size:2.1rem;letter-spacing:-.035em;margin:.3em 0}h2{font-size:1.35rem;margin:32px 0 14px}h3{font-size:1.05rem;margin:0 0 10px}h4{font-size:1rem;margin:8px 0}p{line-height:1.65;max-width:78ch}a{color:#176589;text-underline-offset:3px}a:focus-visible,summary:focus-visible{outline:3px solid #176589;outline-offset:4px}nav{display:flex;gap:12px 24px;flex-wrap:wrap;margin:18px 0}.badge{display:inline-block;background:#e5eef4;padding:8px 12px;font-weight:650}.overview{display:flex;gap:12px 28px;flex-wrap:wrap}.operation-group{margin:24px 0 32px;min-width:0}.operation-group>h3{overflow-wrap:anywhere}.regression{border-left:4px solid #a12e47}.info{border-left:4px solid #126c52}article{background:white;padding:14px 18px;margin:10px 0;overflow-wrap:anywhere}article p{margin:8px 0}code,pre{font-family:ui-monospace,monospace;white-space:pre-wrap;overflow-wrap:anywhere}small,.muted{color:#526a7b}.evidence{font-size:.9rem}.source-label{font-size:.85rem;font-weight:650}details{border:1px solid #d8e2e9;padding:14px;background:white}summary{cursor:pointer;font-weight:650}pre{font-size:.82rem;line-height:1.5;max-width:100%}.table-scroll{overflow-x:auto}table{width:100%;border-collapse:collapse;background:white;font-size:.9rem}caption{text-align:left;font-weight:650;margin:0 0 10px}th,td{padding:12px;text-align:left;border-bottom:1px solid #d8e2e9;vertical-align:top}th{background:#eaf1f6}td:first-child{min-width:220px;overflow-wrap:anywhere}td code{white-space:normal}section{scroll-margin-top:20px}.notice{border-left:3px solid #176589;padding:1px 0 1px 16px}footer{border-top:1px solid #d8e2e9;margin-top:32px;padding-top:12px;font-size:.85rem}@media(max-width:600px){body{margin:20px auto;padding:0 16px 32px}h1{font-size:1.7rem}.overview{display:block}article{padding:12px}.table-scroll{margin:0 -4px}nav{gap:10px 18px}}@media print{:root{background:white}body{max-width:none;margin:0;padding:0;font-size:10pt}nav{display:none}article,table,details{break-inside:avoid}a{color:inherit}header{border-color:#183244}}`;
export const jsonReport = report => JSON.stringify(report, null, 2) + '\n';
const labels = {
  'field-removed': 'Observed field removed', 'field-added': 'Observed field added',
  'type-changed': 'Observed JSON type changed', 'value-changed': 'Scalar value changed',
  'array-length-changed': 'Observed array length changed', 'body-kind-changed': 'Response body kind changed',
  'http-failure': 'Successful response became an HTTP error', 'status-changed': 'HTTP status changed',
  'operation-added': 'Operation added', 'operation-removed': 'Operation removed',
  'contract-status': 'Allowed HTTP status check failed', 'contract-json-missing': 'Required JSON field missing',
  'contract-json-type': 'Required JSON type check failed', 'contract-json-body': 'Required JSON body missing',
  'contract-operation-missing': 'Required operation missing',
};
function findingCard(f, report) {
  const authored = f.source === 'authored-contract';
  let status = '', types = '';
  if (f.status) {
    const allowed = authored ? report.config.contracts?.[f.rule - 1]?.allowedStatuses : undefined;
    status = allowed ? `<p>Allowed HTTP statuses: <code>${escape(allowed.join(', '))}</code>. Observed: <code>${escape(f.status.after)}</code>.</p>` : authored && report.privacy?.labels === 'ids' ? `<p>Authored allowed-status check failed. Observed HTTP status: <code>${escape(f.status.after)}</code>. Configured status list omitted with labels.</p>` : `<p>Observed HTTP status: <code>${escape(f.status.before ?? 'unasserted')}</code> → <code>${escape(f.status.after)}</code>.</p>`;
  }
  if (f.types) types = authored ? `<p>Required type: <code>${escape(f.types.before)}</code>. Observed type: <code>${escape(f.types.after)}</code>.</p>` : `<p>Observed type: <code>${escape(f.types.before ?? 'absent')}</code> → <code>${escape(f.types.after ?? 'absent')}</code>.</p>`;
  return `<article class="${f.severity === 'regression' ? 'regression' : 'info'}"><span class="source-label">${authored ? 'Authored contract' : 'Observed change'}</span><h4>${escape(labels[f.kind] ?? f.kind)}</h4><small>${escape(f.id)} · ${escape(f.severity)}</small>${f.path !== undefined ? `<p>${authored ? 'Authored' : 'Observed'} JSON pointer: <code>${escape(f.path || '(root)')}</code></p>` : ''}${status}${types}<p class="evidence">${escape(f.operation)} · occurrence ${escape(f.occurrence)} · baseline entry ${escape(f.evidence.baselineEntry ?? 'none')} → candidate entry ${escape(f.evidence.candidateEntry ?? 'none')}</p></article>`;
}

export function htmlReport(report) {
  const hash = createHash('sha256').update(css).digest('base64');
  const groups = new Map(report.operations.map(op => [op.id, []]));
  for (const finding of report.findings) {
    if (!groups.has(finding.operation)) fail('REPORT', 'HTML finding refers to an unknown operation.');
    groups.get(finding.operation).push(finding);
  }
  const operationLabel = op => `${op.method ?? ''} ${op.origin ?? ''}${op.path ?? ''}`.trim() || op.id;
  const sections = report.operations.filter(op => groups.get(op.id).length).map(op => {
    const altered = [...op.baseline, ...op.candidate].some(e => e.edited || ['mock', 'blocked'].includes(e.provenance));
    return `<section class="operation-group" id="${escape(op.id)}"><h3><code>${escape(operationLabel(op))}</code></h3><p class="muted">${escape(op.id)} · ${op.baseline.length} baseline / ${op.candidate.length} candidate entries${op.queryValuesOmitted ? ' · query values omitted from label' : ''}${altered ? ' · mock, blocked or edited evidence present' : ''}</p>${groups.get(op.id).map(f => findingCard(f, report)).join('\n')}</section>`;
  }).join('\n');
  const rows = report.operations.map(op => `<tr><td>${groups.get(op.id).length ? `<a href="#${escape(op.id)}"><code>${escape(operationLabel(op))}</code></a>` : `<code>${escape(operationLabel(op))}</code>`}<br><small>${escape(op.id)}${op.queryValuesOmitted ? ' · query label omitted' : ''}</small></td><td>${op.baseline.length}</td><td>${op.candidate.length}</td><td>${groups.get(op.id).length}</td></tr>`).join('\n');
  const coverage = report.coverage;
  const selection = coverage ? `<section class="notice"><h2>Selection scope</h2><p>Baseline: ${coverage.baseline.selectedEntries} selected / ${coverage.baseline.totalEntries} total; ${coverage.baseline.excludedEntries} excluded. Candidate: ${coverage.candidate.selectedEntries} selected / ${coverage.candidate.totalEntries} total; ${coverage.candidate.excludedEntries} excluded.</p><p>Excluded responses were not analyzed. This result applies only to the configured method/origin/path selectors; it is not a health check of the entire capture.</p></section>` : '';
  const assertions = report.contracts ? `<section class="notice"><h2>Authored assertions</h2><p>${report.contracts.checks} checks across ${report.contracts.rules} explicitly configured operation rules; ${report.contracts.regressions} failures. These requirements were authored by the user, not inferred from HAR observations.</p></section>` : '';
  const privacyNotice = report.privacy?.labels === 'ids' ? '<section class="notice"><h2>Captured labels omitted</h2><p>Operations and findings use report-local identifiers. Original method, origin, path, JSON pointer and configuration labels are hidden. Entry numbers still refer to the private HAR files; counts, statuses, types and capture provenance remain visible. This minimizes labels rather than anonymizing interaction patterns.</p></section>' : '';
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'sha256-${hash}'; base-uri 'none'; form-action 'none'"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Snurpo HAR comparison</title><style>${css}</style></head><body>
<header><p>Snurpo HAR comparison</p><h1>${report.summary.regressions ? 'Review required' : 'No configured regression'}</h1><p class="badge">${report.summary.regressions} regression candidates · ${report.summary.informational} informational changes · ${report.summary.pairedEntries} paired responses</p><div class="overview"><p>Mode: <strong>${escape(report.config.mode)}</strong></p><p>Ignored JSON nodes: ${report.summary.ignoredNodes}${report.summary.scopedIgnoredNodes !== undefined ? ` (${report.summary.scopedIgnoredNodes} scoped)` : ''}</p></div></header>
<nav aria-label="Report sections"><a href="#findings">Findings</a><a href="#coverage">Coverage</a><a href="#method">Interpretation</a><a href="#analysis">Analysis JSON</a></nav>
${privacyNotice}${selection}${assertions}<main><section id="findings"><h2>Findings</h2>${sections || '<p>No configured regression or informational changes were found. Review the coverage and configured assertions below.</p>'}</section>
<section id="coverage"><h2>Compared operations</h2><div class="table-scroll"><table id="operation-coverage"><caption>Operation coverage</caption><thead><tr><th scope="col">Operation</th><th scope="col">Baseline entries</th><th scope="col">Candidate entries</th><th scope="col">Findings</th></tr></thead><tbody>${rows || '<tr><td colspan="4">No operations were present in either analyzed capture.</td></tr>'}</tbody></table></div></section>
<section id="method"><h2>How to read this report</h2><p>Observed fields describe captured payloads, not required schema fields or proven client incompatibility. Authored contract failures refer to requirements explicitly supplied in configuration. Without an authored status check, unchanged HTTP errors may pass this change detector.</p><p>Repeated requests pair by encounter order; reordered responses and request-body variants may be ambiguous. Origins and lossless query values participate in matching; query values are omitted from labels. Different operations can therefore share a label. Entry numbers refer to the original HAR log.entries, starting at 1.</p><p>Headers, cookies, body values, URL credentials, query values and fragments are excluded. Origin/path and JSON field names can still be sensitive: review this artifact before sharing. No traffic is replayed and no model or external asset is loaded.</p></section>
<section id="analysis"><h2>Full analysis</h2><details><summary>Analysis JSON, configuration and evidence</summary><pre>${escape(jsonReport(report))}</pre></details></section></main>
<footer>Local comparison report · schema ${escape(report.schemaVersion)} · deterministic evidence without run timestamps</footer>
</body></html>\n`;
}
/** Exclusive output creation: never overwrite HAR/config, existing reports or symlinks. */
export async function writeReports(outputs) {
  const paths = outputs.map(([path]) => resolve(path));
  if (new Set(paths).size !== paths.length) fail('OUTPUT', 'Report output paths must be distinct.');
  const opened = [];
  try {
    for (const [index, [, content]] of outputs.entries()) {
      await mkdir(dirname(paths[index]), { recursive: true });
      const handle = await open(paths[index], 'wx', 0o600);
      opened.push({ handle, path: paths[index], content });
    }
    for (const { handle, content } of opened) await handle.writeFile(content, 'utf8');
  } catch {
    for (const { path } of opened) await unlink(path).catch(() => {});
    fail('OUTPUT', 'Could not create reports; use distinct new output paths in a trusted directory.');
  } finally { await Promise.all(opened.map(({ handle }) => handle.close())); }
}
