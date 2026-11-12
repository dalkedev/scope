#!/usr/bin/env node
// Real local desktop-engine + stdio MCP integration test. No Android device or remote API.
import assert from 'node:assert/strict';
import { spawn, execFile } from 'node:child_process';
import { mkdtemp, readFile, rm, access, realpath } from 'node:fs/promises';
import { createServer as createHttpServer } from 'node:http';
import { createServer as createTcpServer } from 'node:net';
import { once } from 'node:events';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';
import { promisify } from 'node:util';
import { randomUUID } from 'node:crypto';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const extension = process.platform === 'win32' ? '.exe' : '';
const scopeBinary = resolve(
  process.env.SCOPE_BIN ?? join(root, 'target/debug', `scope${extension}`),
);
const mcpBinary = resolve(
  process.env.SCOPE_MCP_BIN ?? join(root, 'target/debug', `scope-mcp${extension}`),
);
const runFile = promisify(execFile);
const children = new Set();
let testDirectory;
let fixture;
let completed = false;

function deferred() {
  let resolvePromise, rejectPromise;
  const promise = new Promise((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;
  });
  return { promise, resolve: resolvePromise, reject: rejectPromise };
}

async function bounded(promise, label, milliseconds = 20_000) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(
          () => reject(new Error(`${label} timed out after ${milliseconds} ms`)),
          milliseconds,
        );
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

function launch(executable, args) {
  const child = spawn(executable, args, {
    cwd: root,
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true,
  });
  children.add(child);
  child.diagnostics = '';
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', (chunk) => {
    child.diagnostics = (child.diagnostics + chunk).slice(-65_536);
  });
  child.on('exit', () => {
    children.delete(child);
  });
  child.on('error', () => {
    children.delete(child);
  });
  return child;
}

async function stop(child, signal = 'SIGINT') {
  if (!child || !child.pid || child.exitCode !== null || child.signalCode !== null) return;
  const exit = once(child, 'exit');
  child.kill(signal);
  try {
    await bounded(exit, `Stopping ${child.spawnfile}`, 7_000);
  } catch (error) {
    child.kill('SIGKILL');
    await bounded(exit, 'Forced process cleanup', 3_000);
    throw error;
  }
}

async function cleanup() {
  const results = await Promise.allSettled([...children].map((child) => stop(child)));
  for (const result of results)
    if (result.status === 'rejected') console.error(result.reason.message);
  if (fixture) {
    fixture.closeAllConnections();
    await new Promise((resolve) => fixture.close(resolve));
  }
  if (testDirectory && (completed || !process.env.SCOPE_KEEP_SMOKE_DATA))
    await rm(testDirectory, { recursive: true, force: true });
}

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.once(signal, async () => {
    await cleanup();
    process.exit(signal === 'SIGINT' ? 130 : 143);
  });
}

async function requireFreePort(port) {
  const probe = createTcpServer();
  await new Promise((resolve, reject) => {
    probe.once('error', (error) =>
      reject(
        new Error(
          `Port ${port} is busy. Stop an existing Snurpo demo before running this test. ${error.message}`,
        ),
      ),
    );
    probe.listen({ host: '127.0.0.1', port, exclusive: true }, resolve);
  });
  await new Promise((resolve) => probe.close(resolve));
}

async function startDesktop(dataDirectory, demo) {
  const args = ['--headless', '--data-dir', dataDirectory, '--bridge-port', '0'];
  if (demo) args.push('--demo');
  const child = launch(scopeBinary, args);
  const ready = deferred();
  const lines = createInterface({ input: child.stdout });
  lines.on('line', (line) => {
    try {
      const value = JSON.parse(line);
      if (value.status === 'ready') ready.resolve(value);
    } catch {
      /* Non-JSON diagnostics cannot count as readiness. */
    }
  });
  child.once('error', ready.reject);
  child.once('exit', (code, signal) =>
    ready.reject(
      new Error(`Snurpo exited before readiness (${code ?? signal}): ${child.diagnostics}`),
    ),
  );
  try {
    const metadata = await bounded(ready.promise, 'Desktop readiness', 45_000);
    assert.equal(metadata.status, 'ready');
    const command = metadata.mcpConfig?.mcpServers?.scope?.command;
    assert.equal(typeof command, 'string', 'Desktop must provide an MCP executable command');
    await access(command);
    if (/\.app[/\\]Contents[/\\]MacOS[/\\]/.test(scopeBinary)) {
      const expected = join(
        dirname(scopeBinary),
        '../Resources/resources/bin',
        `scope-mcp${extension}`,
      );
      assert.equal(
        await realpath(command),
        await realpath(expected),
        'A packaged desktop must resolve MCP inside its relocated application bundle',
      );
    }
    return { child, metadata };
  } catch (error) {
    throw new Error(`${error.message}\n${child.diagnostics}`, { cause: error });
  }
}

