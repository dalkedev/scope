import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App';
import type { PendingExchange, ProxyStatus, Session } from './types';

const mocks = vi.hoisted(() => ({
  bootstrap: vi.fn(),
  list_sessions: vi.fn(),
  proxy_status: vi.fn(),
  pending: vi.fn(),
  device_status: vi.fn(),
  list_exchanges: vi.fn(),
  list_rules: vi.fn(),
  resolve_breakpoint: vi.fn(),
  generate_demo: vi.fn(),
  list_audit: vi.fn(),
  set_breakpoints: vi.fn(),
  start_proxy: vi.fn(),
  stop_proxy: vi.fn(),
  create_session: vi.fn(),
  list_devices: vi.fn(),
  list_apps: vi.fn(),
  start_device: vi.fn(),
}));
vi.mock('./api', () => ({ isNative: () => true, api: mocks }));

const session: Session = {
  id: 'session-1',
  name: 'Endpoint debugging',
  source: 'desktop',
  createdAt: 0,
  exchangeCount: 0,
};
const proxy: ProxyStatus = {
  running: true,
  port: 17890,
  sessionId: session.id,
  requestBreakpoint: false,
  responseBreakpoint: false,
  caCertPath: '/local/ca.pem',
};
const device = { running: false, serial: null, package: null, mode: null, message: '', logs: [] };
const pending: PendingExchange = {
  id: 'pending-1',
  exchangeId: 'exchange-1',
  stage: 'request',
  method: 'POST',
  url: 'https://api.example.com/products',
  status: null,
  headers: [],
  body: 'real body',
  bodyBase64: false,
  createdAt: 0,
};
async function poll() {
  await act(async () => {
    vi.advanceTimersByTime(2000);
  });
}
async function mount() {
  await act(async () => {
    render(<App />);
  });
  await act(async () => {
    vi.advanceTimersByTime(0);
  });
}
beforeEach(() => {
  vi.useFakeTimers();
  vi.resetAllMocks();
  mocks.bootstrap.mockResolvedValue({
    sessions: [session],
    proxy,
    deviceStatus: device,
    version: 'test',
  });
  mocks.list_sessions.mockResolvedValue([session]);
  mocks.proxy_status.mockResolvedValue(proxy);
  mocks.device_status.mockResolvedValue(device);
  mocks.pending.mockResolvedValue([pending]);
  mocks.list_exchanges.mockResolvedValue([]);
  mocks.list_rules.mockResolvedValue([]);
  mocks.list_audit.mockResolvedValue([]);
  mocks.resolve_breakpoint.mockResolvedValue(undefined);
  mocks.start_proxy.mockResolvedValue(proxy);
  mocks.stop_proxy.mockResolvedValue(undefined);
  mocks.create_session.mockResolvedValue(session);
  mocks.generate_demo.mockResolvedValue(undefined);
  mocks.list_devices.mockResolvedValue([]);
  mocks.list_apps.mockResolvedValue([]);
  mocks.start_device.mockResolvedValue({ ...device, running: true, serial: 'test-device' });
});
afterEach(() => {
  vi.useRealTimers();
});

