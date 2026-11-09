import { useCallback, useEffect, useState } from 'react';
import type { FormEvent } from 'react';
import {
  ArrowDownUp,
  ArrowRight,
  Check,
  ChevronRight,
  Copy,
  Filter,
  GitBranch,
  LoaderCircle,
  Plus,
  RefreshCw,
  Save,
  Shield,
  Trash2,
} from 'lucide-react';
import { api } from '../api';
import {
  messageOf,
  validateHeaders,
  validateMethod,
  validateStatus,
  validateUrl,
} from '../form-utils';
import type { Header, RuleAction, TrafficRule } from '../types';
import { HeaderEditor, InlineError, WorkspaceHeading } from './WorkbenchUi';

const actionLabels: Record<RuleAction['type'], string> = {
  pause: 'Pause & edit',
  rewriteRequest: 'Rewrite request',
  rewriteResponse: 'Rewrite response',
  mockResponse: 'Mock response',
  block: 'Block request',
};
const pauseStageLabels = {
  request: 'Request',
  response: 'Response',
  both: 'Request & response',
} as const;
const ruleActionLabel = (action: RuleAction) =>
  action.type === 'pause'
    ? `Pause & edit · ${pauseStageLabels[action.stage]}`
    : actionLabels[action.type];
const newRule = (priority = 10): TrafficRule => ({
  id: '',
  name: '',
  enabled: true,
  priority,
  matcher: {},
  action: { type: 'pause', stage: 'request' },
});
const sortRules = (rules: TrafficRule[]) =>
  [...rules].sort((a, b) => a.priority - b.priority || a.id.localeCompare(b.id));