class McpClient {
  constructor(configPath, executable = mcpBinary) {
    this.child = launch(executable, ['--config', configPath]);
    this.nextId = 1;
    this.pending = new Map();
    this.lines = createInterface({ input: this.child.stdout });
    this.lines.on('line', (line) => {
      let message;
      try {
        message = JSON.parse(line);
      } catch {
        this.fail(new Error(`MCP wrote a non-JSON stdout line: ${line.slice(0, 200)}`));
        return;
      }
      const request = this.pending.get(message.id);
      if (request) {
        this.pending.delete(message.id);
        request.resolve(message);
      }
    });
    this.child.once('error', (error) => this.fail(error));
    this.child.stdin.on('error', (error) => this.fail(error));
    this.child.once('exit', (code, signal) =>
      this.fail(new Error(`MCP exited (${code ?? signal}): ${this.child.diagnostics}`)),
    );
  }
  fail(error) {
    for (const request of this.pending.values()) request.reject(error);
    this.pending.clear();
  }
  async request(method, params = {}) {
    const id = this.nextId++;
    const request = deferred();
    this.pending.set(id, request);
    this.child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
    try {
      const message = await bounded(request.promise, `MCP ${method}`);
      if (message.error) throw new Error(`MCP ${method}: ${JSON.stringify(message.error)}`);
      return message.result;
    } finally {
      this.pending.delete(id);
    }
  }
  async initialize() {
    const result = await this.request('initialize', {
      protocolVersion: '2025-11-25',
      capabilities: {},
      clientInfo: { name: 'scope-e2e-smoke', version: '0.1.0' },
    });
    assert.equal(result.protocolVersion, '2025-11-25');
    assert.ok(result.capabilities.tools, 'MCP must advertise tools');
    this.child.stdin.write(
      `${JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' })}\n`,
    );
  }
  async tool(name, args = {}, expectError = false) {
    const result = await this.request('tools/call', { name, arguments: args });
    const text = result.content
      .filter((item) => item.type === 'text')
      .map((item) => item.text)
      .join('\n');
    if (expectError) {
      assert.equal(result.isError, true, `${name} should deny this operation: ${text}`);
      return text;
    }
    assert.notEqual(result.isError, true, `${name} failed: ${text}`);
    return JSON.parse(text);
  }
  async close() {
    this.child.stdin.end();
    if (this.child.exitCode === null && this.child.signalCode === null) {
      try {
        await bounded(once(this.child, 'exit'), 'MCP clean EOF shutdown', 3_000);
      } catch {
        await stop(this.child);
      }
    }
  }
}

async function openFixture(fixtureMarker) {
  fixture = createHttpServer(async (request, response) => {
    try {
      const parts = [];
      for await (const chunk of request) parts.push(chunk);
      const body = JSON.parse(Buffer.concat(parts).toString('utf8'));
      response.writeHead(200, {
        'Content-Type': 'application/json',
        'Set-Cookie': [`session=${fixtureMarker}-response-cookie; HttpOnly`, 'public=ok'],
        'X-Auth': `${fixtureMarker}-response-auth`,
        'X-Scope-Evidence': 'fixture-response',
      });
      response.end(
        JSON.stringify({
          publicEvidence: 'fixture-response',
          count: 7,
          access_token: `${fixtureMarker}-response-token`,
          nested: { password: `${fixtureMarker}-response-password` },
          echo: body,
        }),
      );
    } catch (error) {
      response.writeHead(500);
      response.end(error.message);
    }
  });
  fixture.listen(0, '127.0.0.1');
  await once(fixture, 'listening');
  return `http://127.0.0.1:${fixture.address().port}/scope-smoke-redaction`;
}

function capturedRows(DatabaseSync, dataDirectory, sessionId) {
  const db = new DatabaseSync(join(dataDirectory, 'scope.sqlite3'), { readOnly: true });
  try {
    return db
      .prepare('SELECT data FROM exchanges WHERE session_id = ? ORDER BY rowid')
      .all(sessionId)
      .map((row) => JSON.parse(row.data));
  } finally {
    db.close();
  }
}

