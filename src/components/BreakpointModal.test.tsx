import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { BreakpointModal } from './BreakpointModal';
import type { PendingExchange } from '../types';

const request: PendingExchange = {
  id: 'pending-request',
  exchangeId: 'exchange-1',
  stage: 'request',
  method: 'POST',
  url: 'https://api.example.com/products',
  status: null,
  headers: [
    { name: 'x-test', value: 'one' },
    { name: 'x-test', value: 'two' },
  ],
  body: '{"sku":"real-data"}',
  bodyBase64: false,
  createdAt: 0,
};
const response: PendingExchange = {
  ...request,
  id: 'pending-response',
  stage: 'response',
  status: 403,
  headers: [{ name: 'content-type', value: 'application/json' }],
  body: '{"error":"denied"}',
};
describe('Paused message editor', () => {
  it('preserves a case-sensitive extension method when only the body is edited', async () => {
    const resolve = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(
      <BreakpointModal
        item={{ ...request, method: 'customVerb' }}
        onResolve={resolve}
        onClose={vi.fn()}
      />,
    );
    fireEvent.change(screen.getByLabelText('Request body'), { target: { value: 'edited body' } });
    await user.click(screen.getByRole('button', { name: 'Send with changes' }));
    expect(resolve).toHaveBeenCalledWith({
      action: 'continue',
      body: 'edited body',
      bodyBase64: false,
    });
  });
  it('sends an explicitly edited extension method with its original casing', async () => {
    const resolve = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<BreakpointModal item={request} onResolve={resolve} onClose={vi.fn()} />);
    fireEvent.change(screen.getByLabelText('Method'), { target: { value: 'customVerb' } });
    await user.click(screen.getByRole('button', { name: 'Send with changes' }));
    expect(resolve).toHaveBeenCalledWith({ action: 'continue', method: 'customVerb' });
  });
  it('shows captured data and sends edited request fields only on explicit Send', async () => {
    const resolve = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<BreakpointModal item={request} onResolve={resolve} onClose={vi.fn()} />);
    expect(screen.getByLabelText('Request body')).toHaveValue(request.body);
    expect(screen.getByRole('button', { name: 'Send with changes' })).toBeDisabled();
    await user.clear(screen.getByLabelText('Method'));
    await user.type(screen.getByLabelText('Method'), 'PATCH');
    await user.clear(screen.getByLabelText('Request URL'));
    await user.type(screen.getByLabelText('Request URL'), 'https://api.example.com/products/1');
    await user.type(screen.getByLabelText('Header 2 value'), '-edited');
    fireEvent.change(screen.getByLabelText('Request body'), {
      target: { value: '{"sku":"edited"}' },
    });
    expect(resolve).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Send with changes' }));
    expect(resolve).toHaveBeenCalledWith({
      action: 'continue',
      method: 'PATCH',
      url: 'https://api.example.com/products/1',
      headers: [
        { name: 'x-test', value: 'one' },
        { name: 'x-test', value: 'two-edited' },
      ],
      body: '{"sku":"edited"}',
      bodyBase64: false,
    });
    expect(request.headers[1].value).toBe('two');
  });
  it('continues unchanged without sending even invalid unsaved edits', async () => {
    const resolve = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<BreakpointModal item={request} onResolve={resolve} onClose={vi.fn()} />);
    await user.clear(screen.getByLabelText('Request URL'));
    await user.click(screen.getByRole('button', { name: 'Continue unchanged' }));
    expect(resolve).toHaveBeenCalledWith({ action: 'continue' });
  });
  it('sends only an edited response status, without request fields or body overrides', async () => {
    const resolve = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<BreakpointModal item={response} onResolve={resolve} onClose={vi.fn()} />);
    expect(screen.queryByLabelText('Request URL')).not.toBeInTheDocument();
    await user.clear(screen.getByLabelText('Response status'));
    await user.type(screen.getByLabelText('Response status'), '200');
    await user.click(screen.getByRole('button', { name: 'Send with changes' }));
    expect(resolve).toHaveBeenCalledWith({ action: 'continue', status: 200 });
  });
  it.each([request, response])(
    'aborts only the selected $stage without applying edits',
    async (item) => {
      const resolve = vi.fn().mockResolvedValue(undefined);
      const user = userEvent.setup();
      render(<BreakpointModal item={item} onResolve={resolve} onClose={vi.fn()} />);
      await user.type(
        screen.getByLabelText(`${item.stage === 'request' ? 'Request' : 'Response'} body`),
        'unsent',
      );
      await user.click(screen.getByRole('button', { name: `Abort ${item.stage}` }));
      expect(resolve).toHaveBeenCalledWith({ action: 'abort' });
    },
  );
  it.each([
    ['Method', 'CONNECT', 'CONNECT cannot'],
    ['Method', 'bad method', 'valid HTTP method'],
    ['Request URL', 'file:///private', 'Only HTTP and HTTPS'],
    ['Request URL', 'https://user:secret@example.com', 'URL credentials'],
    ['Header 1 name', 'bad name', 'valid name'],
  ])('rejects invalid %s before resolving', async (label, value, message) => {
    const resolve = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<BreakpointModal item={request} onResolve={resolve} onClose={vi.fn()} />);
    fireEvent.change(screen.getByLabelText(label), { target: { value } });
    await user.click(screen.getByRole('button', { name: 'Send with changes' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(message);
    expect(resolve).not.toHaveBeenCalled();
  });
  it.each(['101', '600', '200.5', ''])(
    'rejects response status %s before resolving',
    async (status) => {
      const resolve = vi.fn().mockResolvedValue(undefined);
      const user = userEvent.setup();
      render(<BreakpointModal item={response} onResolve={resolve} onClose={vi.fn()} />);
      fireEvent.change(screen.getByLabelText('Response status'), { target: { value: status } });
      await user.click(screen.getByRole('button', { name: 'Send with changes' }));
      expect(await screen.findByRole('alert')).toHaveTextContent(/status/);
      expect(resolve).not.toHaveBeenCalled();
    },
  );
  it('retains binary body representation and rejects malformed base64', async () => {
    const resolve = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(
      <BreakpointModal
        item={{ ...request, body: 'AA==', bodyBase64: true }}
        onResolve={resolve}
        onClose={vi.fn()}
      />,
    );
    fireEvent.change(screen.getByLabelText(/Request body/), { target: { value: 'invalid' } });
    await user.click(screen.getByRole('button', { name: 'Send with changes' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('valid base64');
    expect(resolve).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText(/Request body/), { target: { value: 'AQ==' } });
    await user.click(screen.getByRole('button', { name: 'Send with changes' }));
    expect(resolve).toHaveBeenCalledWith({ action: 'continue', body: 'AQ==', bodyBase64: true });
  });
  it('resets captured fields and errors for the next paused ID and response stage', async () => {
    const resolve = vi.fn().mockResolvedValue(undefined);
    const props = { onResolve: resolve, onClose: vi.fn() };
    const user = userEvent.setup();
    const { rerender } = render(<BreakpointModal item={request} {...props} />);
    fireEvent.change(screen.getByLabelText('Request URL'), { target: { value: 'invalid' } });
    await user.click(screen.getByRole('button', { name: 'Send with changes' }));
    expect(await screen.findByRole('alert')).toBeVisible();
    rerender(<BreakpointModal item={response} {...props} />);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByLabelText('Response status')).toHaveValue('403');
    expect(screen.getByLabelText('Response body')).toHaveValue(response.body);
    expect(screen.getByRole('button', { name: 'Send with changes' })).toBeDisabled();
    rerender(
      <BreakpointModal
        item={{ ...request, id: 'pending-next', body: 'next real body' }}
        {...props}
      />,
    );
    expect(screen.getByLabelText('Request URL')).toHaveValue(request.url);
    expect(screen.getByLabelText('Request body')).toHaveValue('next real body');
  });
  it('keeps edits after a backend rejection and permits a corrected retry', async () => {
    const resolve = vi
      .fn()
      .mockRejectedValueOnce(new Error('Header rejected by engine.'))
      .mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<BreakpointModal item={request} onResolve={resolve} onClose={vi.fn()} />);
    fireEvent.change(screen.getByLabelText('Request body'), { target: { value: 'new body' } });
    await user.click(screen.getByRole('button', { name: 'Send with changes' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Header rejected by engine.');
    expect(screen.getByLabelText('Request body')).toHaveValue('new body');
    await user.click(screen.getByRole('button', { name: 'Send with changes' }));
    await waitFor(() => expect(resolve).toHaveBeenCalledTimes(2));
  });
  it('locks resolution controls during one pending operation', async () => {
    let finish!: () => void;
    const resolve = vi.fn(
      () =>
        new Promise<void>((done) => {
          finish = done;
        }),
    );
    const user = userEvent.setup();
    render(<BreakpointModal item={request} onResolve={resolve} onClose={vi.fn()} />);
    await user.click(screen.getByRole('button', { name: 'Continue unchanged' }));
    expect(screen.getByRole('button', { name: 'Abort request' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Continue unchanged' }));
    expect(resolve).toHaveBeenCalledOnce();
    finish();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Continue unchanged' })).toBeEnabled(),
    );
  });
});
