import { useState } from 'react';
import {
  AlertTriangle,
  ArrowLeftRight,
  CheckCircle2,
  CircleHelp,
  Clipboard,
  Info,
  LoaderCircle,
  Play,
  RefreshCw,
  ShieldCheck,
  Smartphone,
  Stethoscope,
} from 'lucide-react';
import { api } from '../api';
import { messageOf } from '../form-utils';
import type { DeviceDiagnostics, DiagnosticCheck, ProxyStatus } from '../types';
import { CopyCommand, InlineError, WorkspaceHeading } from './WorkbenchUi';
const checkIcon = (status: DiagnosticCheck['status']) =>
  status === 'ok' ? (
    <CheckCircle2 size={17} />
  ) : status === 'warning' || status === 'error' ? (
    <AlertTriangle size={17} />
  ) : (
    <Info size={17} />
  );
export function DiagnosticsWorkspace({
  native,
  proxy,
  version,
  onOpenDevice,
}: {
  native: boolean;
  proxy: ProxyStatus;
  version?: string;
  onOpenDevice: () => void;
}) {
  const [report, setReport] = useState<DeviceDiagnostics | null>(null),
    [serial, setSerial] = useState(''),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(''),
    [copied, setCopied] = useState(false);
  async function run() {
    if (busy || !native) return;
    setBusy(true);
    setError('');
    try {
      const value = await api.device_diagnostics(serial || undefined);
      setReport(value);
      if (value.selectedSerial) setSerial(value.selectedSerial);
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  }
  const failures =
    report?.checks.filter((c) => c.status === 'error' || c.status === 'warning').length || 0;
  return (
    <section className="workbench-page diagnostics-page">
      <WorkspaceHeading
        icon={<Stethoscope size={24} />}
        title="Diagnostics"
        subtitle="Find what is ready, what needs attention and what to do next."
        actions={
          <>
            <button className="button" onClick={onOpenDevice}>
              <Smartphone size={14} /> Device connection
            </button>
            <button
              className="button primary"
              disabled={!native || busy}
              onClick={() => void run()}
            >
              {busy ? (
                <LoaderCircle size={14} className="spin" />
              ) : report ? (
                <RefreshCw size={14} />
              ) : (
                <Play size={14} />
              )}{' '}
              {report ? 'Run checks again' : 'Run read-only checks'}
            </button>
          </>
        }
      />
      <InlineError message={error} />
      <div className="diagnostic-intro">
        <ShieldCheck size={18} />
        <div>
          <strong>Read-only preflight</strong>
          <p>
            Checks ADB, device authorization, runtime versions and Snurpo-owned mappings. Does not
            boot an emulator, install software or change device settings.
          </p>
        </div>
        <span className="mini-badge">{version ? `Snurpo ${version}` : 'Local engine'}</span>
      </div>
      <div className="diagnostic-engine-line">
        <span className={`tiny-dot ${proxy.running ? 'green' : ''}`} />
        <span>Proxy {proxy.running ? `listening on 127.0.0.1:${proxy.port}` : 'stopped'}</span>
        <span>Request breakpoints {proxy.requestBreakpoint ? 'on' : 'off'}</span>
        <span>Response breakpoints {proxy.responseBreakpoint ? 'on' : 'off'}</span>
      </div>
      {!report ? (
        <div className="workbench-surface diagnostics-empty">
          <Stethoscope size={32} />
          <h2>Start with evidence.</h2>
          <p>
            Run the preflight to get actionable checks for your current environment. Snurpo will
            report missing tools and unsupported combinations explicitly.
          </p>
          <button className="button" disabled={!native || busy} onClick={() => void run()}>
            {busy ? <LoaderCircle size={14} className="spin" /> : <Play size={14} />} Run checks
          </button>
          {!native && <small>Diagnostics require the native desktop app.</small>}
        </div>
      ) : (
        <div className="diagnostics-report">
          <div className="diagnostic-report-toolbar">
            <div>
              <strong>
                {failures
                  ? `${failures} ${failures === 1 ? 'item needs' : 'items need'} attention`
                  : 'Preflight complete'}
              </strong>
              <span>Checked {new Date(report.checkedAt).toLocaleString()}</span>
            </div>
            <label className="device-diagnostic-select">
              <span>Device</span>
              <select
                className="text-input"
                value={serial}
                aria-label="Diagnostics device"
                disabled={busy}
                onChange={(e) => setSerial(e.target.value)}
              >
                <option value="">Auto-select when only one device is connected</option>
                {report.devices.map((d) => (
                  <option value={d.serial} key={d.serial}>
                    {d.model || d.serial} · {d.serial} · {d.state}
                  </option>
                ))}
              </select>
            </label>
            <button
              className="button small"
              onClick={() =>
                void navigator.clipboard.writeText(JSON.stringify(report, null, 2)).then(() => {
                  setCopied(true);
                  window.setTimeout(() => setCopied(false), 2000);
                })
              }
            >
              <Clipboard size={13} />
              {copied ? 'Copied' : 'Copy report'}
            </button>
          </div>
          <div className="readiness-lines">
            <div>
              <span>
                <Smartphone size={16} /> System proxy
              </span>
              <strong className={report.readyForProxy ? 'readiness-ok' : 'readiness-attention'}>
                {report.readyForProxy ? 'Preflight ready' : 'Setup required'}
              </strong>
            </div>
            <div>
              <span>
                <Stethoscope size={16} /> Frida instrumentation <small>experimental</small>
              </span>
              <strong className={report.readyForFrida ? 'readiness-ok' : 'readiness-attention'}>
                {report.readyForFrida ? 'Preflight ready' : 'Setup required'}
              </strong>
            </div>
          </div>
          <div className="diagnostic-checks">
            {report.checks.map((check) => (
              <article className={`diagnostic-check check-${check.status}`} key={check.id}>
                <span className="check-status-icon">{checkIcon(check.status)}</span>
                <div>
                  <h3>{check.title}</h3>
                  <p>{check.detail}</p>
                  {check.action && <p className="check-next-action">{check.action}</p>}
                  {check.command && <CopyCommand command={check.command} />}
                </div>
                <span className="check-status-word">{check.status}</span>
              </article>
            ))}
          </div>
          <details className="diagnostic-details">
            <summary>
              <CircleHelp size={15} /> Environment evidence
            </summary>
            <dl>
              <div>
                <dt>ADB</dt>
                <dd>
                  {report.adb.available
                    ? report.adb.version || 'Available'
                    : report.adb.error || 'Not available'}
                </dd>
              </div>
              <div>
                <dt>Device</dt>
                <dd>
                  {report.device
                    ? `${report.device.model} · Android ${report.device.androidVersion || 'unknown'} · ${report.device.abi || 'unknown ABI'}`
                    : 'No selected device'}
                </dd>
              </div>
              <div>
                <dt>Root state</dt>
                <dd>{report.device?.rootStatus || 'Not checked'}</dd>
              </div>
              <div>
                <dt>Frida runtime</dt>
                <dd>
                  {report.frida.runtimeVersion || report.frida.runtimeError || 'Not available'}
                </dd>
              </div>
              <div>
                <dt>Frida server</dt>
                <dd>{report.frida.serverVersion || report.frida.serverError || 'Not available'}</dd>
              </div>
              <div>
                <dt>Recovery record</dt>
                <dd>
                  {report.recovery.needsCleanup
                    ? 'Cleanup required; open Device connection to restore settings.'
                    : report.recovery.active
                      ? 'Current connection has an active recovery record.'
                      : report.recovery.exists
                        ? 'Record present.'
                        : 'No outstanding record.'}
                </dd>
              </div>
            </dl>
          </details>
          {[...(report.mappings.forward || []), ...(report.mappings.reverse || [])].length > 0 && (
            <section className="diagnostic-mappings">
              <h3>
                <ArrowLeftRight size={15} /> Observed ADB mappings
              </h3>
              <table>
                <thead>
                  <tr>
                    <th>Device</th>
                    <th>Direction</th>
                    <th>Local</th>
                    <th>Remote</th>
                    <th>Owner</th>
                  </tr>
                </thead>
                <tbody>
                  {[...report.mappings.forward, ...report.mappings.reverse].map((m, i) => (
                    <tr key={i}>
                      <td>{m.serial}</td>
                      <td>{m.kind}</td>
                      <td>{m.local}</td>
                      <td>{m.remote}</td>
                      <td>{m.owned ? 'Snurpo' : 'Other'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          )}
          {report.notes.length > 0 && (
            <div className="diagnostic-notes">
              {report.notes.map((note, i) => (
                <p key={i}>
                  <Info size={13} />
                  {note}
                </p>
              ))}
            </div>
          )}
          <p className="diagnostic-share-note">
            Diagnostic reports contain local device identifiers and runtime paths. Review a copied
            report before sharing it.
          </p>
        </div>
      )}
    </section>
  );
}