try {
  // Built-in SQLite avoids platform-specific sqlite3/Python installation requirements.
  let DatabaseSync;
  try {
    ({ DatabaseSync } = await import('node:sqlite'));
  } catch {
    throw new Error(
      'This smoke test requires Node.js 22.13 or newer with node:sqlite (Node.js 24 LTS recommended).',
    );
  }
  await Promise.all([access(scopeBinary), access(mcpBinary)]).catch(() => {
    throw new Error('Build the native binaries first: cargo build -p scope-desktop -p scope-mcp');
  });
  await Promise.all([requireFreePort(17890), requireFreePort(17891)]);
  await runFile('curl', ['--version'], { windowsHide: true });
  testDirectory = await mkdtemp(join(tmpdir(), 'scope-smoke-'));
  const dataDirectory = join(testDirectory, 'data');
  const configPath = join(dataDirectory, 'mcp-bridge.json');

  const desktop = await startDesktop(dataDirectory, true);
  const config = JSON.parse(await readFile(configPath, 'utf8'));
  assert.match(config.endpoint, /^http:\/\/127\.0\.0\.1:\d+\/api$/);
  const unauthorized = await fetch(config.endpoint, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ command: 'list_sessions' }),
  });
  assert.equal(unauthorized.status, 401, 'Bridge must reject requests without its local secret');

  const mcp = new McpClient(configPath, desktop.metadata.mcpConfig.mcpServers.scope.command);
  await mcp.initialize();
  const tools = await mcp.request('tools/list');
  for (const tool of [
    'status',
    'list_sessions',
    'search_requests',
    'get_request',
    'analyze_session',
    'compare_sessions',
    'replay_request',
    'infer_openapi',
    'list_rules',
    'save_rule',
    'delete_rule',
    'device_diagnostics',
  ])
    assert.ok(
      tools.tools.some((item) => item.name === tool),
      `Missing MCP tool ${tool}`,
    );
  const sessions = await mcp.tool('list_sessions');
  assert.equal(sessions.length, 1);
  const sessionId = sessions[0].id;
  assert.equal(sessions[0].exchangeCount, 6, 'Demo must have captured six real local requests');
  const rows = await mcp.tool('search_requests', { sessionId, limit: 50 });
  assert.equal(rows.length, 6);
  const one = await mcp.tool('get_request', { id: rows[0].id });
  assert.equal(one.sessionId, sessionId);
  assert.ok(one.responseBody.includes('demo'), 'MCP must return readable captured JSON evidence');
  const analysis = await mcp.tool('analyze_session', { sessionId });
  assert.equal(analysis.totalRequests, 6);
  assert.equal(analysis.totalErrors, 1);
  assert.equal(analysis.inFlightRequests, 0);
  const openapi = await mcp.tool('infer_openapi', { sessionId });
  assert.match(openapi.openapi, /^3\.1\./);
  assert.ok(Object.keys(openapi.paths).length >= 1);
  assert.match(await mcp.tool('list_rules', {}, true), /rule access is disabled/i);
  assert.match(await mcp.tool('device_diagnostics', {}, true), /diagnostics access is disabled/i);
  assert.match(await mcp.tool('replay_request', { id: one.id }, true), /replay is disabled/i);
  assert.match(await mcp.tool('search_requests', { sessionId: randomUUID() }, true), /not shared/i);
  assert.match(
    await mcp.tool('compare_sessions', { sessionId, rightSessionId: randomUUID() }, true),
    /both sessions must be shared/i,
  );
  console.log(
    'PASS desktop + MCP initialization, twelve tools, capture reads, observed OpenAPI, authentication, scope/rule/diagnostic gates and replay denial',
  );

  // Random fixture evidence, never a real credential or an authentication key.
  // Credential-shaped fields intentionally exercise redaction across the real proxy.
  const fixtureMarker = `scope-smoke-fixture-${randomUUID()}`;
  const target = await openFixture(fixtureMarker);
  const body = JSON.stringify({
    publicEvidence: 'fixture-request',
    token: `${fixtureMarker}-request-token`,
    nested: [{ client_secret: `${fixtureMarker}-request-secret`, keep: 42 }],
  });
  const curl = await runFile(
    'curl',
    [
      '--silent',
      '--show-error',
      '--fail',
      '--max-time',
      '15',
      '--noproxy',
      '',
      '--proxy',
      'http://127.0.0.1:17890',
      '--request',
      'POST',
      '--header',
      'Content-Type: application/json',
      '--header',
      `Authorization: Bearer ${fixtureMarker}-authorization`,
      '--header',
      `Cookie: session=${fixtureMarker}-request-cookie`,
      '--header',
      `X-Access-Key: ${fixtureMarker}-access-key`,
      '--header',
      `X-Session-Id: ${fixtureMarker}-session-id`,
      '--data-binary',
      body,
      `${target}?token=${encodeURIComponent(`${fixtureMarker}-query`)}&public=keep`,
    ],
    { timeout: 20_000, maxBuffer: 1024 * 1024, windowsHide: true },
  );
  assert.ok(
    curl.stdout.includes(fixtureMarker),
    'Fixture must actually receive and return the synthetic secret',
  );
  const matches = await mcp.tool('search_requests', { sessionId, query: 'scope-smoke-redaction' });
  assert.equal(matches.length, 1);
  assert.ok(
    !JSON.stringify(matches).includes(fixtureMarker),
    'MCP request metadata leaked a credential',
  );
  assert.ok(
    !('requestBody' in matches[0]) && !('responseBody' in matches[0]),
    'Search results must omit bodies',
  );
  const exchange = await mcp.tool('get_request', { id: matches[0].id });
  assert.ok(
    !JSON.stringify(exchange).includes(fixtureMarker),
    'MCP detailed request leaked a credential',
  );
  assert.ok(
    !('originalRequest' in exchange) && !('originalResponse' in exchange),
    'MCP must omit raw original snapshots',
  );
  const requestJson = JSON.parse(exchange.requestBody);
  const responseJson = JSON.parse(exchange.responseBody);
  assert.equal(requestJson.publicEvidence, 'fixture-request');
  assert.equal(requestJson.nested[0].keep, 42);
  assert.equal(responseJson.publicEvidence, 'fixture-response');
  assert.equal(responseJson.count, 7);
  assert.equal(responseJson.access_token, '[redacted]');
  assert.equal(requestJson.token, '[redacted]');
  assert.equal(requestJson.nested[0].client_secret, '[redacted]');
  assert.ok(exchange.url.includes('public=keep'));
  const persisted = capturedRows(DatabaseSync, dataDirectory, sessionId);
  assert.equal(persisted.length, 7);
  const raw = persisted.find((row) => row.id === exchange.id);
  assert.ok(
    raw.requestBody.includes(fixtureMarker) && raw.responseBody.includes(fixtureMarker),
    'Raw local bodies must be persisted, while MCP alone redacts its view',
  );
  for (const [side, name] of [
    ['requestHeaders', 'cookie'],
    ['responseHeaders', 'set-cookie'],
  ]) {
    const rawCookies = raw[side].filter((header) => header.name.toLowerCase() === name);
    const sharedCookies = exchange[side].filter((header) => header.name.toLowerCase() === name);
    assert.ok(
      rawCookies.some((header) => header.value.includes(fixtureMarker)),
      'The local archive must actually contain the synthetic cookie fixture',
    );
    assert.equal(sharedCookies.length, rawCookies.length);
    assert.ok(
      sharedCookies.every((header) => header.value === '[redacted]'),
      'MCP must redact every captured cookie header value',
    );
  }
  console.log(
    'PASS real proxied request, metadata/query/header/JSON credential redaction and retained public evidence',
  );

  await mcp.close();
  await stop(desktop.child);
  const afterStop = capturedRows(DatabaseSync, dataDirectory, sessionId);
  assert.equal(afterStop.length, 7);
  assert.ok(afterStop.every((row) => row.error === null));
  const reopened = await startDesktop(dataDirectory, false);
  const reopenedMcp = new McpClient(
    configPath,
    reopened.metadata.mcpConfig.mcpServers.scope.command,
  );
  await reopenedMcp.initialize();
  assert.deepEqual(
    await reopenedMcp.tool('list_sessions'),
    [],
    'Session sharing must default to none on restart',
  );
  assert.match(await reopenedMcp.tool('search_requests', { sessionId }, true), /not shared/i);
  assert.match(await reopenedMcp.tool('get_request', { id: exchange.id }, true), /not shared/i);
  assert.equal(
    capturedRows(DatabaseSync, dataDirectory, sessionId).length,
    7,
    'Reopening must retain every exchange',
  );
  await reopenedMcp.close();
  await stop(reopened.child);
  console.log(
    'PASS graceful shutdown, SQLite persistence, same-directory restart and actual previously shared-session isolation',
  );
  completed = true;
  console.log(
    'Snurpo MCP smoke test passed. All traffic stayed on localhost; no Android device was modified.',
  );
} catch (error) {
  console.error(`FAIL ${error.stack ?? error}`);
  if (testDirectory && process.env.SCOPE_KEEP_SMOKE_DATA)
    console.error(`Preserved smoke data: ${testDirectory}`);
  for (const child of children) if (child.diagnostics) console.error(child.diagnostics);
  process.exitCode = 1;
} finally {
  await cleanup();
}
