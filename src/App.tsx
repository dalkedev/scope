import { useCallback, useEffect, useRef, useState } from 'react';
import type { ChangeEvent, FormEvent, ReactNode } from 'react';
import {
  Activity,
  ArrowDown,
  ArrowDownToLine,
  ArrowLeftRight,
  ArrowRight,
  ArrowUpFromLine,
  BarChart3,
  Check,
  Clock3,
  GitBranch,
  Pin,
  Stethoscope,
  ChevronDown,
  ChevronRight,
  Circle,
  Code2,
  Copy,
  Download,
  FolderArchive,
  Globe2,
  HardDrive,
  Info,
  Layers3,
  LoaderCircle,
  MoreHorizontal,
  Pause,
  Play,
  Plus,
  Radio,
  RefreshCw,
  RotateCcw,
  Search,
  Settings2,
  ShieldCheck,
  Smartphone,
  Square,
  Terminal,
  Unplug,
  Wifi,
  X,
  Zap,
} from 'lucide-react';
import { api, isNative } from './api';
import { isExchangeError } from './exchange-utils';
import { queryTraffic } from './traffic-search';
import { ExchangeStatus as Status } from './components/ExchangeStatus';
import { TrafficSearchHelp } from './components/TrafficSearchHelp';
import { RulesWorkspace } from './components/RulesWorkspace';
import { ReplayComposer } from './components/ReplayComposer';
import { BreakpointModal } from './components/BreakpointModal';
import { SessionDetails } from './components/SessionDetails';
import { ActivityWorkspace } from './components/ActivityWorkspace';
import { DiagnosticsWorkspace } from './components/DiagnosticsWorkspace';
import type {
  Analysis,
  App as AndroidApp,
  Bootstrap,
  Comparison,
  Device,
  DeviceStatus,
  EditMessage,
  Endpoint,
  Exchange,
  Header,
  McpStatus,
  PendingExchange,
  ProxyStatus,
  Session,
} from './types';

type View = 'traffic' | 'analysis' | 'compare';
type Panel = 'device' | 'mcp' | 'connection' | 'rules' | 'activity' | 'diagnostics' | null;
const initialProxy: ProxyStatus = {
  running: false,
  port: 17890,
  sessionId: null,
  requestBreakpoint: false,
  responseBreakpoint: false,
  caCertPath: '',
};
const initialDevice: DeviceStatus = {
  running: false,
  serial: null,
  package: null,
  mode: null,
  message: '',
  logs: [],
};
const bytes = (n = 0) =>
  n < 1024
    ? `${n} B`
    : n < 1048576
      ? `${(n / 1024).toFixed(1)} kB`
      : `${(n / 1048576).toFixed(1)} MB`;
const ms = (n = 0) => (n < 1000 ? `${Math.round(n)} ms` : `${(n / 1000).toFixed(2)} s`);
const safeUrl = (url: string) => {
  try {
    const u = new URL(url);
    return { host: u.host, path: u.pathname + u.search, protocol: u.protocol };
  } catch {
    return { host: '', path: url, protocol: '' };
  }
};
const pretty = (value: string, binary: boolean) => {
  if (binary) return value;
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
};
const time = (value: number) =>
  new Date(value).toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  });
const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));
function IconButton({
  children,
  label,
  onClick,
  disabled,
  active,
}: {
  children: ReactNode;
  label: string;
  onClick?: () => void;
  disabled?: boolean;
  active?: boolean;
}) {
  return (
    <button
      className={`icon-button ${active ? 'active' : ''}`}
      title={label}
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
    >
      {children}
    </button>
  );
}
function Empty({
  icon,
  title,
  children,
  action,
}: {
  icon: ReactNode;
  title: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="empty-state">
      <div className="empty-icon">{icon}</div>
      <h2>{title}</h2>
      <p>{children}</p>
      {action}
    </div>
  );
}
function Headers({ headers }: { headers: Header[] }) {
  return headers.length ? (
    <dl className="header-list">
      {headers.map((h, i) => (
        <div key={i}>
          <dt>{h.name}</dt>
          <dd>{h.value}</dd>
        </div>
      ))}
    </dl>
  ) : (
    <p className="quiet-note">No headers recorded.</p>
  );
}
function Body({ body, binary }: { body: string; binary: boolean }) {
  return body ? (
    <>
      <div className="code-toolbar">
        <span>
          {binary
            ? 'Binary · base64'
            : (() => {
                try {
                  JSON.parse(body);
                  return 'JSON';
                } catch {
                  return 'Text';
                }
              })()}
        </span>
        <button className="text-button" onClick={() => navigator.clipboard.writeText(body)}>
          <Copy size={12} /> Copy
        </button>
      </div>
      <pre className="body-code">
        <code>{pretty(body, binary)}</code>
      </pre>
    </>
  ) : (
    <p className="quiet-note">This message has no body.</p>
  );
}
function download(value: unknown, name: string) {
  const contents = typeof value === 'string' ? value : JSON.stringify(value, null, 2);
  const objectUrl = URL.createObjectURL(new Blob([contents], { type: 'application/json' }));
  const a = document.createElement('a');
  a.href = objectUrl;
  a.download = name;
  a.click();
  window.setTimeout(() => URL.revokeObjectURL(objectUrl), 1000);
}

function validatedListenerPort(draft: string): number {
  const port = Number(draft);
  if (!Number.isInteger(port) || port < 1024 || port > 65535)
    throw new Error('Enter a whole proxy port from 1024 to 65535.');
  return port;
}

