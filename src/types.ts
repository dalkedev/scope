export interface Header {
  name: string;
  value: string;
}
export interface Session {
  id: string;
  name: string;
  source: string;
  createdAt: number;
  exchangeCount: number;
  revision?: number | string;
  notes?: string;
  tags?: string[];
  pinned?: boolean;
}
export interface Exchange {
  id: string;
  sessionId: string;
  method: string;
  url: string;
  status: number | null;
  requestHeaders: Header[];
  responseHeaders: Header[];
  requestBody: string;
  responseBody: string;
  requestBodyBase64: boolean;
  responseBodyBase64: boolean;
  startedAt: number;
  durationMs: number;
  requestBytes: number;
  responseBytes: number;
  error: string | null;
  edited: boolean;
  completed: boolean;
  provenance?: 'upstream' | 'mock' | 'blocked';
  appliedRules?: AppliedRule[];
  originalRequest?: unknown;
  originalResponse?: unknown;
}
export interface ProxyStatus {
  running: boolean;
  port: number;
  sessionId: string | null;
  requestBreakpoint: boolean;
  responseBreakpoint: boolean;
  caCertPath: string;
}
export interface PendingExchange {
  id: string;
  exchangeId: string;
  stage: string;
  method: string;
  url: string;
  status: number | null;
  headers: Header[];
  body: string;
  bodyBase64: boolean;
  createdAt: number;
}
export interface EditMessage {
  action: string;
  method?: string;
  url?: string;
  status?: number;
  headers?: Header[];
  body?: string;
  bodyBase64?: boolean;
}
export interface Device {
  serial: string;
  model: string;
  state: string;
  rooted: boolean;
}
export interface App {
  package: string;
  name: string;
  running: boolean;
}
export interface DeviceStatus {
  running: boolean;
  serial: string | null;
  package: string | null;
  mode: string | null;
  message: string;
  logs: string[];
}
export interface Bootstrap {
  sessions: Session[];
  proxy: ProxyStatus;
  dataDir: string;
  version: string;
  deviceStatus: DeviceStatus;
  mcpPath?: string;
  mcpConfig?: unknown;
  mcpStatus?: McpStatus;
}
export interface Endpoint {
  method?: string;
  path?: string;
  host?: string;
  count?: number;
  errorCount?: number;
  averageDurationMs?: number;
  avgDurationMs?: number;
  totalBytes?: number;
  [key: string]: unknown;
}
export interface Analysis {
  totalRequests?: number;
  totalErrors?: number;
  totalBytes?: number;
  averageDurationMs?: number;
  endpoints?: Endpoint[];
  findings?: Array<{ kind?: string; message: string; exchangeIds?: string[] }>;
  [key: string]: unknown;
}
export interface Comparison {
  added?: Endpoint[];
  removed?: Endpoint[];
  changed?: Endpoint[];
  [key: string]: unknown;
}
export interface McpStatus {
  sessionIds: string[];
  allowWrites: boolean;
  allowRules?: boolean;
  allowDeviceDiagnostics?: boolean;
  port: number;
}

export interface RuleMatcher {
  method?: string;
  host?: string;
  pathPrefix?: string;
  urlContains?: string;
}
export type RuleAction =
  | { type: 'pause'; stage: 'request' | 'response' | 'both' }
  | {
      type: 'rewriteRequest';
      method?: string;
      url?: string;
      headers?: Header[];
      body?: string;
      bodyBase64?: boolean;
    }
  | {
      type: 'rewriteResponse';
      status?: number;
      headers?: Header[];
      body?: string;
      bodyBase64?: boolean;
    }
  | { type: 'mockResponse'; status: number; headers: Header[]; body: string; bodyBase64: boolean }
  | { type: 'block'; reason?: string };
export interface TrafficRule {
  id: string;
  name: string;
  enabled: boolean;
  priority: number;
  matcher: RuleMatcher;
  action: RuleAction;
}
export interface AppliedRule {
  ruleId: string;
  name: string;
  action: string;
  stage: string;
}
export interface SessionUpdate {
  name?: string;
  notes?: string;
  tags?: string[];
  pinned?: boolean;
}
export interface AuditEvent {
  id: string;
  createdAt: number;
  action: string;
  targetId?: string | null;
}
export interface DiagnosticCheck {
  id: string;
  status: 'ok' | 'warning' | 'error' | 'info';
  title: string;
  detail: string;
  action: string | null;
  command: string | null;
}
export interface DeviceDiagnostics {
  checkedAt: number;
  readOnly: true;
  selectedSerial: string | null;
  readyForProxy: boolean;
  readyForFrida: boolean;
  adb: { path: string | null; available: boolean; version: string | null; error: string | null };
  devices: Device[];
  device: {
    serial: string;
    model: string;
    state: string;
    authorized: boolean;
    rootStatus: string;
    rootMethod: string | null;
    abi: string | null;
    androidVersion: string | null;
    sdk: number | null;
    error: string | null;
  } | null;
  frida: {
    runtimeKind: string | null;
    runtimePath: string | null;
    runtimeAvailable: boolean;
    runtimeVersion: string | null;
    runtimeError: string | null;
    serverSource: string | null;
    serverPath: string | null;
    serverAvailable: boolean;
    serverVersion: string | null;
    serverVersionVerified: boolean;
    serverRunning: boolean;
    versionMatch: boolean | null;
    abiSupported: boolean | null;
    serverError: string | null;
    experimental: true;
  };
  mappings: {
    reverse: Mapping[];
    forward: Mapping[];
    owned: Mapping[];
    globalProxy: string | null;
    error: string | null;
  };
  recovery: {
    exists: boolean;
    valid: boolean;
    needsCleanup: boolean;
    active: boolean;
    serial: string | null;
    package: string | null;
    mode: string | null;
    proxyPort: number | null;
    forwardPort: number | null;
    serverPid: string | number | null;
    caHash: string | null;
    error: string | null;
  };
  checks: DiagnosticCheck[];
  notes: string[];
}
export interface Mapping {
  serial: string;
  kind: 'forward' | 'reverse';
  local: string;
  remote: string;
  owned: boolean;
}
