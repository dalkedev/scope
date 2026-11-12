import { useEffect, useMemo, useRef, useState } from 'react';
import { Terminal } from 'lucide-react';
import { requestToCurl } from '../curl';
import { messageOf } from '../form-utils';
import type { Exchange } from '../types';
import { InlineError, ModalFrame } from './WorkbenchUi';

export function CurlExport({ exchange }: { exchange: Exchange }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button className="button small" onClick={() => setOpen(true)}>
        <Terminal size={13} /> Copy as cURL
      </button>
      {open && <CurlDialog exchange={exchange} onClose={() => setOpen(false)} />}
    </>
  );
}

function CurlDialog({ exchange, onClose }: { exchange: Exchange; onClose: () => void }) {
  const { command, error } = useMemo(() => {
    try {
      return { command: requestToCurl(exchange), error: '' };
    } catch (e) {
      return { command: '', error: messageOf(e) };
    }
  }, [
    exchange.method,
    exchange.url,
    exchange.requestHeaders,
    exchange.requestBody,
    exchange.requestBodyBase64,
  ]);
  return <CurlPreview key={command || error} command={command} error={error} onClose={onClose} />;
}

function CurlPreview({
  command,
  error,
  onClose,
}: {
  command: string;
  error: string;
  onClose: () => void;
}) {
  const [copyState, setCopyState] = useState<'idle' | 'copying' | 'copied' | 'failed'>('idle');
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  async function copy() {
    setCopyState('copying');
    try {
      await navigator.clipboard.writeText(command);
      if (mounted.current) setCopyState('copied');
    } catch {
      if (mounted.current) setCopyState('failed');
    }
  }
  return (
    <ModalFrame
      title="Copy as cURL"
      subtitle="POSIX shell: sh, bash or zsh. Not PowerShell or cmd.exe."
      icon={<Terminal size={22} />}
      onClose={onClose}
    >
      <p className="field-help">
        This command may contain credentials in its URL, headers and body. Review it before sharing.
        Nothing is sent when you preview or copy. Running it sends a new request using your
        terminal's network settings; Snurpo rules and breakpoints are not included.
      </p>
      <InlineError message={error} />
      {command && (
        <>
          <label className="field-label" htmlFor="curl-command">
            cURL command (POSIX shell)
          </label>
          <textarea
            id="curl-command"
            className="code-editor"
            rows={12}
            value={command}
            readOnly
            spellCheck={false}
          />
          <p className="field-help">
            Host, content length and connection headers are regenerated. Redirects are not followed
            automatically.
          </p>
        </>
      )}
      {copyState === 'copied' && <p role="status">Copied to clipboard.</p>}
      <InlineError
        message={
          copyState === 'failed'
            ? 'Clipboard unavailable. Select the command and copy it manually.'
            : ''
        }
      />
      <footer className="modal-actions">
        <button className="button" onClick={onClose}>
          Close
        </button>
        <button
          className="button primary"
          onClick={() => void copy()}
          disabled={!command || copyState === 'copying'}
        >
          {copyState === 'copying' ? 'Copying…' : 'Copy command'}
        </button>
      </footer>
    </ModalFrame>
  );
}