describe('Proxy listener settings', () => {
  function idleListener() {
    const idle = { ...proxy, running: false, port: 0, sessionId: null };
    mocks.bootstrap.mockResolvedValue({ sessions: [session], proxy: idle, deviceStatus: device });
    mocks.proxy_status.mockResolvedValue(idle);
    mocks.pending.mockResolvedValue([]);
  }
  async function openSettings() {
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Connection settings' }));
    });
  }
  async function setDraft(port: string) {
    await openSettings();
    fireEvent.change(screen.getByLabelText('Proxy port'), { target: { value: port } });
  }
  async function generateDemo() {
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /^Traffic \d/ }));
    });
    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: 'Generate real test traffic through the proxy' }),
      );
    });
  }
  function androidAvailable() {
    mocks.list_devices.mockResolvedValue([
      { serial: 'test-device', model: 'Test Android', state: 'device', rooted: false },
    ]);
    mocks.list_apps.mockResolvedValue([
      { name: 'Demo app', package: 'com.example.demo', running: false },
    ]);
  }
  async function interceptAndroid() {
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Connect a device Android/ }));
    });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Demo app com.example.demo/ }));
    });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Start interception' }));
    });
  }
  it('uses the configured port when demo traffic starts an idle listener', async () => {
    idleListener();
    await mount();
    await setDraft('17990');
    await generateDemo();
    expect(mocks.start_proxy).toHaveBeenCalledWith(session.id, 17990);
    expect(mocks.generate_demo).toHaveBeenCalledWith(session.id);
  });
  it('uses the configured port before starting Android interception', async () => {
    idleListener();
    androidAvailable();
    await mount();
    await setDraft('17990');
    await interceptAndroid();
    expect(mocks.start_proxy).toHaveBeenCalledWith(session.id, 17990);
    expect(mocks.start_device).toHaveBeenCalledWith('test-device', 'com.example.demo', 'proxy', [
      'certificate',
      'unpinning',
      'proxy',
    ]);
    expect(mocks.start_proxy.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.start_device.mock.invocationCallOrder[0],
    );
  });
  it.each(['demo', 'Android'] as const)(
    'validates the port before any %s start side effects',
    async (entrypoint) => {
      idleListener();
      androidAvailable();
      mocks.bootstrap.mockResolvedValue({
        sessions: [],
        proxy: { ...proxy, running: false, port: 0, sessionId: null },
        deviceStatus: device,
      });
      await mount();
      for (const port of ['', '1023', '65536', '17990.5']) {
        await setDraft(port);
        if (entrypoint === 'demo') await generateDemo();
        else await interceptAndroid();
        expect(screen.getByRole('alert')).toHaveTextContent('whole proxy port from 1024 to 65535');
        expect(mocks.create_session).not.toHaveBeenCalled();
        expect(mocks.start_proxy).not.toHaveBeenCalled();
        expect(mocks.generate_demo).not.toHaveBeenCalled();
        expect(mocks.start_device).not.toHaveBeenCalled();
      }
    },
  );
  it('does not let a delayed pre-Stop status snapshot overwrite the new idle port draft', async () => {
    await mount();
    await openSettings();
    let releaseOldPoll!: (sessions: Session[]) => void;
    mocks.list_sessions.mockImplementationOnce(
      () =>
        new Promise<Session[]>((resolve) => {
          releaseOldPoll = resolve;
        }),
    );
    await poll();
    mocks.proxy_status.mockResolvedValue({ ...proxy, running: false, port: 0, sessionId: null });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Stop capture' }));
    });
    fireEvent.change(screen.getByLabelText('Proxy port'), { target: { value: '17990' } });
    await act(async () => {
      releaseOldPoll([session]);
    });
    expect(screen.getByLabelText('Proxy port')).toHaveValue(17990);
    expect(screen.getByLabelText('Proxy port')).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Start capture' })).toBeVisible();
  });
  it('preserves a typed port through idle polling and starts the proxy on that port', async () => {
    idleListener();
    mocks.start_proxy.mockResolvedValue({ ...proxy, port: 17990 });
    await mount();
    await openSettings();
    expect(screen.getByLabelText('Proxy port')).toHaveValue(17890);
    fireEvent.change(screen.getByLabelText('Proxy port'), { target: { value: '17990' } });
    await poll();
    expect(screen.getByLabelText('Proxy port')).toHaveValue(17990);
    expect(screen.getByLabelText('Proxy port')).toBeEnabled();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Start capture' }));
    });
    expect(mocks.start_proxy).toHaveBeenCalledWith(session.id, 17990);
    expect(screen.getByLabelText('Proxy port')).toHaveValue(17990);
    expect(screen.getByLabelText('Proxy port')).toBeDisabled();
  });
  it('retains the draft between panels and adopts the authoritative running listener port', async () => {
    idleListener();
    await mount();
    await openSettings();
    fireEvent.change(screen.getByLabelText('Proxy port'), { target: { value: '17990' } });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Traffic rules' }));
    });
    await poll();
    await openSettings();
    expect(screen.getByLabelText('Proxy port')).toHaveValue(17990);
    mocks.proxy_status.mockResolvedValue({ ...proxy, port: 18001 });
    await poll();
    expect(screen.getByLabelText('Proxy port')).toHaveValue(18001);
    expect(screen.getByLabelText('Proxy port')).toBeDisabled();
    expect(screen.getByText(/curl --proxy/)).toHaveTextContent('127.0.0.1:18001');
    mocks.proxy_status.mockResolvedValue({ ...proxy, running: false, port: 0, sessionId: null });
    await poll();
    expect(screen.getByLabelText('Proxy port')).toHaveValue(18001);
    expect(screen.getByLabelText('Proxy port')).toBeEnabled();
  });
  it.each(['', '1023', '65536', '17990.5'])(
    'rejects invalid port %s before starting capture',
    async (port) => {
      idleListener();
      await mount();
      await openSettings();
      fireEvent.change(screen.getByLabelText('Proxy port'), { target: { value: port } });
      await act(async () => {
        fireEvent.click(screen.getByRole('button', { name: 'Start capture' }));
      });
      expect(screen.getByRole('alert')).toHaveTextContent('whole proxy port from 1024 to 65535');
      expect(mocks.start_proxy).not.toHaveBeenCalled();
    },
  );
});