export function RulesWorkspace({ native }: { native: boolean }) {
  const [rules, setRules] = useState<TrafficRule[]>([]);
  const [selection, setSelection] = useState<TrafficRule | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [deleteTarget, setDeleteTarget] = useState<TrafficRule | null>(null);
  const [notice, setNotice] = useState('');
  const load = useCallback(async () => {
    if (!native) return;
    setBusy(true);
    setError('');
    try {
      setRules(sortRules(await api.list_rules()));
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  }, [native]);
  useEffect(() => {
    void load();
  }, [load]);
  async function mutate(task: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setError('');
    setNotice('');
    try {
      await task();
      setRules(sortRules(await api.list_rules()));
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  }
  async function save(rule: TrafficRule) {
    setBusy(true);
    try {
      const saved = await api.save_rule(rule);
      setSelection(saved);
      setNotice('Rule saved. New requests use the updated rule set.');
      setRules(sortRules(await api.list_rules()));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="workbench-page rules-page">
      <WorkspaceHeading
        icon={<GitBranch size={24} />}
        title="Traffic rules"
        subtitle="Shape a conversation before it reaches the application."
        actions={
          <>
            <button className="button" disabled={!native || busy} onClick={() => void load()}>
              <RefreshCw size={14} className={busy ? 'spin' : ''} /> Refresh
            </button>
            <button
              className="button primary"
              disabled={!native || busy}
              onClick={() => {
                setSelection(newRule(Math.max(0, ...rules.map((r) => r.priority)) + 10));
                setNotice('');
              }}
            >
              <Plus size={15} /> New rule
            </button>
          </>
        }
      />
      <InlineError message={error} />
      {notice && (
        <div className="workbench-notice" role="status">
          <Check size={14} />
          {notice}
        </div>
      )}
      <div className="rules-workbench">
        <aside className="rule-library">
          <div className="rule-library-heading">
            <span>
              {rules.length} {rules.length === 1 ? 'rule' : 'rules'}
            </span>
            <span>
              <ArrowDownUp size={12} /> Priority order
            </span>
          </div>
          {!rules.length ? (
            <div className="rule-empty">
              <Filter size={25} />
              <h3>No rules yet</h3>
              <p>
                Match an endpoint, then pause and edit its real traffic. Or apply an automatic
                action.
              </p>
              {!native && <small>Rules require the native desktop engine.</small>}
            </div>
          ) : (
            rules.map((rule) => (
              <div
                className={`rule-list-row ${selection?.id === rule.id ? 'selected-rule' : ''}`}
                key={rule.id}
              >
                <button
                  role="switch"
                  aria-checked={rule.enabled}
                  aria-label={`Enable rule ${rule.name}`}
                  className={`switch compact-switch ${rule.enabled ? 'on' : ''}`}
                  disabled={busy}
                  onClick={() =>
                    void mutate(async () => {
                      const updated = await api.save_rule({ ...rule, enabled: !rule.enabled });
                      if (selection?.id === rule.id) setSelection(updated);
                    })
                  }
                >
                  <span />
                </button>
                <button
                  className="rule-select"
                  onClick={() => {
                    setSelection(rule);
                    setNotice('');
                  }}
                >
                  <strong>{rule.name}</strong>
                  <small>{ruleActionLabel(rule.action)}</small>
                  <span className="rule-match-summary">
                    {rule.matcher.method || 'Any method'} <span>·</span>{' '}
                    {rule.matcher.host ||
                      rule.matcher.pathPrefix ||
                      rule.matcher.urlContains ||
                      'All traffic'}
                  </span>
                </button>
                <span className="rule-priority" title="Priority">
                  {rule.priority}
                </span>
              </div>
            ))
          )}
          <div className="rule-order-note">
            <Shield size={14} />
            <p>
              Lower priorities run first. Matching uses the original method and URL. A request keeps
              its own snapshot of the rules.
            </p>
          </div>
        </aside>
        <main className="rule-editor-pane">
          {selection ? (
            <>
              <div className="rule-editor-toolbar">
                <span>{selection.id ? 'Edit rule' : 'New rule'}</span>
                {selection.id && (
                  <>
                    <button
                      className="text-button"
                      disabled={busy}
                      onClick={() =>
                        void mutate(async () => {
                          const copy = await api.save_rule({
                            ...selection,
                            id: '',
                            name: `${selection.name} copy`,
                            enabled: false,
                            priority: selection.priority + 1,
                          });
                          setSelection(copy);
                          setNotice('Duplicated as a disabled rule. Enable it when ready.');
                        })
                      }
                    >
                      <Copy size={13} /> Duplicate
                    </button>
                    <button
                      className="text-button danger-text"
                      disabled={busy}
                      onClick={() => setDeleteTarget(selection)}
                    >
                      <Trash2 size={13} /> Delete
                    </button>
                  </>
                )}
              </div>
              {deleteTarget?.id === selection.id && (
                <div className="rule-delete-confirm">
                  <p>Delete “{deleteTarget.name}”? Existing captures keep their rule history.</p>
                  <button className="button small" onClick={() => setDeleteTarget(null)}>
                    Keep rule
                  </button>
                  <button
                    className="button small danger"
                    onClick={() =>
                      void mutate(async () => {
                        await api.delete_rule(deleteTarget.id);
                        setSelection(null);
                        setDeleteTarget(null);
                        setNotice('Rule deleted.');
                      })
                    }
                  >
                    Delete rule
                  </button>
                </div>
              )}
              <RuleEditor
                key={`${selection.id || 'new'}:${selection.enabled}:${selection.priority}`}
                rule={selection}
                disabled={!native || busy}
                onSave={save}
              />
            </>
          ) : (
            <div className="rules-start">
              <div className="rule-flow">
                <span>
                  <Filter size={22} />
                </span>
                <ArrowRight size={18} />
                <span>
                  <GitBranch size={22} />
                </span>
                <ArrowRight size={18} />
                <span>
                  <Check size={22} />
                </span>
              </div>
              <h2>Catch the next request. Change what happens.</h2>
              <p>
                Pause a matching request or response, inspect the actual data, then send it
                unchanged or edit it first. Automatic actions are available too.
              </p>
              <button
                className="button"
                disabled={!native || busy}
                onClick={() => setSelection(newRule())}
              >
                <Plus size={14} /> Create your first rule
              </button>
              <div className="rule-examples">
                <span>Edit an authorization header before sending</span>
                <span>Change a real response to exercise an error state</span>
                <span>Mock an unavailable service automatically</span>
              </div>
            </div>
          )}
        </main>
      </div>
    </section>
  );
}

export function RuleEditor({
  rule,
  disabled,
  onSave,
}: {
  rule: TrafficRule;
  disabled: boolean;
  onSave: (rule: TrafficRule) => Promise<void>;
}) {
  const [name, setName] = useState(rule.name),
    [enabled, setEnabled] = useState(rule.enabled),
    [priority, setPriority] = useState(String(rule.priority));
  const [method, setMethod] = useState(rule.matcher.method || ''),
    [host, setHost] = useState(rule.matcher.host || ''),
    [path, setPath] = useState(rule.matcher.pathPrefix || ''),
    [contains, setContains] = useState(rule.matcher.urlContains || '');
  const [matchAll, setMatchAll] = useState(!!rule.id && !Object.values(rule.matcher).some(Boolean));
  const [kind, setKind] = useState<RuleAction['type']>(rule.action.type);
  const [pauseStage, setPauseStage] = useState<'request' | 'response' | 'both'>(
    rule.action.type === 'pause' ? rule.action.stage : 'request',
  );
  const action =
    rule.action.type === 'block' || rule.action.type === 'pause' ? undefined : rule.action;
  const [status, setStatus] = useState(
    String(action && 'status' in action ? (action.status ?? '') : ''),
  );
  const [rewriteMethod, setRewriteMethod] = useState(
    action && 'method' in action ? action.method || '' : '',
  );
  const [url, setUrl] = useState(action && 'url' in action ? action.url || '' : '');
  const [headers, setHeaders] = useState<Header[]>(action?.headers || []),
    [replaceHeaders, setReplaceHeaders] = useState(action?.headers !== undefined);
  const [body, setBody] = useState(action?.body || ''),
    [replaceBody, setReplaceBody] = useState(action?.body !== undefined),
    [base64, setBase64] = useState(action?.bodyBase64 || false);
  const [reason, setReason] = useState(
    rule.action.type === 'block' ? rule.action.reason || '' : '',
  );
  const [error, setError] = useState(''),
    [saving, setSaving] = useState(false);
  const mock = kind === 'mockResponse';
  const locked = disabled || saving;
  async function submit(event: FormEvent) {
    event.preventDefault();
    setError('');
    try {
      if (!name.trim()) throw new Error('Give this rule a name.');
      if (!priority.trim() || !Number.isSafeInteger(Number(priority)) || Number(priority) < 0)
        throw new Error('Priority must be a non-negative whole number.');
      const matcher = matchAll
        ? {}
        : {
            ...(method.trim() ? { method: method.trim().toUpperCase() } : {}),
            ...(host.trim() ? { host: host.trim() } : {}),
            ...(path.trim() ? { pathPrefix: path.trim() } : {}),
            ...(contains.trim() ? { urlContains: contains.trim() } : {}),
          };
      if (!matchAll && !Object.keys(matcher).length)
        throw new Error('Add at least one matcher, or explicitly match all traffic.');
      if (matcher.method) validateMethod(matcher.method);
      if (matcher.host && /[\s/:]/.test(matcher.host))
        throw new Error('Enter a hostname without a scheme, path or port.');
      if (matcher.pathPrefix && !matcher.pathPrefix.startsWith('/'))
        throw new Error('A path prefix must start with /.');
      let nextAction: RuleAction;
      if (kind === 'pause') nextAction = { type: 'pause', stage: pauseStage };
      else if (kind === 'block')
        nextAction = { type: 'block', ...(reason.trim() ? { reason: reason.trim() } : {}) };
      else {
        if (mock || replaceHeaders) validateHeaders(headers);
        if (rewriteMethod.trim()) validateMethod(rewriteMethod.trim());
        if (url.trim()) validateUrl(url.trim());
        const fields = {
          ...(mock || replaceHeaders ? { headers } : {}),
          ...(mock || replaceBody ? { body, bodyBase64: base64 } : {}),
        };
        nextAction =
          kind === 'rewriteRequest'
            ? {
                type: kind,
                ...fields,
                ...(rewriteMethod.trim() ? { method: rewriteMethod.trim().toUpperCase() } : {}),
                ...(url.trim() ? { url: url.trim() } : {}),
              }
            : kind === 'rewriteResponse'
              ? {
                  type: kind,
                  ...fields,
                  ...(status.trim() ? { status: validateStatus(status) } : {}),
                }
              : {
                  type: 'mockResponse',
                  status: validateStatus(status || '200'),
                  headers,
                  body,
                  bodyBase64: base64,
                };
        if (kind !== 'mockResponse' && Object.keys(nextAction).length === 1)
          throw new Error('Choose at least one field to rewrite.');
      }
      setSaving(true);
      await onSave({
        id: rule.id,
        name: name.trim(),
        enabled,
        priority: Number(priority),
        matcher,
        action: nextAction,
      });
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setSaving(false);
    }
  }
  return (
    <form className="rule-editor-form" onSubmit={(event) => void submit(event)}>
      <div className="rule-identification">
        <div>
          <label className="field-label" htmlFor="rule-name">
            Rule name
          </label>
          <input
            id="rule-name"
            className="text-input"
            value={name}
            placeholder="e.g. Product API maintenance"
            maxLength={120}
            disabled={locked}
            onChange={(e) => setName(e.target.value)}
          />
        </div>
        <div>
          <label className="field-label" htmlFor="rule-priority">
            Priority
          </label>
          <input
            id="rule-priority"
            type="number"
            min="0"
            step="1"
            className="text-input"
            value={priority}
            disabled={locked}
            onChange={(e) => setPriority(e.target.value)}
          />
        </div>
        <label className="enabled-label">
          <input
            type="checkbox"
            checked={enabled}
            disabled={locked}
            onChange={(e) => setEnabled(e.target.checked)}
          />{' '}
          Enabled
        </label>
      </div>
      <section className="rule-editor-section">
        <div className="editor-section-heading">
          <span className="section-step">
            <Filter size={15} />
          </span>
          <h3>When the request matches</h3>
          <label className="match-all-label">
            <input
              type="checkbox"
              checked={matchAll}
              disabled={locked}
              onChange={(e) => setMatchAll(e.target.checked)}
            />{' '}
            All traffic
          </label>
        </div>
        {matchAll ? (
          <div className="subtle-warning">
            This rule matches every request. Add a host or path to narrow it.
          </div>
        ) : (
          <div className="matcher-fields">
            <div>
              <label className="field-label" htmlFor="match-method">
                Method
              </label>
              <select
                id="match-method"
                className="text-input"
                value={method}
                disabled={locked}
                onChange={(e) => setMethod(e.target.value)}
              >
                <option value="">Any method</option>
                {['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS', 'CONNECT'].map(
                  (m) => (
                    <option key={m}>{m}</option>
                  ),
                )}
              </select>
            </div>
            <div>
              <label className="field-label" htmlFor="match-host">
                Host
              </label>
              <input
                id="match-host"
                className="text-input"
                placeholder="api.example.com"
                value={host}
                disabled={locked}
                onChange={(e) => setHost(e.target.value)}
              />
            </div>
            <div>
              <label className="field-label" htmlFor="match-path">
                Path starts with
              </label>
              <input
                id="match-path"
                className="text-input"
                placeholder="/api/products"
                value={path}
                disabled={locked}
                onChange={(e) => setPath(e.target.value)}
              />
            </div>
            <div>
              <label className="field-label" htmlFor="match-url">
                URL contains
              </label>
              <input
                id="match-url"
                className="text-input"
                placeholder="Optional exact text"
                value={contains}
                disabled={locked}
                onChange={(e) => setContains(e.target.value)}
              />
            </div>
          </div>
        )}
        <p className="field-help">
          All supplied matchers must match the original incoming request.
        </p>
      </section>
      <section className="rule-editor-section">
        <div className="editor-section-heading">
          <span className="section-step">
            <ChevronRight size={16} />
          </span>
          <h3>Apply this action</h3>
        </div>
        <div className="rule-action-picker" role="group" aria-label="Rule action">
          {Object.entries(actionLabels).map(([type, label]) => (
            <button
              type="button"
              key={type}
              className={kind === type ? 'action-chosen' : ''}
              aria-pressed={kind === type}
              disabled={locked}
              onClick={() => {
                setKind(type as RuleAction['type']);
                if (type === 'mockResponse' && !status) setStatus('200');
              }}
            >
              {label}
            </button>
          ))}
        </div>
        {kind === 'pause' ? (
          <div className="pause-rule-details">
            <label className="field-label" htmlFor="rule-pause-stage">
              Pause at
            </label>
            <select
              id="rule-pause-stage"
              className="text-input"
              value={pauseStage}
              disabled={locked}
              onChange={(event) => setPauseStage(event.target.value as typeof pauseStage)}
            >
              {Object.entries(pauseStageLabels).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
            <p className="field-help">
              Save this rule, then trigger a matching request in your app. Scope waits for the
              actual {pauseStage === 'both' ? 'request and response' : pauseStage} before you edit
              it. This rule pauses only matching endpoints.
            </p>
            <p className="field-help">
              {pauseStage === 'request'
                ? 'Edit the URL, method, headers or body before the request reaches the server.'
                : pauseStage === 'response'
                  ? 'Edit the status, headers or body after the server replies, before your app receives it.'
                  : 'Edit the request before it reaches the server, then edit the response before your app receives it.'}{' '}
              Choose Continue unchanged, Send with changes or Abort in the paused-message editor.
            </p>
          </div>
        ) : kind === 'block' ? (
          <>
            <label className="field-label" htmlFor="block-reason">
              Reason shown in the capture
            </label>
            <input
              id="block-reason"
              className="text-input"
              placeholder="Blocked by local test rule"
              value={reason}
              disabled={locked}
              onChange={(e) => setReason(e.target.value)}
            />
            <p className="field-help">
              No request is sent upstream. The blocked exchange remains in the session.
            </p>
          </>
        ) : (
          <>
            {kind === 'rewriteRequest' ? (
              <div className="rewrite-request-fields">
                <div>
                  <label className="field-label" htmlFor="rewrite-method">
                    New method
                  </label>
                  <input
                    id="rewrite-method"
                    className="text-input"
                    value={rewriteMethod}
                    placeholder="Keep original"
                    disabled={locked}
                    onChange={(e) => setRewriteMethod(e.target.value)}
                  />
                </div>
                <div>
                  <label className="field-label" htmlFor="rewrite-url">
                    New URL
                  </label>
                  <input
                    id="rewrite-url"
                    className="text-input"
                    value={url}
                    placeholder="Keep original URL"
                    disabled={locked}
                    onChange={(e) => setUrl(e.target.value)}
                  />
                </div>
              </div>
            ) : (
              <>
                <label className="field-label" htmlFor="rule-response-status">
                  Response status
                </label>
                <input
                  id="rule-response-status"
                  className="text-input narrow-status"
                  type="number"
                  min="100"
                  max="599"
                  value={status}
                  placeholder={mock ? '200' : 'Keep original'}
                  disabled={locked}
                  onChange={(e) => setStatus(e.target.value)}
                />
              </>
            )}
            {!mock && (
              <label className="field-check">
                <input
                  type="checkbox"
                  checked={replaceHeaders}
                  disabled={locked}
                  onChange={(e) => setReplaceHeaders(e.target.checked)}
                />{' '}
                Replace the complete header list
              </label>
            )}
            {(mock || replaceHeaders) && (
              <HeaderEditor value={headers} onChange={setHeaders} disabled={locked} />
            )}
            {!mock && (
              <label className="field-check">
                <input
                  type="checkbox"
                  checked={replaceBody}
                  disabled={locked}
                  onChange={(e) => setReplaceBody(e.target.checked)}
                />{' '}
                Replace body
              </label>
            )}
            {(mock || replaceBody) && (
              <>
                <div className="editor-field-heading">
                  <label htmlFor="rule-body">Body</label>
                  <label className="field-check compact">
                    <input
                      type="checkbox"
                      checked={base64}
                      disabled={locked}
                      onChange={(e) => setBase64(e.target.checked)}
                    />{' '}
                    Binary as base64
                  </label>
                </div>
                <textarea
                  id="rule-body"
                  className="code-editor rule-body"
                  spellCheck={false}
                  value={body}
                  disabled={locked}
                  onChange={(e) => setBody(e.target.value)}
                />
              </>
            )}
            <p className="field-help">
              {mock
                ? 'Returns a local response without contacting upstream.'
                : 'Omitted fields keep their current values. Headers replace the whole list.'}{' '}
              Transport headers and body length are normalized by the engine.
            </p>
          </>
        )}
      </section>
      <InlineError message={error} />
      <div className="rule-save-footer">
        <span>{rule.id ? `Rule ${rule.id.slice(0, 8)}` : 'Not saved yet'}</span>
        <button className="button primary" disabled={locked}>
          {saving ? <LoaderCircle size={14} className="spin" /> : <Save size={14} />} Save rule
        </button>
      </div>
    </form>
  );
}
