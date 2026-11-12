import { invoke } from '@tauri-apps/api/core';
import type {
  Analysis,
  AuditEvent,
  DeviceDiagnostics,
  SessionUpdate,
  TrafficRule,
  App,
  Bootstrap,
  Comparison,
  Device,
  DeviceStatus,
  EditMessage,
  Exchange,
  PendingExchange,
  ProxyStatus,
  Session,
} from './types';
export const isNative = () => '__TAURI_INTERNALS__' in window;
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isNative())
    throw new Error(
      'Open the Snurpo desktop app to connect devices and capture traffic. This browser preview shows the interface only.',
    );
  return invoke<T>('scope_command', { command, args: args || {} });
}
export const api = {
  list_rules: () => call<TrafficRule[]>('list_rules'),
  save_rule: (rule: TrafficRule) => call<TrafficRule>('save_rule', { rule }),
  delete_rule: (id: string) => call<void>('delete_rule', { id }),
  update_session: (id: string, update: SessionUpdate) =>
    call<Session>('update_session', { id, update }),
  delete_session: (id: string) => call<void>('delete_session', { id }),
  list_audit: (limit = 200) => call<AuditEvent[]>('list_audit', { limit }),
  device_diagnostics: (serial?: string) =>
    call<DeviceDiagnostics>('device_diagnostics', { serial }),
  infer_openapi: (sessionId: string) => call<unknown>('infer_openapi', { sessionId }),
  bootstrap: () => call<Bootstrap>('bootstrap'),
  list_sessions: () => call<Session[]>('list_sessions'),
  create_session: (name: string, source = 'desktop') =>
    call<Session>('create_session', { name, source }),
  list_exchanges: (sessionId: string, query?: string, limit = 500, offset = 0) =>
    call<Exchange[]>('list_exchanges', { sessionId, query: query || undefined, limit, offset }),
  get_exchange: (id: string) => call<Exchange>('get_exchange', { id }),
  analysis: (sessionId: string) => call<Analysis>('analysis', { sessionId }),
  save_export: (sessionId: string, format: 'har' | 'scope' | 'openapi', redact = false) =>
    call<{ saved: boolean; path?: string }>('save_export', { sessionId, format, redact }),
  export_har: (sessionId: string) => call<unknown>('export_har', { sessionId }),
  import_har: (content: string, name: string) => call<Session>('import_har', { content, name }),
  export_session: (sessionId: string) => call<unknown>('export_session', { sessionId }),
  import_session: (content: string) => call<Session>('import_session', { content }),
  start_proxy: (sessionId: string, port: number) =>
    call<ProxyStatus>('start_proxy', { sessionId, port }),
  stop_proxy: () => call<void>('stop_proxy'),
  proxy_status: () => call<ProxyStatus>('proxy_status'),
  set_breakpoints: (request: boolean, response: boolean) =>
    call<void>('set_breakpoints', { request, response }),
  pending: () => call<PendingExchange[]>('pending'),
  resolve_breakpoint: (id: string, edit: EditMessage) =>
    call<void>('resolve_breakpoint', { id, edit }),
  replay: (id: string, edit?: EditMessage) => call<Exchange>('replay', { id, edit }),
  compare_sessions: (left: string, right: string) =>
    call<Comparison>('compare_sessions', { left, right }),
  list_devices: () => call<Device[]>('list_devices'),
  list_apps: (serial: string) => call<App[]>('list_apps', { serial }),
  start_device: (serial: string, packageName: string, mode: string, profile: string[]) =>
    call<DeviceStatus>('start_device', { serial, package: packageName, mode, profile }),
  stop_device: () => call<DeviceStatus>('stop_device'),
  device_status: () => call<DeviceStatus>('device_status'),
  set_mcp_scope: (
    sessionIds: string[],
    allowWrites: boolean,
    allowRules = false,
    allowDeviceDiagnostics = false,
  ) =>
    call<unknown>('set_mcp_scope', { sessionIds, allowWrites, allowRules, allowDeviceDiagnostics }),
  generate_demo: (sessionId: string) => call<unknown>('generate_demo', { sessionId }),
};