describe('Paused traffic across workspaces', () => {
  it('keeps unsent edits when an older empty poll arrives after a newer paused message', async () => {
    let releaseOldPoll!: (sessions: Session[]) => void;
    mocks.list_sessions.mockImplementationOnce(
      () =>
        new Promise<Session[]>((resolve) => {
          releaseOldPoll = resolve;
        }),
    );
    mocks.pending.mockResolvedValueOnce([]).mockResolvedValue([pending]);
    await mount();
    await poll();
    expect(screen.queryByRole('button', { name: 'Inspect & edit' })).not.toBeInTheDocument();
    await poll();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Inspect & edit' }));
    });
    fireEvent.change(screen.getByLabelText('Request body'), {
      target: { value: 'keep my unsent edits' },
    });
    await act(async () => {
      releaseOldPoll([session]);
    });
    expect(screen.getByRole('dialog', { name: 'Paused request' })).toBeVisible();
    expect(screen.getByLabelText('Request body')).toHaveValue('keep my unsent edits');
    expect(mocks.resolve_breakpoint).not.toHaveBeenCalled();
  });
  it('does not resurrect a resolved ID from a poll that began before Continue', async () => {
    await mount();
    await poll();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Inspect & edit' }));
    });
    let releaseOldPoll!: (sessions: Session[]) => void;
    mocks.list_sessions.mockImplementationOnce(
      () =>
        new Promise<Session[]>((resolve) => {
          releaseOldPoll = resolve;
        }),
    );
    await poll();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Continue unchanged' }));
    });
    expect(mocks.resolve_breakpoint).toHaveBeenCalledWith(pending.id, { action: 'continue' });
    expect(screen.queryByRole('button', { name: 'Inspect & edit' })).not.toBeInTheDocument();
    await act(async () => {
      releaseOldPoll([session]);
    });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Inspect & edit' })).not.toBeInTheDocument();
  });
  it('keeps a pending action visible in Traffic rules and Activity without interrupting rule edits', async () => {
    await mount();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Traffic rules' }));
    });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'New rule' }));
    });
    expect(screen.getByLabelText('Pause at')).toHaveValue('request');
    expect(screen.queryByLabelText('Response status')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Body', { exact: true })).not.toBeInTheDocument();
    screen.getByLabelText('Rule name').focus();
    fireEvent.change(screen.getByLabelText('Rule name'), { target: { value: 'Inspect products' } });
    await poll();
    expect(screen.getByRole('heading', { name: 'Traffic rules' })).toBeVisible();
    expect(screen.getByLabelText('Rule name')).toHaveValue('Inspect products');
    expect(screen.getByLabelText('Rule name')).toHaveFocus();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(mocks.set_breakpoints).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Inspect & edit' })).toBeVisible();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Inspect & edit' }));
    });
    expect(screen.getByRole('dialog', { name: 'Paused request' })).toBeVisible();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Close dialog' }));
    });
    expect(mocks.resolve_breakpoint).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Activity trail' }));
    });
    expect(screen.getByRole('heading', { name: 'Activity trail' })).toBeVisible();
    expect(screen.getByRole('button', { name: 'Inspect & edit' })).toBeVisible();
  });
  it('can resolve while an unrelated demo request is still running', async () => {
    let finishDemo!: () => void;
    mocks.generate_demo.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finishDemo = resolve;
        }),
    );
    await mount();
    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: 'Generate real test traffic through the proxy' }),
      );
    });
    expect(mocks.generate_demo).toHaveBeenCalledWith(session.id);
    await poll();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Inspect & edit' }));
    });
    expect(screen.getByRole('button', { name: 'Continue unchanged' })).toBeEnabled();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Continue unchanged' }));
    });
    expect(mocks.resolve_breakpoint).toHaveBeenCalledWith(pending.id, { action: 'continue' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Message continued unchanged.');
    await act(async () => {
      finishDemo();
    });
  });
  it('retires only an expired editor and leaves another pending transfer available', async () => {
    await mount();
    await poll();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Inspect & edit' }));
    });
    fireEvent.change(screen.getByLabelText('Request body'), { target: { value: 'unsent edits' } });
    const next = {
      ...pending,
      id: 'pending-next',
      exchangeId: 'exchange-next',
      body: 'next real body',
    };
    mocks.pending.mockResolvedValue([next]);
    await poll();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('no longer paused');
    expect(mocks.resolve_breakpoint).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Inspect & edit' }));
    });
    expect(screen.getByLabelText('Request body')).toHaveValue('next real body');
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Abort request' }));
    });
    expect(mocks.resolve_breakpoint).toHaveBeenCalledWith('pending-next', { action: 'abort' });
  });
});
