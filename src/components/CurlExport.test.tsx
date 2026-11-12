import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CurlExport } from './CurlExport';
import type { Exchange } from '../types';
const exchange: Exchange = {
  id: 'one',
  sessionId: 'session',
  method: 'POST',
  url: 'https://example.com/one',
  requestHeaders: [{ name: 'Authorization', value: 'Bearer synthetic' }],
  responseHeaders: [],
  requestBody: 'test',
  responseBody: '',
  requestBodyBase64: false,
  responseBodyBase64: false,
  status: 200,
  startedAt: 0,
  durationMs: 1,
  requestBytes: 4,
  responseBytes: 0,
  error: null,
  completed: true,
  edited: false,
};
afterEach(() => vi.unstubAllGlobals());
function open() {
  fireEvent.click(screen.getByRole('button', { name: 'Copy as cURL' }));
}
describe('cURL preview', () => {
  it('escapes inspector layout and restores trigger focus after Escape', () => {
    const view = render(
      <aside className="inspector">
        <CurlExport exchange={exchange} />
      </aside>,
    );
    const trigger = screen.getByRole('button', { name: 'Copy as cURL' });
    trigger.focus();
    open();
    const dialog = screen.getByRole('dialog');
    expect(dialog.closest('.inspector')).toBeNull();
    expect(view.container).not.toContainElement(dialog);
    expect(screen.getByRole('button', { name: 'Close dialog' })).toHaveFocus();
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });
  it('previews actual credentials but only writes clipboard after explicit copy', async () => {
    const copy = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal('navigator', { clipboard: { writeText: copy } });
    render(<CurlExport exchange={exchange} />);
    expect(copy).not.toHaveBeenCalled();
    open();
    expect(screen.getByRole('dialog')).toHaveAccessibleName('Copy as cURL');
    expect(screen.getByText(/may contain credentials/)).toBeVisible();
    const command = (screen.getByLabelText('cURL command (POSIX shell)') as HTMLTextAreaElement)
      .value;
    expect(command).toContain('Bearer synthetic');
    expect(copy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Copy command' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Copied'));
    expect(copy).toHaveBeenCalledWith(command);
  });
  it('reports clipboard rejection while leaving selectable command available', async () => {
    vi.stubGlobal('navigator', {
      clipboard: { writeText: vi.fn().mockRejectedValue(new Error('denied')) },
    });
    render(<CurlExport exchange={exchange} />);
    open();
    fireEvent.click(screen.getByRole('button', { name: 'Copy command' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Select the command and copy it manually',
    );
    expect(
      (screen.getByLabelText('cURL command (POSIX shell)') as HTMLTextAreaElement).value,
    ).toContain('curl');
    expect(screen.queryByText('Copied to clipboard.')).not.toBeInTheDocument();
  });
  it('explains invalid requests and prevents copying a partial command', () => {
    render(<CurlExport exchange={{ ...exchange, url: 'file:///secret' }} />);
    open();
    expect(screen.getByRole('alert')).toHaveTextContent('Only HTTP and HTTPS');
    expect(screen.getByRole('button', { name: 'Copy command' })).toBeDisabled();
    expect(screen.queryByLabelText('cURL command (POSIX shell)')).not.toBeInTheDocument();
  });
  it('refreshes a same-ID effective request and ignores an old pending clipboard result', async () => {
    let finish!: () => void;
    const copy = vi.fn().mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    vi.stubGlobal('navigator', { clipboard: { writeText: copy } });
    const view = render(<CurlExport exchange={exchange} />);
    open();
    fireEvent.click(screen.getByRole('button', { name: 'Copy command' }));
    view.rerender(<CurlExport exchange={{ ...exchange, method: 'PUT', requestBody: 'updated' }} />);
    const command = (screen.getByLabelText('cURL command (POSIX shell)') as HTMLTextAreaElement)
      .value;
    expect(command).toContain("--request 'PUT'");
    expect(command).toContain('updated');
    await act(async () => finish());
    expect(screen.queryByText('Copied to clipboard.')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Copy command' })).toBeEnabled();
  });
  it('uses the newly selected exchange and discards old clipboard feedback', async () => {
    const copy = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal('navigator', { clipboard: { writeText: copy } });
    const view = render(<CurlExport key="one" exchange={exchange} />);
    open();
    fireEvent.click(screen.getByRole('button', { name: 'Copy command' }));
    await screen.findByText('Copied to clipboard.');
    view.rerender(
      <CurlExport
        key="two"
        exchange={{ ...exchange, id: 'two', url: 'https://example.com/two' }}
      />,
    );
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    open();
    expect(
      (screen.getByLabelText('cURL command (POSIX shell)') as HTMLTextAreaElement).value,
    ).toContain('/two');
    expect(screen.queryByText('Copied to clipboard.')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Close dialog' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});
