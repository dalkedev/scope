import { useRef, useState } from 'react';
import { LoaderCircle, Pause, Play } from 'lucide-react';
import {
  messageOf,
  validateHeaders,
  validateMethod,
  validateStatus,
  validateUrl,
} from '../form-utils';
import type { EditMessage, Header, PendingExchange } from '../types';
import { HeaderEditor, InlineError, ModalFrame } from './WorkbenchUi';

type Props = {
  item: PendingExchange;
  onClose: () => void;
  onResolve: (edit: EditMessage) => Promise<void>;
};

// Request and response pauses for the same exchange are separate messages.
// A new pending ID must never inherit edits from the previous one.
export function BreakpointModal(props: Props) {
  return <BreakpointEditor key={`${props.item.id}:${props.item.stage}`} {...props} />;
}

function BreakpointEditor({ item, onClose, onResolve }: Props) {
  const request = item.stage === 'request';
  const [method, setMethod] = useState(item.method);
  const [url, setUrl] = useState(item.url);
  const [status, setStatus] = useState(String(item.status ?? 200));
  const [headers, setHeaders] = useState<Header[]>(item.headers.map((header) => ({ ...header })));
  const [body, setBody] = useState(item.body);
  const [error, setError] = useState('');
  const [resolving, setResolving] = useState(false);
  const resolvingRef = useRef(false);
  const headersChanged = JSON.stringify(headers) !== JSON.stringify(item.headers);
  const bodyChanged = body !== item.body;
  const changed =
    headersChanged ||
    bodyChanged ||
    (request ? method !== item.method || url !== item.url : status !== String(item.status ?? 200));

  async function resolve(edit: EditMessage) {
    if (resolvingRef.current) return;
    resolvingRef.current = true;
    setResolving(true);
    setError('');
    try {
      await onResolve(edit);
    } catch (e) {
      setError(messageOf(e));
    } finally {
      resolvingRef.current = false;
      setResolving(false);
    }
  }

  function sendChanges() {
    setError('');
    try {
      const edit: EditMessage = { action: 'continue' };
      if (request) {
        // HTTP method tokens are case-sensitive, including extension methods.
        // Editing a body or header must never rewrite the captured method.
        const nextMethod = method.trim();
        const nextUrl = url.trim();
        validateMethod(nextMethod);
        if (nextMethod === 'CONNECT')
          throw new Error(
            'CONNECT cannot be used as an edited request method. Continue unchanged or choose another method.',
          );
        validateUrl(nextUrl);
        if (nextMethod !== item.method) edit.method = nextMethod;
        if (nextUrl !== item.url) edit.url = nextUrl;
      } else {
        const nextStatus = validateStatus(status);
        if (nextStatus < 200) throw new Error('Enter a final response status from 200 to 599.');
        if (nextStatus !== item.status) edit.status = nextStatus;
      }
      if (headersChanged) {
        validateHeaders(headers);
        edit.headers = headers;
      }
      if (bodyChanged) {
        if (
          item.bodyBase64 &&
          (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(body) ||
            btoa(atob(body)) !== body)
        )
          throw new Error('Enter valid base64 for this binary body.');
        edit.body = body;
        edit.bodyBase64 = item.bodyBase64;
      }
      void resolve(edit);
    } catch (e) {
      setError(messageOf(e));
    }
  }

  return (
    <ModalFrame
      title={`Paused ${request ? 'request' : 'response'}`}
      subtitle={
        request
          ? 'Inspect the captured request before it reaches the server. Changes apply only to this message.'
          : 'Inspect the server’s response before your app receives it. Changes apply only to this message.'
      }
      icon={<Pause size={20} />}
      onClose={onClose}
      className="breakpoint-modal"
    >
      <p className="breakpoint-destination">
        <strong>{item.method}</strong> {item.url}
      </p>
      <p className="field-help breakpoint-hold-note">
        Scope holds this message for up to 60 seconds from capture, then aborts it. Your client may
        time out sooner. Closing this editor keeps the message paused.
      </p>
      {request ? (
        <div className="composer-destination">
          <div>
            <label className="field-label" htmlFor="breakpoint-method">
              Method
            </label>
            <input
              id="breakpoint-method"
              className="text-input"
              value={method}
              disabled={resolving}
              onChange={(event) => setMethod(event.target.value)}
            />
          </div>
          <div>
            <label className="field-label" htmlFor="breakpoint-url">
              Request URL
            </label>
            <input
              id="breakpoint-url"
              className="text-input"
              value={url}
              disabled={resolving}
              onChange={(event) => setUrl(event.target.value)}
            />
          </div>
        </div>
      ) : (
        <>
          <label className="field-label" htmlFor="breakpoint-status">
            Response status
          </label>
          <input
            id="breakpoint-status"
            className="text-input narrow-status"
            value={status}
            inputMode="numeric"
            disabled={resolving}
            onChange={(event) => setStatus(event.target.value)}
          />
        </>
      )}
      <HeaderEditor value={headers} onChange={setHeaders} disabled={resolving} />
      <label className="field-label" htmlFor="breakpoint-body">
        {request ? 'Request' : 'Response'} body {item.bodyBase64 ? '· base64' : ''}
      </label>
      <textarea
        id="breakpoint-body"
        spellCheck={false}
        className="code-editor"
        value={body}
        disabled={resolving}
        onChange={(event) => setBody(event.target.value)}
      />
      <p className="field-help">
        Only edited fields are replaced. Repeated headers keep their order. Transport headers and
        body length are normalized by the engine. The captured original stays in the session.
      </p>
      <InlineError message={error} />
      <div className="modal-actions breakpoint-actions">
        <button
          type="button"
          className="button danger"
          disabled={resolving}
          onClick={() => void resolve({ action: 'abort' })}
        >
          Abort {request ? 'request' : 'response'}
        </button>
        <button
          type="button"
          className="button"
          disabled={resolving}
          onClick={() => void resolve({ action: 'continue' })}
        >
          Continue unchanged
        </button>
        <button
          type="button"
          className="button primary"
          disabled={resolving || !changed}
          onClick={sendChanges}
        >
          {resolving ? <LoaderCircle size={14} className="spin" /> : <Play size={14} />}
          Send with changes
        </button>
      </div>
    </ModalFrame>
  );
}