export default function App() {
  const [boot, setBoot] = useState<Bootstrap | null>(null);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [sessionId, setSessionId] = useState('');
  const sessionIdRef = useRef(sessionId);
  const [proxy, setProxy] = useState(initialProxy);
  const [proxyPortDraft, setProxyPortDraft] = useState('17890');
  const proxyEpoch = useRef(0);
  const [device, setDevice] = useState(initialDevice);
  const [exchanges, setExchanges] = useState<Exchange[]>([]);
  const [selected, setSelected] = useState<Exchange | null>(null);
  const [pending, setPending] = useState<PendingExchange[]>([]);
  const [activePending, setActivePending] = useState<PendingExchange | null>(null);
  const resolvingPending = useRef<string | null>(null);
  const pendingEpoch = useRef(0);
  const [view, setView] = useState<View>('traffic');
  const [panel, setPanel] = useState<Panel>(null);
  const [query, setQuery] = useState('');
  const [searchError, setSearchError] = useState('');
  const [statusFilter, setStatusFilter] = useState('all');
  const [limit, setLimit] = useState(250);
  const [busy, setBusy] = useState('');
  const [replayingId, setReplayingId] = useState('');
  const [loading, setLoading] = useState(isNative());
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [newSession, setNewSession] = useState(false);
  const [sessionDetails, setSessionDetails] = useState<Session | null>(null);
  const [replayDraft, setReplayDraft] = useState<Exchange | null>(null);
  const [exportRedacted, setExportRedacted] = useState(true);
  const [exportOpen, setExportOpen] = useState(false);
  const [analysis, setAnalysis] = useState<Analysis | null>(null);
  const [comparison, setComparison] = useState<Comparison | null>(null);
  const [compareId, setCompareId] = useState('');
  const [mcp, setMcp] = useState<McpStatus>({ sessionIds: [], allowWrites: false, port: 0 });
  const importRef = useRef<HTMLInputElement>(null);
  const selectedSession = sessions.find((s) => s.id === sessionId);
  const count = selectedSession?.exchangeCount ?? 0;
  const revision = selectedSession?.revision;
  const pendingSignature = pending
    .map((p) => `${p.id}:${p.stage}:${p.exchangeId}`)
    .sort()
    .join('|');
  const native = isNative();

  useEffect(() => {
    // Polling reports listener state, not the user's next-listener preference.
    // Only a running listener has an authoritative bound port to adopt.
    if (proxy.running) setProxyPortDraft(String(proxy.port));
  }, [proxy.running, proxy.port]);

  const refreshSessions = useCallback(async () => {
    const all = await api.list_sessions();
    setSessions(all);
    return all;
  }, []);
  const loadBootstrap = useCallback(async () => {
    setLoading(true);
    try {
      const b = await api.bootstrap();
      setBoot(b);
      setSessions(b.sessions);
      setProxy(b.proxy);
      setDevice(b.deviceStatus);
      if (b.mcpStatus) setMcp(b.mcpStatus);
      setSessionId((id) => id || b.proxy.sessionId || b.sessions[0]?.id || '');
      setError('');
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }, []);
  useEffect(() => {
    if (native) void loadBootstrap();
  }, [native, loadBootstrap]);
  useEffect(() => {
    if (!native) return;
    let alive = true;
    let nextPoll = 0;
    let lastAppliedPoll = 0;
    const poll = async () => {
      const sequence = ++nextPoll;
      const epoch = pendingEpoch.current;
      const listenerEpoch = proxyEpoch.current;
      const results = await Promise.allSettled([
        api.list_sessions(),
        api.proxy_status(),
        api.pending(),
        api.device_status(),
      ]);
      // Native calls can outlive the polling interval. Never let an older
      // snapshot discard a newer pending message and its unsent editor fields.
      if (!alive || sequence < lastAppliedPoll) return;
      lastAppliedPoll = sequence;
      const [s, p, b, d] = results;
      if (s.status === 'fulfilled') setSessions(s.value);
      if (p.status === 'fulfilled' && listenerEpoch === proxyEpoch.current) setProxy(p.value);
      // A successful Continue/Abort retires its ID immediately. A snapshot
      // requested before that resolution must not resurrect it afterwards.
      if (b.status === 'fulfilled' && epoch === pendingEpoch.current) setPending(b.value);
      if (d.status === 'fulfilled') setDevice(d.value);
    };
    const id = window.setInterval(() => void poll(), 2000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [native]);
  useEffect(() => {
    if (activePending && !pending.some((item) => item.id === activePending.id)) {
      setActivePending(null);
      setNotice(
        'This message is no longer paused. It may have expired, been aborted or continued elsewhere.',
      );
    }
  }, [pendingSignature, activePending?.id]);
  useEffect(() => {
    sessionIdRef.current = sessionId;
    setSelected(null);
    setExchanges([]);
    setSearchError('');
    setLimit(250);
    setAnalysis(null);
    setComparison(null);
    setCompareId((current) => (current === sessionId ? '' : current));
  }, [sessionId]);
  useEffect(() => {
    if (!native || !sessionId) return;
    let alive = true;
    const timer = window.setTimeout(
      () => {
        queryTraffic(sessionId, query, limit).then(({ rows, error }) => {
          if (alive) {
            setExchanges(rows);
            setSearchError(error);
          }
        });
      },
      query ? 220 : 0,
    );
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [native, sessionId, query, count, revision, pendingSignature, limit]);
  useEffect(() => {
    if (view !== 'analysis' || !sessionId || !native) return;
    let alive = true;
    api
      .analysis(sessionId)
      .then((value) => {
        if (alive) setAnalysis(value);
      })
      .catch((e) => setError(errorText(e)));
    return () => {
      alive = false;
    };
  }, [view, sessionId, count, revision, native]);
  useEffect(() => {
    if (!selected || !native) return;
    const summary = exchanges.find((row) => row.id === selected.id);
    if (!summary) return;
    const lifecycle = (row: Exchange) =>
      [
        row.status,
        row.error,
        row.edited,
        row.completed,
        row.durationMs,
        row.requestBytes,
        row.responseBytes,
      ].join('|');
    if (lifecycle(summary) === lifecycle(selected)) return;
    let alive = true;
    api
      .get_exchange(selected.id)
      .then((full) => {
        if (alive) setSelected((current) => (current?.id === full.id ? full : current));
      })
      .catch((e) => {
        if (alive) setError(errorText(e));
      });
    return () => {
      alive = false;
    };
  }, [exchanges, selected?.id, native]);
  useEffect(() => {
    if (!notice) return;
    const id = window.setTimeout(() => setNotice(''), 5000);
    return () => clearTimeout(id);
  }, [notice]);
  async function perform(label: string, task: () => Promise<void>) {
    if (busy) return;
    setBusy(label);
    setError('');
    try {
      await task();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy('');
    }
  }
  async function ensureSession(name = `Session ${sessions.length + 1}`, source = 'desktop') {
    if (sessionId) return sessionId;
    const s = await api.create_session(name, source);
    setSessions((all) => [s, ...all]);
    setSessionId(s.id);
    return s.id;
  }
  function applyProxyControl(status: ProxyStatus) {
    // A listener control receipt is newer than any status poll begun before it.
    proxyEpoch.current += 1;
    setProxy(status);
  }
  const toggleCapture = () =>
    perform('capture', async () => {
      if (proxy.running) {
        if (device.running) setDevice(await api.stop_device());
        await api.stop_proxy();
        applyProxyControl(await api.proxy_status());
        setNotice('Capture stopped. Your traffic is saved.');
      } else {
        const port = validatedListenerPort(proxyPortDraft);
        const id = await ensureSession();
        applyProxyControl(await api.start_proxy(id, port));
        setNotice('Local proxy listening. Connect a device or configure your HTTP client.');
      }
    });
  const demo = () =>
    perform('demo', async () => {
      if (proxy.requestBreakpoint || proxy.responseBreakpoint)
        throw new Error('Disable request and response breakpoints before generating test traffic.');
      const port = proxy.running ? proxy.port : validatedListenerPort(proxyPortDraft);
      const id = await ensureSession('Demo traffic', 'local-demo');
      if (!proxy.running) applyProxyControl(await api.start_proxy(id, port));
      if (proxy.running && proxy.sessionId !== id)
        throw new Error('Stop the active capture before generating traffic in another session.');
      await api.generate_demo(id);
      await refreshSessions();
      setNotice('Real HTTP requests captured through your local proxy.');
    });
  const exportFile = (format: 'har' | 'scope' | 'openapi') =>
    perform('export', async () => {
      if (!sessionId) return;
      if (native) {
        const result = await api.save_export(sessionId, format, exportRedacted);
        if (result.saved)
          setNotice(
            `${format === 'har' ? 'HAR' : format === 'openapi' ? 'Observed OpenAPI' : 'Scope session'} exported${result.path ? ` to ${result.path}` : ''}.`,
          );
        return;
      }
      const data =
        format === 'har'
          ? await api.export_har(sessionId)
          : format === 'openapi'
            ? await api.infer_openapi(sessionId)
            : await api.export_session(sessionId);
      download(
        data,
        `${(selectedSession?.name || 'session').replace(/[^a-z0-9-]/gi, '-').toLowerCase()}.${format === 'har' ? 'har' : format === 'openapi' ? 'openapi.json' : 'scope.json'}`,
      );
      setNotice(
        `${format === 'har' ? 'HAR' : format === 'openapi' ? 'Observed OpenAPI' : 'Scope session'} exported.`,
      );
    });
  async function importFile(e: ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    await perform('import', async () => {
      const content = await file.text();
      const parsed = JSON.parse(content);
      const s = parsed.log
        ? await api.import_har(content, file.name.replace(/\.har$/i, ''))
        : await api.import_session(content);
      await refreshSessions();
      setSessionId(s.id);
      setNotice(`Imported ${s.name}.`);
    });
    e.target.value = '';
  }
  async function selectExchange(row: Exchange) {
    setSelected(row);
    try {
      const full = await api.get_exchange(row.id);
      setSelected((current) => (current?.id === full.id ? full : current));
    } catch (e) {
      setError(errorText(e));
    }
  }
  const filtered = exchanges.filter(
    (e) =>
      statusFilter === 'all' ||
      (statusFilter === 'errors' && isExchangeError(e)) ||
      (statusFilter === '2xx' && (e.status ?? 0) >= 200 && (e.status ?? 0) < 300) ||
      (statusFilter === 'edited' && e.edited),
  );
  const requestCount = exchanges.length;
  const totalSize = exchanges.reduce((n, e) => n + e.requestBytes + e.responseBytes, 0);
  const runMcp = (
    ids: string[],
    writes: boolean,
    rules = mcp.allowRules || false,
    diagnostics = mcp.allowDeviceDiagnostics || false,
  ) =>
    perform('mcp', async () => {
      await api.set_mcp_scope(ids, writes, rules, diagnostics);
      setMcp({
        ...mcp,
        sessionIds: ids,
        allowWrites: writes,
        allowRules: rules,
        allowDeviceDiagnostics: diagnostics,
      });
      setNotice('MCP access updated.');
    });

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand-row">
          <div className="brand-mark">
            <span />
            <span />
            <span />
          </div>
          <span className="brand-name">scope</span>
          <span className="preview-badge">local</span>
        </div>
        <button
          className={`device-switch ${panel === 'device' ? 'is-open' : ''}`}
          onClick={() => setPanel('device')}
        >
          <span className={`device-symbol ${device.running ? 'connected' : ''}`}>
            <Smartphone size={19} />
          </span>
          <span>
            <strong>{device.running ? device.serial : 'Connect a device'}</strong>
            <small>{device.running ? device.package : 'Android · USB or emulator'}</small>
          </span>
          <ChevronDown size={13} />
        </button>
        <nav className="primary-nav" aria-label="Workspace">
          <button
            onClick={() => {
              setView('traffic');
              setPanel(null);
            }}
            className={view === 'traffic' && !panel ? 'selected' : ''}
          >
            <Activity size={17} />
            <span>Traffic</span>
            <span className="nav-count">{count}</span>
          </button>
          <button className={panel === 'rules' ? 'selected' : ''} onClick={() => setPanel('rules')}>
            <GitBranch size={17} />
            <span>Traffic rules</span>
          </button>
          <button
            onClick={() => {
              setView('analysis');
              setPanel(null);
            }}
            className={view === 'analysis' && !panel ? 'selected' : ''}
          >
            <BarChart3 size={17} />
            <span>Analysis</span>
          </button>
          <button
            onClick={() => {
              setView('compare');
              setPanel(null);
            }}
            className={view === 'compare' && !panel ? 'selected' : ''}
          >
            <ArrowLeftRight size={17} />
            <span>Compare sessions</span>
          </button>
        </nav>
        <div className="sidebar-heading">
          <span>Saved sessions</span>
          <div className="sidebar-session-actions">
            {selectedSession && (
              <IconButton
                label="Manage selected session"
                onClick={() => setSessionDetails(selectedSession)}
              >
                <MoreHorizontal size={15} />
              </IconButton>
            )}
            <IconButton label="New session" onClick={() => setNewSession(true)}>
              <Plus size={16} />
            </IconButton>
          </div>
        </div>
        <div className="session-list">
          {loading ? (
            <div className="sidebar-hint">
              <LoaderCircle size={14} className="spin" /> Opening archive…
            </div>
          ) : sessions.length ? (
            [...sessions]
              .sort((a, b) => Number(!!b.pinned) - Number(!!a.pinned) || b.createdAt - a.createdAt)
              .map((s) => (
                <button
                  className={`session-item ${s.id === sessionId ? 'current' : ''}`}
                  onClick={() => {
                    setSessionId(s.id);
                    setPanel(null);
                  }}
                  key={s.id}
                >
                  {s.pinned ? <Pin size={15} /> : <FolderArchive size={15} />}
                  <span>
                    <strong>{s.name}</strong>
                    <small>
                      {new Date(s.createdAt).toLocaleDateString([], {
                        month: 'short',
                        day: 'numeric',
                      })}{' '}
                      <span>·</span> {s.exchangeCount} requests
                    </small>
                  </span>
                  {proxy.running && proxy.sessionId === s.id && <span className="live-dot" />}
                </button>
              ))
          ) : (
            <div className="sidebar-hint">
              Your captures will live here.
              <br />
              Saved locally, automatically.
            </div>
          )}
        </div>
        <button
          className="sidebar-import"
          onClick={() => importRef.current?.click()}
          disabled={!native || !!busy}
        >
          <ArrowUpFromLine size={15} /> Import session or HAR
        </button>
        <div className="sidebar-bottom">
          <div className="sidebar-heading">
            <span>Tools</span>
          </div>
          <button
            onClick={() => setPanel('activity')}
            className={panel === 'activity' ? 'selected' : ''}
          >
            <Clock3 size={17} />
            <span>Activity trail</span>
          </button>
          <button
            onClick={() => setPanel('diagnostics')}
            className={panel === 'diagnostics' ? 'selected' : ''}
          >
            <Stethoscope size={17} />
            <span>Diagnostics</span>
          </button>
          <button onClick={() => setPanel('mcp')} className={panel === 'mcp' ? 'selected' : ''}>
            <Code2 size={17} />
            <span>MCP server</span>
            <span className={`tiny-dot ${mcp.sessionIds.length ? 'green' : ''}`} />
          </button>
          <button
            onClick={() => setPanel('connection')}
            className={panel === 'connection' ? 'selected' : ''}
          >
            <Settings2 size={17} />
            <span>Connection settings</span>
          </button>
          <div className="local-note">
            <HardDrive size={13} />
            <span>Local by design. Open source.</span>
          </div>
        </div>
      </aside>
      <main className="workspace">
        <header className="topbar">
          <div className="breadcrumb">
            <Layers3 size={16} />
            <span>Workspace</span>
            <ChevronRight size={13} />
            <strong>{selectedSession?.name || 'New capture'}</strong>
            {selectedSession && (
              <IconButton
                label="Session details"
                onClick={() => setSessionDetails(selectedSession)}
              >
                <MoreHorizontal size={15} />
              </IconButton>
            )}
            <span className="autosave">
              <ShieldCheck size={12} /> {selectedSession ? 'Autosaved' : 'Local archive'}
            </span>
          </div>
          <div className="topbar-actions">
            <div className="export-control">
              <button
                className="button small plain"
                aria-expanded={exportOpen}
                disabled={!sessionId || !!busy}
                onClick={() => setExportOpen((v) => !v)}
              >
                <Download size={14} /> Export <ChevronDown size={11} />
              </button>
              {exportOpen && (
                <div className="export-menu">
                  <div className="export-privacy">
                    <strong>Capture export content</strong>
                    <label>
                      <input
                        type="radio"
                        name="export-privacy"
                        checked={exportRedacted}
                        onChange={() => setExportRedacted(true)}
                      />{' '}
                      Sanitized
                    </label>
                    <label>
                      <input
                        type="radio"
                        name="export-privacy"
                        checked={!exportRedacted}
                        onChange={() => setExportRedacted(false)}
                      />{' '}
                      Raw capture
                    </label>
                    <p>
                      {exportRedacted
                        ? 'Masks recognized credential fields. Review custom secrets before sharing.'
                        : 'Includes captured headers and bodies, including credentials.'}
                    </p>
                  </div>
                  <button
                    onClick={() => {
                      setExportOpen(false);
                      void exportFile('har');
                    }}
                  >
                    <Download size={14} />
                    <span>
                      HAR archive<small>Compatible with other HTTP tools</small>
                    </span>
                  </button>
                  <button
                    onClick={() => {
                      setExportOpen(false);
                      void exportFile('scope');
                    }}
                  >
                    <FolderArchive size={14} />
                    <span>
                      Scope session<small>Includes originals and edits</small>
                    </span>
                  </button>
                  <button
                    onClick={() => {
                      setExportOpen(false);
                      void exportFile('openapi');
                    }}
                  >
                    <Code2 size={14} />
                    <span>
                      Observed OpenAPI<small>Inferred from this session's evidence</small>
                    </span>
                  </button>
                </div>
              )}
            </div>
            <button className="button small plain" onClick={() => setPanel('mcp')}>
              <Code2 size={15} />
              <span>MCP</span>
            </button>
          </div>
        </header>
        {!native && (
          <div className="preview-notice">
            <Info size={14} />
            <span>
              Browser preview. Open the desktop app to capture traffic, connect devices and use MCP.
            </span>
            <span className="native-label">Tauri + Rust</span>
          </div>
        )}
        {error && (
          <div className="error-banner" role="alert">
            <Info size={15} />
            <span>{error}</span>
            <IconButton
              label="Copy error details"
              onClick={() => void navigator.clipboard.writeText(error)}
            >
              <Copy size={14} />
            </IconButton>
            <IconButton label="Dismiss error" onClick={() => setError('')}>
              <X size={14} />
            </IconButton>
          </div>
        )}
        {pending.length > 0 && (
          <div
            className="pending-banner"
            role="region"
            aria-label="Paused traffic"
            aria-live="polite"
          >
            <Pause size={15} />
            <span>
              <strong>
                {pending.length} {pending.length === 1 ? 'message is' : 'messages are'} paused
              </strong>{' '}
              at a breakpoint.
            </span>
            <button className="button small" onClick={() => setActivePending(pending[0])}>
              Inspect & edit <ArrowRight size={13} />
            </button>
          </div>
        )}
        {panel === 'rules' ? (
          <RulesWorkspace native={native} />
        ) : panel === 'activity' ? (
          <ActivityWorkspace native={native} />
        ) : panel === 'diagnostics' ? (
          <DiagnosticsWorkspace
            native={native}
            proxy={proxy}
            version={boot?.version}
            onOpenDevice={() => setPanel('device')}
          />
        ) : panel === 'device' ? (
          <DevicePanel
            device={device}
            proxy={proxy}
            portDraft={proxyPortDraft}
            busy={busy}
            native={native}
            onClose={() => setPanel(null)}
            perform={perform}
            ensureSession={ensureSession}
            onProxy={applyProxyControl}
            onDevice={setDevice}
            onNotice={setNotice}
          />
        ) : panel === 'mcp' ? (
          <McpPanel
            boot={boot}
            mcp={mcp}
            session={selectedSession}
            busy={busy}
            native={native}
            onClose={() => setPanel(null)}
            onUpdate={runMcp}
          />
        ) : panel === 'connection' ? (
          <ConnectionPanel
            proxy={proxy}
            portDraft={proxyPortDraft}
            onPortChange={setProxyPortDraft}
            boot={boot}
            native={native}
            busy={busy}
            onClose={() => setPanel(null)}
            onToggle={toggleCapture}
          />
        ) : (
          <>
            <div className="workspace-heading">
              <div>
                <div className="section-context">
                  <span className={`connection-pill ${proxy.running ? 'is-live' : ''}`}>
                    <span />
                    {proxy.running ? `Listening on :${proxy.port}` : 'Capture ready'}
                  </span>
                  {device.running && (
                    <span className="context-text">
                      {device.mode === 'proxy' ? 'Device-wide proxy' : 'Frida attached'}
                    </span>
                  )}
                </div>
                <h1>
                  {view === 'traffic'
                    ? 'Traffic inspector'
                    : view === 'analysis'
                      ? 'Understand the conversation.'
                      : 'Find what changed.'}
                </h1>
                <p>
                  {view === 'traffic'
                    ? 'Every request. Every response. Yours to inspect.'
                    : view === 'analysis'
                      ? 'Explore patterns, slow endpoints and errors in your captured traffic.'
                      : 'Compare the endpoints and behavior of two saved sessions.'}
                </p>
              </div>
              <div className="heading-actions">
                <IconButton
                  label="Generate real test traffic through the proxy"
                  disabled={!native || !!busy}
                  onClick={() => void demo()}
                >
                  {busy === 'demo' ? (
                    <LoaderCircle size={16} className="spin" />
                  ) : (
                    <Zap size={16} />
                  )}
                </IconButton>
                <button className="button" onClick={() => setNewSession(true)} disabled={!!busy}>
                  <Plus size={15} /> New session
                </button>
                <button
                  className={`button primary ${proxy.running ? 'is-capturing' : ''}`}
                  onClick={() => void toggleCapture()}
                  disabled={!native || !!busy}
                >
                  {busy === 'capture' ? (
                    <LoaderCircle size={15} className="spin" />
                  ) : proxy.running ? (
                    <Square size={12} fill="currentColor" />
                  ) : (
                    <Radio size={16} />
                  )}{' '}
                  {proxy.running ? 'Stop capture' : 'Start capture'}
                </button>
              </div>
            </div>
            {view === 'traffic' ? (
              <section className="traffic-workspace">
                <div className="traffic-toolbar">
                  <label className="search-field">
                    <Search size={15} />
                    <input
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                      placeholder="Search text or filter: method=POST status>=400"
                      aria-label="Filter captured traffic"
                    />
                    {query && (
                      <IconButton label="Clear search" onClick={() => setQuery('')}>
                        <X size={12} />
                      </IconButton>
                    )}
                    <kbd>⌕</kbd>
                  </label>
                  <select
                    className="status-select"
                    value={statusFilter}
                    onChange={(e) => setStatusFilter(e.target.value)}
                    aria-label="Filter status"
                  >
                    <option value="all">All traffic</option>
                    <option value="2xx">Successful</option>
                    <option value="errors">Errors</option>
                    <option value="edited">Edited</option>
                  </select>
                  <span className="toolbar-divider" />
                  <div className="breakpoints">
                    <span>Breakpoints</span>
                    <button
                      className={`breakpoint-button ${proxy.requestBreakpoint ? 'enabled' : ''}`}
                      aria-pressed={proxy.requestBreakpoint}
                      disabled={!native || !!busy}
                      title="Pause outbound requests for editing"
                      onClick={() =>
                        void perform('breakpoint', async () => {
                          await api.set_breakpoints(
                            !proxy.requestBreakpoint,
                            proxy.responseBreakpoint,
                          );
                          applyProxyControl(await api.proxy_status());
                        })
                      }
                    >
                      <span /> Request
                    </button>
                    <button
                      className={`breakpoint-button ${proxy.responseBreakpoint ? 'enabled' : ''}`}
                      aria-pressed={proxy.responseBreakpoint}
                      disabled={!native || !!busy}
                      title="Pause inbound responses for editing"
                      onClick={() =>
                        void perform('breakpoint', async () => {
                          await api.set_breakpoints(
                            proxy.requestBreakpoint,
                            !proxy.responseBreakpoint,
                          );
                          applyProxyControl(await api.proxy_status());
                        })
                      }
                    >
                      <span /> Response
                    </button>
                  </div>
                </div>
                <TrafficSearchHelp error={searchError} onSelect={setQuery} />
                <div className={`traffic-split ${selected ? 'has-selection' : ''}`}>
                  <div className="traffic-table-wrap">
                    <table className="traffic-table">
                      <thead>
                        <tr>
                          <th className="method-col">Method</th>
                          <th className="url-col">
                            Request <ArrowDown size={10} />
                          </th>
                          <th className="status-col">Status</th>
                          <th className="time-col">Time</th>
                          <th className="size-col">Size</th>
                        </tr>
                      </thead>
                      <tbody>
                        {filtered.map((row) => {
                          const url = safeUrl(row.url);
                          const isPaused = pending.some((p) => p.exchangeId === row.id);
                          return (
                            <tr
                              key={row.id}
                              className={selected?.id === row.id ? 'row-selected' : ''}
                              onClick={() => void selectExchange(row)}
                              tabIndex={0}
                              onKeyDown={(e) => {
                                if (e.key === 'Enter') void selectExchange(row);
                              }}
                              aria-label={`${row.method} ${row.url}`}
                            >
                              <td>
                                <span className={`method method-${row.method.toLowerCase()}`}>
                                  {row.method}
                                </span>
                              </td>
                              <td className="request-cell">
                                <strong>{url.path || '/'}</strong>
                                <span>
                                  {url.host}
                                  {row.edited && <em>edited</em>}
                                </span>
                              </td>
                              <td>
                                {isPaused ? (
                                  <span className="status-code status-paused">Paused</span>
                                ) : (
                                  <Status
                                    code={row.status}
                                    error={row.error}
                                    completed={row.completed}
                                  />
                                )}
                              </td>
                              <td className="numeric">{ms(row.durationMs)}</td>
                              <td className="numeric">{bytes(row.responseBytes)}</td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                    {!filtered.length && (
                      <Empty
                        icon={
                          query || statusFilter !== 'all' ? (
                            <Search size={29} />
                          ) : (
                            <Activity size={31} />
                          )
                        }
                        title={
                          query || statusFilter !== 'all'
                            ? 'No matching traffic'
                            : 'A clear view starts here.'
                        }
                        action={
                          query || statusFilter !== 'all' ? (
                            <button
                              className="button"
                              onClick={() => {
                                setQuery('');
                                setStatusFilter('all');
                              }}
                            >
                              Clear filters
                            </button>
                          ) : (
                            <div className="empty-actions">
                              <button className="button primary" onClick={() => setPanel('device')}>
                                <Smartphone size={15} /> Connect a device
                              </button>
                              <button
                                className="text-button demo-button"
                                onClick={() => void demo()}
                                disabled={!native || !!busy}
                              >
                                {busy === 'demo' ? (
                                  <LoaderCircle size={13} className="spin" />
                                ) : (
                                  <Zap size={13} />
                                )}{' '}
                                Generate test traffic
                              </button>
                            </div>
                          )
                        }
                      >
                        {query || statusFilter !== 'all' ? (
                          'Try a different search or clear your filters.'
                        ) : (
                          <>
                            Connect an Android app or point your HTTP client at the local proxy.
                            Captured traffic is saved as it arrives.
                          </>
                        )}
                      </Empty>
                    )}
                    {exchanges.length >= limit && (
                      <button className="load-more" onClick={() => setLimit((n) => n + 250)}>
                        Load 250 more requests
                      </button>
                    )}
                  </div>
                  <RequestInspector
                    selected={selected}
                    onReplay={() => {
                      if (selected && !replayingId) setReplayDraft(selected);
                    }}
                    busy={!!busy || !!replayingId}
                  />
                </div>
                <footer className="traffic-footer">
                  <span>
                    <span className={`tiny-dot ${proxy.running ? 'green' : ''}`} />
                    {proxy.running ? 'Capturing' : 'Idle'}
                  </span>
                  <span>
                    {filtered.length} shown{count > requestCount ? ` of ${count}` : ''}
                  </span>
                  <span>{bytes(totalSize)} loaded</span>
                  <span className="footer-right">
                    <HardDrive size={12} /> Saved on this computer
                  </span>
                </footer>
              </section>
            ) : view === 'analysis' ? (
              <AnalysisView
                value={analysis}
                session={selectedSession}
                onTraffic={() => setView('traffic')}
                onInspect={(id) =>
                  void perform('inspect', async () => {
                    setSelected(await api.get_exchange(id));
                    setView('traffic');
                  })
                }
              />
            ) : (
              <CompareView
                sessions={sessions}
                selected={sessionId}
                right={compareId}
                onRight={(id) => {
                  setCompareId(id);
                  setComparison(null);
                }}
                value={comparison}
                busy={!!busy}
                onCompare={() =>
                  void perform('compare', async () =>
                    setComparison(await api.compare_sessions(sessionId, compareId)),
                  )
                }
              />
            )}
          </>
        )}
        <footer className="app-status">
          <span className="status-wordmark">
            <span className="tiny-scope" />
            Scope {boot?.version || '0.1.0'}
          </span>
          <span>HTTP, in plain sight.</span>
          <span className="status-right">
            {native ? 'Native engine' : 'Interface preview'}
            <span className="engine-dot" />
          </span>
        </footer>
      </main>
      <input
        ref={importRef}
        type="file"
        accept=".har,.json"
        hidden
        onChange={(e) => void importFile(e)}
      />
      {notice && (
        <div className="toast" role="status">
          <Check size={16} />
          {notice}
          <IconButton label="Dismiss notification" onClick={() => setNotice('')}>
            <X size={13} />
          </IconButton>
        </div>
      )}
      {sessionDetails && (
        <SessionDetails
          session={sessionDetails}
          active={proxy.running && proxy.sessionId === sessionDetails.id}
          onClose={() => setSessionDetails(null)}
          onUpdated={(session) => {
            setSessions((all) => all.map((s) => (s.id === session.id ? session : s)));
            setNotice('Session details saved.');
          }}
          onDeleted={(id) => {
            setSessions((all) => all.filter((s) => s.id !== id));
            if (sessionId === id) setSessionId(sessions.find((s) => s.id !== id)?.id || '');
            setMcp((m) => ({ ...m, sessionIds: m.sessionIds.filter((s) => s !== id) }));
            setNotice('Session deleted.');
          }}
        />
      )}
      {replayDraft && (
        <ReplayComposer
          exchange={replayDraft}
          onClose={() => setReplayDraft(null)}
          onSend={(edit) => {
            const id = replayDraft.id;
            setReplayingId(id);
            setReplayDraft(null);
            setError('');
            void api
              .replay(id, edit)
              .then(async (replayed) => {
                await refreshSessions();
                if (sessionIdRef.current === replayed.sessionId) setSelected(replayed);
                setNotice('Replay completed. Current traffic rules were applied.');
              })
              .catch((e) => setError(errorText(e)))
              .finally(() => setReplayingId(''));
          }}
        />
      )}
      {newSession && (
        <NewSessionModal
          busy={!!busy}
          onClose={() => setNewSession(false)}
          onCreate={(name) =>
            void perform('create', async () => {
              const s = await api.create_session(name);
              await refreshSessions();
              setSessionId(s.id);
              setNewSession(false);
              setNotice('Session created. Captures will be saved automatically.');
            })
          }
          native={native}
        />
      )}
      {activePending && (
        <BreakpointModal
          item={activePending}
          onClose={() => setActivePending(null)}
          onResolve={async (edit: EditMessage) => {
            // Resolving a paused transfer must not wait behind demo generation or
            // any other operation that may itself be waiting for that transfer.
            if (resolvingPending.current)
              throw new Error('Another paused message is being continued. Try again in a moment.');
            const item = activePending;
            resolvingPending.current = item.id;
            try {
              await api.resolve_breakpoint(item.id, edit);
              pendingEpoch.current += 1;
              setActivePending((current) => (current?.id === item.id ? null : current));
              setPending((items) => items.filter((pendingItem) => pendingItem.id !== item.id));
              setNotice(
                edit.action === 'abort'
                  ? `${item.stage === 'request' ? 'Request' : 'Response'} aborted.`
                  : Object.keys(edit).length === 1
                    ? 'Message continued unchanged.'
                    : 'Message sent with your changes.',
              );
              // Resolving succeeded. Refresh errors must not offer to send the
              // same transfer again; report them separately in the workspace.
              void refreshSessions().catch((e) => setError(errorText(e)));
              if (selected?.id === item.exchangeId)
                void api
                  .get_exchange(item.exchangeId)
                  .then((full) =>
                    setSelected((current) => (current?.id === full.id ? full : current)),
                  )
                  .catch((e) => setError(errorText(e)));
            } finally {
              resolvingPending.current = null;
            }
          }}
        />
      )}
    </div>
  );
}

function RequestInspector({
  selected,
  onReplay,
  busy,
}: {
  selected: Exchange | null;
  onReplay: () => void;
  busy: boolean;
}) {
  const [side, setSide] = useState<'request' | 'response'>('response');
  const [tab, setTab] = useState<'body' | 'headers' | 'timing' | 'original'>('body');
  useEffect(() => {
    setTab('body');
  }, [selected?.id]);
  if (!selected)
    return (
      <aside className="request-inspector inspector-empty">
        <div className="inspector-label">
          <span>Inspector</span>
          <MoreHorizontal size={17} />
        </div>
        <div className="inspector-placeholder">
          <div className="inspection-graphic">
            <span />
            <span />
            <span />
            <Search size={24} />
          </div>
          <h3>The details make the difference.</h3>
          <p>Select a request to inspect its headers, body and timing.</p>
          <div className="inspector-shortcuts">
            <span>
              <ArrowDownToLine size={13} /> Request
            </span>
            <span>
              <ArrowUpFromLine size={13} /> Response
            </span>
            <span>
              <RotateCcw size={13} /> Replay
            </span>
          </div>
        </div>
        <div className="inspector-bottom">
          <ShieldCheck size={16} />
          <span>Your traffic stays on your computer.</span>
        </div>
      </aside>
    );
  const url = safeUrl(selected.url),
    headers = side === 'request' ? selected.requestHeaders : selected.responseHeaders,
    body = side === 'request' ? selected.requestBody : selected.responseBody,
    binary = side === 'request' ? selected.requestBodyBase64 : selected.responseBodyBase64;
  return (
    <aside className="request-inspector">
      <div className="inspector-label">
        <span>Inspector</span>
        <span className="exchange-id">{selected.id.slice(0, 8)}</span>
      </div>
      <div className="request-summary">
        <div>
          <span className={`method method-${selected.method.toLowerCase()}`}>
            {selected.method}
          </span>
          <Status code={selected.status} error={selected.error} completed={selected.completed} />
          <span className="summary-time">{ms(selected.durationMs)}</span>
        </div>
        <h3 title={selected.url}>{url.host}</h3>
        <p title={selected.url}>{url.path}</p>
        <div className="request-summary-actions">
          <button className="button small" onClick={onReplay} disabled={busy}>
            <RotateCcw size={13} /> Compose replay
          </button>
          <IconButton
            label="Copy request URL"
            onClick={() => void navigator.clipboard.writeText(selected.url)}
          >
            <Copy size={13} />
          </IconButton>
        </div>
      </div>
      {selected.completed !== false && selected.error && (
        <div className="request-error">{selected.error}</div>
      )}
      {(selected.provenance === 'mock' ||
        selected.provenance === 'blocked' ||
        !!selected.appliedRules?.length) && (
        <div className="exchange-rule-history">
          <strong>
            {selected.provenance === 'mock'
              ? 'Mock response'
              : selected.provenance === 'blocked'
                ? 'Blocked by a rule'
                : 'Traffic rules applied'}
          </strong>
          {selected.appliedRules?.map((rule, i) => (
            <span key={`${rule.ruleId}-${i}`}>
              <GitBranch size={11} />
              {rule.name}
              <small>{rule.stage}</small>
            </span>
          ))}
        </div>
      )}
      <div className="message-toggle">
        <button className={side === 'request' ? 'active' : ''} onClick={() => setSide('request')}>
          <ArrowUpFromLine size={13} /> Request
        </button>
        <button className={side === 'response' ? 'active' : ''} onClick={() => setSide('response')}>
          <ArrowDownToLine size={13} /> Response
        </button>
      </div>
      <div className="detail-tabs">
        <button className={tab === 'body' ? 'active' : ''} onClick={() => setTab('body')}>
          Body
        </button>
        <button className={tab === 'headers' ? 'active' : ''} onClick={() => setTab('headers')}>
          Headers <span>{headers.length}</span>
        </button>
        <button className={tab === 'timing' ? 'active' : ''} onClick={() => setTab('timing')}>
          Timing
        </button>
        {selected.edited && (
          <button className={tab === 'original' ? 'active' : ''} onClick={() => setTab('original')}>
            Original
          </button>
        )}
      </div>
      <div className="inspector-content">
        {tab === 'body' ? (
          <Body body={body} binary={binary} />
        ) : tab === 'headers' ? (
          <Headers headers={headers} />
        ) : tab === 'original' ? (
          <pre className="body-code">
            {JSON.stringify(
              side === 'request' ? selected.originalRequest : selected.originalResponse,
              null,
              2,
            ) || 'No original recorded for this message.'}
          </pre>
        ) : (
          <div className="timing-details">
            <div>
              <span>Started</span>
              <strong>{time(selected.startedAt)}</strong>
            </div>
            <div>
              <span>Total duration</span>
              <strong>{ms(selected.durationMs)}</strong>
            </div>
            <div className="duration-track">
              <span style={{ width: '100%' }} />
            </div>
            <div>
              <span>Request size</span>
              <strong>{bytes(selected.requestBytes)}</strong>
            </div>
            <div>
              <span>Response size</span>
              <strong>{bytes(selected.responseBytes)}</strong>
            </div>
            <p>Duration covers the proxy's outbound HTTP exchange.</p>
          </div>
        )}
      </div>
    </aside>
  );
}

function NewSessionModal({
  busy,
  onClose,
  onCreate,
  native,
}: {
  busy: boolean;
  onClose: () => void;
  onCreate: (name: string) => void;
  native: boolean;
}) {
  const [name, setName] = useState('');
  return (
    <div
      className="modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <form
        className="modal"
        onSubmit={(e: FormEvent) => {
          e.preventDefault();
          if (name.trim()) onCreate(name.trim());
        }}
      >
        <div className="modal-heading">
          <FolderArchive size={22} />
          <IconButton label="Close new session" onClick={onClose}>
            <X size={18} />
          </IconButton>
        </div>
        <h2>Give this capture a home.</h2>
        <p>Requests and responses are saved locally as you capture them.</p>
        <label className="field-label" htmlFor="session-name">
          Session name
        </label>
        <input
          autoFocus
          id="session-name"
          className="text-input"
          placeholder="e.g. Android login investigation"
          value={name}
          onChange={(e) => setName(e.target.value)}
          maxLength={120}
        />
        {!native && (
          <p className="inline-warning">Creating sessions requires the Scope desktop app.</p>
        )}
        <div className="modal-actions">
          <button type="button" className="button" onClick={onClose}>
            Cancel
          </button>
          <button className="button primary" disabled={!name.trim() || busy || !native}>
            {busy ? <LoaderCircle size={14} className="spin" /> : <Plus size={14} />} Create session
          </button>
        </div>
      </form>
    </div>
  );
}

function PanelHeading({
  icon,
  title,
  subtitle,
  onClose,
}: {
  icon: ReactNode;
  title: string;
  subtitle: string;
  onClose: () => void;
}) {
  return (
    <div className="panel-heading">
      <div className="panel-icon">{icon}</div>
      <div>
        <h1>{title}</h1>
        <p>{subtitle}</p>
      </div>
      <IconButton label="Close panel" onClick={onClose}>
        <X size={18} />
      </IconButton>
    </div>
  );
}
function DevicePanel({
  device,
  proxy,
  portDraft,
  busy,
  native,
  onClose,
  perform,
  ensureSession,
  onProxy,
  onDevice,
  onNotice,
}: {
  device: DeviceStatus;
  proxy: ProxyStatus;
  portDraft: string;
  busy: string;
  native: boolean;
  onClose: () => void;
  perform: (label: string, task: () => Promise<void>) => Promise<void>;
  ensureSession: () => Promise<string>;
  onProxy: (proxy: ProxyStatus) => void;
  onDevice: (status: DeviceStatus) => void;
  onNotice: (text: string) => void;
}) {
  const [devices, setDevices] = useState<Device[]>([]),
    [apps, setApps] = useState<AndroidApp[]>([]),
    [serial, setSerial] = useState(device.serial || ''),
    [app, setApp] = useState(device.package || ''),
    [mode, setMode] = useState('proxy'),
    [filter, setFilter] = useState(''),
    [profiles, setProfiles] = useState<string[]>(['certificate', 'unpinning', 'proxy']),
    [scan, setScan] = useState(false),
    [appsLoading, setAppsLoading] = useState(false),
    [scanError, setScanError] = useState('');
  const refresh = useCallback(async () => {
    if (!native) return;
    setScan(true);
    try {
      const list = await api.list_devices();
      setDevices(list);
      setSerial((s) => s || list.find((d) => d.state === 'device')?.serial || '');
    } finally {
      setScan(false);
    }
  }, [native]);
  useEffect(() => {
    void perform('devices', refresh);
  }, []);
  useEffect(() => {
    setApps([]);
    if (!serial) return;
    let alive = true;
    setAppsLoading(true);
    setScanError('');
    api
      .list_apps(serial)
      .then((value) => {
        if (alive) setApps(value);
      })
      .catch((e) => {
        if (alive) setScanError(errorText(e));
      })
      .finally(() => {
        if (alive) setAppsLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [serial]);
  const available = apps.filter((a) =>
    `${a.name} ${a.package}`.toLowerCase().includes(filter.toLowerCase()),
  );
  const start = () =>
    void perform('device', async () => {
      let p = proxy;
      if (!p.running) {
        const port = validatedListenerPort(portDraft);
        p = await api.start_proxy(await ensureSession(), port);
        onProxy(p);
      }
      onDevice(await api.start_device(serial, app, mode, profiles));
      onNotice('Device interception started. Check the log for setup details.');
    });
  return (
    <section className="settings-page">
      <PanelHeading
        icon={<Smartphone size={24} />}
        title="Connect your Android app"
        subtitle="Choose a device, select an app and start a capture."
        onClose={onClose}
      />
      <div className="settings-grid">
        <div className="settings-main">
          <div className="settings-section">
            <div className="section-title">
              <h2>Device</h2>
              <button
                className="text-button"
                onClick={() => void perform('devices', refresh)}
                disabled={!native || !!busy}
              >
                <RefreshCw size={13} className={scan ? 'spin' : ''} /> Refresh
              </button>
            </div>
            <select
              className="text-input"
              aria-label="Android device"
              value={serial}
              onChange={(e) => {
                setSerial(e.target.value);
                setApp('');
              }}
            >
              <option value="">
                {scan ? 'Looking for devices…' : 'Select a connected device'}
              </option>
              {devices.map((d) => (
                <option key={d.serial} value={d.serial} disabled={d.state !== 'device'}>
                  {d.model || d.serial} · {d.serial} · {d.state}
                  {d.rooted ? ' · rooted' : ''}
                </option>
              ))}
            </select>
            {!devices.length && (
              <p className="help-copy">
                Connect a phone with USB debugging enabled, or start an Android emulator. Authorize
                this computer on the device.
              </p>
            )}
          </div>
          <div className="settings-section">
            <div className="section-title">
              <h2>Application</h2>
              <span>{apps.length} installed</span>
            </div>
            <label className="search-field app-search">
              <Search size={14} />
              <input
                placeholder="Find an app or package…"
                aria-label="Find installed app"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
              />
            </label>
            <div className="app-picker">
              {available.length ? (
                available.map((a) => (
                  <button
                    className={app === a.package ? 'chosen' : ''}
                    onClick={() => setApp(a.package)}
                    key={a.package}
                  >
                    <span className="android-icon">
                      <Smartphone size={17} />
                    </span>
                    <span>
                      <strong>{a.name || a.package.split('.').pop()}</strong>
                      <small>{a.package}</small>
                    </span>
                    {app === a.package ? (
                      <Check size={16} />
                    ) : a.running ? (
                      <span className="tiny-dot green" />
                    ) : (
                      <Circle size={15} />
                    )}
                  </button>
                ))
              ) : (
                <div className="picker-empty">
                  {appsLoading ? (
                    <>
                      <LoaderCircle size={16} className="spin" /> Loading applications…
                    </>
                  ) : serial ? (
                    'No apps match your search.'
                  ) : (
                    'Select a device to see installed apps.'
                  )}
                </div>
              )}
            </div>
            {scanError && <p className="inline-warning">{scanError}</p>}
          </div>
          <div className="settings-section">
            <h2>Interception mode</h2>
            <div className="mode-options">
              <button className={mode === 'proxy' ? 'chosen' : ''} onClick={() => setMode('proxy')}>
                <Globe2 size={19} />
                <strong>System proxy</strong>
                <small>ADB + local HTTP proxy</small>
                {mode === 'proxy' && <Check size={14} />}
              </button>
              <button className={mode === 'frida' ? 'chosen' : ''} onClick={() => setMode('frida')}>
                <Zap size={19} />
                <strong>Frida</strong>
                <small>Optional instrumentation</small>
                {mode === 'frida' && <Check size={14} />}
              </button>
            </div>
            {mode === 'proxy' ? (
              <div className="info-callout">
                <Info size={16} />
                <p>
                  <strong>Device-wide capture.</strong> Android's proxy setting applies to the whole
                  device. Other apps may appear in this session; choosing an app launches it and
                  does not isolate its traffic.
                </p>
              </div>
            ) : (
              <div className="profile-options">
                <p className="help-copy">
                  Select the bundled hook profiles for an app you own or are authorized to test.
                  Frida support depends on Android and application versions.
                </p>
                <div className="info-callout">
                  <Info size={15} />
                  <p>
                    Frida needs an optional local runtime. If it is missing, Scope shows the setup
                    command before changing your device. Runtime installation is a separate action.
                  </p>
                </div>
                {[
                  ['certificate', 'Trust the Scope CA'],
                  ['proxy', 'Proxy configuration'],
                  ['unpinning', 'TLS certificate unpinning'],
                  ['root', 'Root detection hooks'],
                ].map(([id, label]) => (
                  <label key={id}>
                    <input
                      type="checkbox"
                      checked={profiles.includes(id)}
                      onChange={(e) =>
                        setProfiles((p) =>
                          e.target.checked ? [...p, id] : p.filter((x) => x !== id),
                        )
                      }
                    />
                    {label}
                  </label>
                ))}
              </div>
            )}
          </div>
          <div className="panel-actions">
            {device.serial ? (
              <button
                className="button danger"
                disabled={!!busy}
                onClick={() =>
                  void perform('device-stop', async () => {
                    onDevice(await api.stop_device());
                    onNotice('Device settings restored.');
                  })
                }
              >
                <Unplug size={15} />{' '}
                {device.running ? 'Stop interception' : 'Restore device settings'}
              </button>
            ) : (
              <button
                className="button primary"
                disabled={!native || !serial || !app || !!busy}
                onClick={start}
              >
                {busy === 'device' ? (
                  <LoaderCircle size={15} className="spin" />
                ) : (
                  <Radio size={15} />
                )}{' '}
                Start interception
              </button>
            )}
            <span>Device settings change only when you start.</span>
          </div>
        </div>
        <aside className="settings-aside">
          <h3>
            <Terminal size={16} /> Device log
          </h3>
          <div className="device-log">
            {device.logs?.length ? (
              device.logs.map((line, i) => <p key={i}>{line}</p>)
            ) : (
              <p>
                No device activity yet.
                <br />
                <br />
                Setup progress and instrumentation output will appear here.
              </p>
            )}
          </div>
          {device.message && (
            <div className="info-callout">
              <Info size={15} />
              <p>{device.message}</p>
            </div>
          )}
          <div className="aside-note">
            <ShieldCheck size={20} />
            <h3>Built for your lab.</h3>
            <p>
              Capture applications you own or have permission to debug. Sessions stay on your
              computer.
            </p>
          </div>
        </aside>
      </div>
    </section>
  );
}

function McpPanel({
  boot,
  mcp,
  session,
  busy,
  native,
  onClose,
  onUpdate,
}: {
  boot: Bootstrap | null;
  mcp: McpStatus;
  session?: Session;
  busy: string;
  native: boolean;
  onClose: () => void;
  onUpdate: (ids: string[], writes: boolean, rules?: boolean, diagnostics?: boolean) => void;
}) {
  const [copied, setCopied] = useState(false);
  const config = boot?.mcpConfig
    ? typeof boot.mcpConfig === 'string'
      ? boot.mcpConfig
      : JSON.stringify(boot.mcpConfig, null, 2)
    : null;
  const shared = !!session && mcp.sessionIds.includes(session.id);
  return (
    <section className="settings-page">
      <PanelHeading
        icon={<Code2 size={24} />}
        title="A debugger your agent can use."
        subtitle="Connect a compatible MCP client to your local Scope engine."
        onClose={onClose}
      />
      <div className="settings-grid">
        <div className="settings-main">
          <div className="settings-section">
            <div className="section-title">
              <h2>Client configuration</h2>
              <span className="mini-badge">stdio</span>
            </div>
            <p className="help-copy">
              Add this server configuration to your MCP client. Keep Scope open while the agent
              inspects your captures.
            </p>
            <div className="config-block">
              <div className="code-toolbar">
                <span>MCP server configuration</span>
                <button
                  className="text-button"
                  disabled={!config}
                  onClick={() => {
                    if (config)
                      void navigator.clipboard.writeText(config).then(() => {
                        setCopied(true);
                        window.setTimeout(() => setCopied(false), 2000);
                      });
                  }}
                >
                  {copied ? <Check size={13} /> : <Copy size={13} />}{' '}
                  {copied ? 'Copied' : 'Copy config'}
                </button>
              </div>
              <pre>
                {config ||
                  'Open the Scope desktop app to get the configuration\nwith the actual path to your MCP executable.'}
              </pre>
            </div>
          </div>
          <div className="settings-section">
            <h2>Session access</h2>
            <p className="help-copy">
              Only explicitly shared sessions are visible to the agent. Credentials are redacted in
              MCP reads by default.
            </p>
            <div className="permission-row">
              <div>
                <strong>{session?.name || 'No session selected'}</strong>
                <span>
                  {shared ? 'Shared with MCP clients' : 'Private · not visible to agents'}
                </span>
              </div>
              <button
                role="switch"
                aria-checked={shared}
                aria-label="Share selected session with MCP"
                className={`switch ${shared ? 'on' : ''}`}
                disabled={!native || !session || !!busy}
                onClick={() =>
                  session &&
                  onUpdate(
                    shared
                      ? mcp.sessionIds.filter((id) => id !== session.id)
                      : [...mcp.sessionIds, session.id],
                    mcp.allowWrites,
                  )
                }
              >
                <span />
              </button>
            </div>
            <div className="permission-row">
              <div>
                <strong>Allow traffic changes</strong>
                <span>Agents can send captured requests again.</span>
              </div>
              <button
                role="switch"
                aria-checked={mcp.allowWrites}
                aria-label="Allow MCP traffic changes"
                className={`switch ${mcp.allowWrites ? 'on' : ''}`}
                disabled={!native || !!busy}
                onClick={() => onUpdate(mcp.sessionIds, !mcp.allowWrites)}
              >
                <span />
              </button>
            </div>
            <div className="permission-row">
              <div>
                <strong>Allow global traffic rules</strong>
                <span>
                  View rules across the workspace. Mutations also require traffic changes.
                </span>
              </div>
              <button
                role="switch"
                aria-checked={!!mcp.allowRules}
                aria-label="Allow MCP global rules"
                className={`switch ${mcp.allowRules ? 'on' : ''}`}
                disabled={!native || !!busy}
                onClick={() =>
                  onUpdate(
                    mcp.sessionIds,
                    mcp.allowWrites,
                    !mcp.allowRules,
                    !!mcp.allowDeviceDiagnostics,
                  )
                }
              >
                <span />
              </button>
            </div>
            {mcp.allowRules && (
              <div className="info-callout">
                <Info size={15} />
                <p>
                  Rules are global. A changed rule can affect every capture, including sessions that
                  are not shared with the agent.
                </p>
              </div>
            )}
            <div className="permission-row">
              <div>
                <strong>Allow device diagnostics</strong>
                <span>
                  Read-only checks expose connected device identifiers and local runtime paths.
                </span>
              </div>
              <button
                role="switch"
                aria-checked={!!mcp.allowDeviceDiagnostics}
                aria-label="Allow MCP device diagnostics"
                className={`switch ${mcp.allowDeviceDiagnostics ? 'on' : ''}`}
                disabled={!native || !!busy}
                onClick={() =>
                  onUpdate(
                    mcp.sessionIds,
                    mcp.allowWrites,
                    !!mcp.allowRules,
                    !mcp.allowDeviceDiagnostics,
                  )
                }
              >
                <span />
              </button>
            </div>
            {mcp.allowWrites && (
              <div className="info-callout">
                <Info size={15} />
                <p>
                  Write access can send real network requests. Only enable it for a trusted agent in
                  your test environment.
                </p>
              </div>
            )}
          </div>
          <div className="settings-section">
            <h2>Try asking your agent</h2>
            <div className="prompt-example">
              <span>“</span>
              <p>
                Compare these two login sessions and show me the requests that explain the failure.
              </p>
            </div>
            <div className="capability-list">
              <span>
                <Search size={14} /> Search captured traffic
              </span>
              <span>
                <BarChart3 size={14} /> Analyze endpoints
              </span>
              <span>
                <ArrowLeftRight size={14} /> Compare sessions
              </span>
              <span>
                <RotateCcw size={14} /> Replay with write access
              </span>
            </div>
          </div>
        </div>
        <aside className="settings-aside">
          <h3>
            <ShieldCheck size={16} /> Access boundary
          </h3>
          <div className="mcp-boundary">
            <span className="boundary-node">
              <HardDrive size={18} /> Your saved sessions
            </span>
            <span className="boundary-line" />
            <span className="boundary-node highlighted">
              <ShieldCheck size={18} /> Explicit session scope
            </span>
            <span className="boundary-line" />
            <span className="boundary-node">
              <Code2 size={18} /> Your MCP client
            </span>
          </div>
          <p className="help-copy">
            The MCP bridge connects to an authenticated local endpoint. Unshared sessions remain
            outside the agent's scope.
          </p>
          <div className="aside-note">
            <span className={`connection-pill ${mcp.sessionIds.length ? 'is-live' : ''}`}>
              <span />
              {mcp.sessionIds.length} shared {mcp.sessionIds.length === 1 ? 'session' : 'sessions'}
            </span>
            <p>
              {mcp.allowWrites
                ? 'Read and write access enabled.'
                : 'Read-only access. Traffic changes disabled.'}
            </p>
          </div>
        </aside>
      </div>
    </section>
  );
}

function ConnectionPanel({
  proxy,
  portDraft,
  onPortChange,
  boot,
  native,
  busy,
  onClose,
  onToggle,
}: {
  proxy: ProxyStatus;
  portDraft: string;
  onPortChange: (port: string) => void;
  boot: Bootstrap | null;
  native: boolean;
  busy: string;
  onClose: () => void;
  onToggle: () => void;
}) {
  const displayedPort = proxy.running ? String(proxy.port) : portDraft;
  return (
    <section className="settings-page">
      <PanelHeading
        icon={<Settings2 size={24} />}
        title="Your local capture engine"
        subtitle="Point an HTTP client at Scope or connect an Android device."
        onClose={onClose}
      />
      <div className="settings-grid">
        <div className="settings-main">
          <div className="settings-section">
            <h2>Proxy listener</h2>
            <p className="help-copy">
              The proxy binds to localhost. Device connections are forwarded over ADB.
            </p>
            <div className="listener-fields">
              <div>
                <label className="field-label">Host</label>
                <input className="text-input" readOnly value="127.0.0.1" aria-label="Proxy host" />
              </div>
              <div>
                <label className="field-label">Port</label>
                <input
                  className="text-input"
                  type="number"
                  min="1024"
                  max="65535"
                  value={displayedPort}
                  disabled={proxy.running || !!busy}
                  aria-label="Proxy port"
                  onChange={(e) => onPortChange(e.target.value)}
                />
              </div>
            </div>
            <div className="panel-actions">
              <button className="button primary" onClick={onToggle} disabled={!native || !!busy}>
                {proxy.running ? <Square size={12} /> : <Play size={14} />}{' '}
                {proxy.running ? 'Stop capture' : 'Start capture'}
              </button>
              <span>{proxy.running ? 'Listening for requests' : 'Not currently listening'}</span>
            </div>
          </div>
          <div className="settings-section">
            <h2>Certificate authority</h2>
            <p className="help-copy">
              Scope creates a local CA for HTTPS inspection. Trust this certificate only in devices
              and clients you use for testing.
            </p>
            <div className="path-display">
              <ShieldCheck size={16} />
              <code>{proxy.caCertPath || 'Available when the desktop engine starts.'}</code>
              <IconButton
                label="Copy certificate path"
                disabled={!proxy.caCertPath}
                onClick={() => void navigator.clipboard.writeText(proxy.caCertPath)}
              >
                <Copy size={14} />
              </IconButton>
            </div>
          </div>
          <div className="settings-section">
            <h2>Session archive</h2>
            <p className="help-copy">
              Bodies and metadata are persisted locally. The current capture limit is 16 MiB per
              body; oversized bodies are rejected explicitly.
            </p>
            <div className="path-display">
              <HardDrive size={16} />
              <code>{boot?.dataDir || 'Available in the desktop app.'}</code>
            </div>
          </div>
        </div>
        <aside className="settings-aside">
          <h3>
            <Terminal size={16} /> Connect a command-line client
          </h3>
          <p className="help-copy">With capture running, route a request through Scope:</p>
          <pre className="command-example">
            curl --proxy http://127.0.0.1:{displayedPort} \ --cacert "
            {proxy.caCertPath || '/path/to/scope-ca.pem'}" \ https://example.com
          </pre>
          <div className="aside-note">
            <Wifi size={20} />
            <h3>One engine, every view.</h3>
            <p>
              The desktop inspector and MCP clients access the same saved sessions and exchange
              identifiers.
            </p>
          </div>
        </aside>
      </div>
    </section>
  );
}

function AnalysisView({
  value,
  session,
  onTraffic,
  onInspect,
}: {
  value: Analysis | null;
  session?: Session;
  onTraffic: () => void;
  onInspect: (id: string) => void;
}) {
  if (!session || !session.exchangeCount)
    return (
      <div className="full-empty">
        <Empty
          icon={<BarChart3 size={32} />}
          title="Patterns begin with traffic."
          action={
            <button className="button" onClick={onTraffic}>
              Open traffic inspector <ArrowRight size={14} />
            </button>
          }
        >
          Capture or import a session to explore its endpoints, errors and response times.
        </Empty>
      </div>
    );
  if (!value)
    return (
      <div className="full-empty">
        <LoaderCircle size={25} className="spin" />
        <p>Analyzing saved traffic…</p>
      </div>
    );
  const insights =
    (value.insights as
      Array<{ kind: string; title: string; detail: string; exchangeIds: string[] }> | undefined) ||
    [];
  const endpoints = value.endpoints || [];
  return (
    <div className="analysis-view">
      <div className="analysis-metrics">
        <div>
          <span>Requests</span>
          <strong>{value.totalRequests ?? session.exchangeCount}</strong>
        </div>
        <div>
          <span>Average duration</span>
          <strong>{ms(value.averageDurationMs)}</strong>
        </div>
        <div>
          <span>Errors</span>
          <strong className={value.totalErrors ? 'red-text' : ''}>{value.totalErrors || 0}</strong>
        </div>
        <div>
          <span>Total traffic</span>
          <strong>{bytes(value.totalBytes)}</strong>
        </div>
      </div>
      {insights.length > 0 && (
        <div className="insight-list">
          {insights.map((i, n) => (
            <div className="insight" key={n}>
              <Info size={17} />
              <div>
                <strong>{i.title}</strong>
                <p>{i.detail}</p>
                {i.exchangeIds?.length > 0 && (
                  <button className="text-button" onClick={() => onInspect(i.exchangeIds[0])}>
                    Inspect a request <ArrowRight size={12} />
                  </button>
                )}
              </div>
              <span>{i.kind}</span>
            </div>
          ))}
        </div>
      )}
      <div className="analysis-table-heading">
        <h2>Endpoint map</h2>
        <span>{endpoints.length} observed endpoints</span>
      </div>
      <table className="analysis-table">
        <thead>
          <tr>
            <th>Endpoint</th>
            <th>Requests</th>
            <th>Errors</th>
            <th>Average</th>
            <th>Volume</th>
          </tr>
        </thead>
        <tbody>
          {endpoints.map((e, n) => (
            <tr key={n}>
              <td>
                <span className={`method method-${(e.method || '').toLowerCase()}`}>
                  {e.method}
                </span>
                <strong>{e.path}</strong>
                <small>{e.host}</small>
              </td>
              <td>{e.count}</td>
              <td className={e.errorCount ? 'red-text' : ''}>{e.errorCount || '—'}</td>
              <td>
                <div className="duration-value">
                  <span
                    style={{
                      width: `${Math.max(4, Math.min(85, ((e.averageDurationMs || 0) / Math.max(...endpoints.map((x) => x.averageDurationMs || 1))) * 85))}%`,
                    }}
                  />
                  <strong>{ms(e.averageDurationMs)}</strong>
                </div>
              </td>
              <td>{bytes(e.totalBytes)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="analysis-footnote">
        <Info size={13} /> Findings describe observed traffic. Repeated calls can be intentional;
        verify behavior in your application.
      </p>
    </div>
  );
}
function CompareView({
  sessions,
  selected,
  right,
  onRight,
  value,
  busy,
  onCompare,
}: {
  sessions: Session[];
  selected: string;
  right: string;
  onRight: (id: string) => void;
  value: Comparison | null;
  busy: boolean;
  onCompare: () => void;
}) {
  const added = (value?.addedEndpoints || value?.added || []) as Array<string | Endpoint>;
  const removed = (value?.removedEndpoints || value?.removed || []) as Array<string | Endpoint>;
  const changed = (value?.changedEndpoints || value?.changed || []) as Endpoint[];
  const endpointName = (row: string | Endpoint) =>
    typeof row === 'string'
      ? row
      : String(row.endpoint || `${row.method || ''} ${row.host || ''}${row.path || ''}`);
  return (
    <div className="compare-view">
      <div className="compare-controls">
        <div>
          <span className="field-label">Baseline session</span>
          <div className="baseline-session">
            <FolderArchive size={17} />
            {sessions.find((s) => s.id === selected)?.name || 'Select a session in the sidebar'}
          </div>
        </div>
        <ArrowLeftRight size={20} />
        <div>
          <label className="field-label" htmlFor="compare-session">
            Compare with
          </label>
          <select
            id="compare-session"
            className="text-input"
            value={right}
            onChange={(e) => onRight(e.target.value)}
          >
            <option value="">Choose a saved session</option>
            {sessions
              .filter((s) => s.id !== selected)
              .map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
          </select>
        </div>
        <button
          className="button primary"
          disabled={!selected || !right || busy}
          onClick={onCompare}
        >
          <ArrowLeftRight size={15} /> Compare
        </button>
      </div>
      {!value ? (
        <Empty icon={<ArrowLeftRight size={31} />} title="Two sessions. A clearer picture.">
          Choose two captures to find added, removed and changed endpoints. Sessions can be imported
          from HAR or Scope archives.
        </Empty>
      ) : (
        <>
          <div className="diff-summary">
            <span>
              <Plus size={15} />
              {added.length} added
            </span>
            <span>
              <X size={15} />
              {removed.length} removed
            </span>
            <span>
              <ArrowLeftRight size={15} />
              {changed.length} changed
            </span>
          </div>
          {added.length + removed.length + changed.length === 0 ? (
            <Empty icon={<Check size={30} />} title="No endpoint differences found.">
              The observed endpoints match between these sessions.
            </Empty>
          ) : (
            <>
              {[
                { label: 'Added', rows: added },
                { label: 'Removed', rows: removed },
              ].map(
                (group) =>
                  group.rows.length > 0 && (
                    <div className="diff-section" key={group.label}>
                      <h2>{group.label} endpoints</h2>
                      {group.rows.map((row, i) => (
                        <div className="diff-endpoint" key={i}>
                          <span
                            className={`method method-${endpointName(row).split(' ')[0].toLowerCase()}`}
                          >
                            {endpointName(row).split(' ')[0]}
                          </span>
                          <strong>{endpointName(row).split(' ').slice(1).join(' ')}</strong>
                        </div>
                      ))}
                    </div>
                  ),
              )}
              {changed.length > 0 && (
                <div className="diff-section">
                  <h2>Changed endpoints</h2>
                  {changed.map((row, i) => (
                    <div className="diff-endpoint changed-endpoint" key={i}>
                      <strong>{endpointName(row)}</strong>
                      <div className="change-metrics">
                        <span>
                          Requests <b>{String(row.leftCount ?? '—')}</b>
                          <ArrowRight size={11} />
                          <b>{String(row.rightCount ?? '—')}</b>
                        </span>
                        <span>
                          Average <b>{ms(Number(row.leftAverageDurationMs || 0))}</b>
                          <ArrowRight size={11} />
                          <b>{ms(Number(row.rightAverageDurationMs || 0))}</b>
                        </span>
                      </div>
                      <div className="field-differences">
                        {((row.addedResponseFields as string[]) || []).map((field) => (
                          <span className="added-field" key={field}>
                            <Plus size={11} />
                            {field}
                          </span>
                        ))}
                        {((row.removedResponseFields as string[]) || []).map((field) => (
                          <span className="removed-field" key={field}>
                            <X size={11} />
                            {field}
                          </span>
                        ))}
                      </div>
                      <div className="status-difference">
                        Status codes: {JSON.stringify(row.leftStatuses)} <ArrowRight size={11} />{' '}
                        {JSON.stringify(row.rightStatuses)}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </>
          )}
          <p className="analysis-footnote">
            <Info size={13} />
            {String(value.method || 'Comparison describes the traffic observed in these sessions.')}
          </p>
        </>
      )}
    </div>
  );
}
