// @vitest-environment node
import { execFile } from 'node:child_process';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { describe, expect, it } from 'vitest';
import { requestToCurl } from './curl';

// Executes only synthetic requests to a server owned by this test. No remote traffic.
describe.skipIf(process.platform === 'win32')('POSIX cURL loopback byte acceptance', () => {
  it('preserves text/binary bytes, URL, method and repeated/empty headers without executing captured syntax', async () => {
    const observed = [];
    const server = createServer(async (req, res) => {
      const chunks = [];
      for await (const chunk of req) chunks.push(Buffer.from(chunk));
      observed.push({
        method: req.method,
        url: req.url,
        headers: req.rawHeaders,
        body: Buffer.concat(chunks),
      });
      res.setHeader('Content-Length', '0');
      res.end();
    });
    server.listen(0, '127.0.0.1');
    await once(server, 'listening');
    const address = server.address();
    const target = `/a/../literal?array=[1]&quote='&shell=$(exit%209)`;
    const bodies = [
      Buffer.from("@file\nżółć '$(exit 9)`exit 9`\\\n"),
      Buffer.from([0, 255, 128, 10, 0]),
      Buffer.alloc(0),
      Buffer.alloc(0),
    ];
    try {
      for (const [i, body] of bodies.entries()) {
        const source = {
          id: 'synthetic',
          sessionId: 'synthetic',
          method: ['POST', 'GET', 'POST', 'HEAD'][i],
          url: `http://127.0.0.1:${address.port}${target}`,
          requestHeaders: [
            { name: 'X-Test', value: "'$(exit 9)`exit 9`" },
            { name: 'X-Test', value: 'second' },
            { name: 'X-Empty', value: '' },
            { name: 'X-Blank', value: ' \t' },
            { name: 'Host', value: 'stale.invalid' },
            { name: 'Content-Length', value: '999' },
            { name: 'Connection', value: 'X-Remove' },
            { name: 'X-Remove', value: 'stale' },
          ],
          requestBody: i ? body.toString('base64') : body.toString('utf8'),
          requestBodyBase64: !!i,
          responseHeaders: [],
          responseBody: '',
          responseBodyBase64: false,
          status: 200,
          startedAt: 0,
          durationMs: 0,
          requestBytes: body.length,
          responseBytes: 0,
          error: null,
          completed: true,
          edited: false,
        };
        const command = requestToCurl(source);
        await new Promise((resolve, reject) => {
          execFile(
            'sh',
            ['-c', command],
            {
              timeout: 5000,
              env: {
                ...process.env,
                http_proxy: '',
                https_proxy: '',
                all_proxy: '',
                HTTP_PROXY: '',
                HTTPS_PROXY: '',
                ALL_PROXY: '',
                NO_PROXY: '127.0.0.1',
              },
            },
            (error) => (error ? reject(error) : resolve()),
          );
        });
        expect(observed[i].body).toEqual(body);
        expect(observed[i].method).toBe(source.method);
        expect(observed[i].url).toBe(target);
        const pairs = observed[i].headers.flatMap((name, index, list) =>
          index % 2 ? [] : [[name.toLowerCase(), list[index + 1]]],
        );
        expect(pairs.filter(([name]) => name === 'x-test').map(([, value]) => value)).toEqual([
          "'$(exit 9)`exit 9`",
          'second',
        ]);
        expect(pairs).toContainEqual(['x-empty', '']);
        expect(pairs).toContainEqual(['x-blank', '']);
        expect(pairs).toContainEqual(['content-length', String(body.length)]);
        expect(pairs).toContainEqual(['host', `127.0.0.1:${address.port}`]);
        expect(pairs).not.toContainEqual(['x-remove', 'stale']);
        for (const header of ['accept', 'user-agent', 'content-type', 'expect'])
          expect(pairs.some(([name]) => name === header)).toBe(false);
      }
    } finally {
      server.closeAllConnections();
      await new Promise((resolve) => server.close(() => resolve()));
    }
  });
});
