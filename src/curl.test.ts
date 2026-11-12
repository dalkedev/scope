import { describe, expect, it } from 'vitest';
import { requestToCurl } from './curl';
import type { Exchange } from './types';
export const request = (overrides: Partial<Exchange> = {}): Exchange => ({
  id: 'test',
  sessionId: 'session',
  method: 'GET',
  url: 'https://example.com/a?x=[1]',
  requestHeaders: [],
  responseHeaders: [],
  requestBody: '',
  responseBody: '',
  requestBodyBase64: false,
  responseBodyBase64: false,
  status: 200,
  startedAt: 0,
  durationMs: 1,
  requestBytes: 0,
  responseBytes: 0,
  error: null,
  completed: true,
  edited: false,
  ...overrides,
});
describe('requestToCurl', () => {
  it('restricts protocols, disables curlrc and globbing, and preserves literal URL', () => {
    const command = requestToCurl(request());
    expect(command).toContain("curl --disable --globoff --path-as-is --proto '=http,https'");
    expect(command).toContain("--request 'GET'");
    expect(command).toContain("--url 'https://example.com/a?x=[1]'");
    expect(command).not.toMatch(/--location|--insecure|--retry/);
  });
  it('quotes shell syntax and sends leading @ and trailing newlines as literal stdin', () => {
    const body = "@file\n'$(exit 9)`exit 9`\\\n";
    const source = request({ method: 'POST', requestBody: body });
    expect(requestToCurl(source)).toContain("printf '%s' '@file\n'\"'\"'$(exit 9)`exit 9`\\\n'");
    expect(requestToCurl(source)).toContain('--data-binary @-');
    expect(source.requestBody).toBe(body);
  });
  it('keeps repeated/empty headers but removes stale framing and connection extensions', () => {
    const command = requestToCurl(
      request({
        requestHeaders: [
          { name: 'X-Tag', value: 'a' },
          { name: 'X-Tag', value: '' },
          { name: 'X-Blank', value: ' \t' },
          { name: 'Connection', value: 'X-Transport' },
          { name: 'x-transport', value: 'stale' },
          { name: 'Host', value: 'old.example' },
          { name: 'Content-Length', value: '900' },
          { name: 'Transfer-Encoding', value: 'chunked' },
          { name: 'Proxy-Authorization', value: 'secret' },
        ],
      }),
    );
    expect(command).toContain("--header 'X-Tag: a'");
    expect(command).toContain("--header 'X-Tag;'");
    expect(command).toContain("--header 'X-Blank;'");
    expect(command).not.toMatch(/stale|old.example|900|chunked|secret/);
    for (const name of ['Accept', 'User-Agent', 'Content-Type', 'Expect'])
      expect(command).toContain(`--header '${name}:'`);
  });
  it('uses POSIX octal printf for binary and embedded NUL bytes', () => {
    expect(requestToCurl(request({ requestBody: 'AP+A', requestBodyBase64: true }))).toContain(
      "printf '%b' '\\0000\\0377\\0200'",
    );
    expect(requestToCurl(request({ requestBody: 'a\0b' }))).toContain(
      "printf '%b' '\\0141\\0000\\0142'",
    );
  });
  it('regenerates explicit zero-length framing for bodyless POST and HEAD', () => {
    for (const method of ['POST', 'HEAD']) {
      const command = requestToCurl(
        request({ method, requestHeaders: [{ name: 'Content-Length', value: '0' }] }),
      );
      expect(command).toContain("--header 'Content-Length: 0'");
      expect(command).not.toContain('--data-binary');
    }
  });
  it('uses HEAD handling without changing custom method case', () => {
    expect(requestToCurl(request({ method: 'HEAD' }))).toContain('--head');
    expect(requestToCurl(request({ method: 'Custom' }))).toContain("--request 'Custom'");
  });
  it('rejects HEAD bodies rather than emitting incompatible cURL flags', () => {
    expect(() => requestToCurl(request({ method: 'HEAD', requestBody: 'body' }))).toThrow(/HEAD/);
  });
  it.each([
    { url: 'file:///tmp/input' },
    { url: 'https://user:pass@example.com' },
    { url: 'https://example.com/\nattack' },
    { method: 'GET\n-X POST' },
    { requestHeaders: [{ name: 'X-Test', value: 'a\r\nb' }] },
    { requestHeaders: [{ name: 'bad name', value: 'x' }] },
    { requestBody: 'not base64!', requestBodyBase64: true },
    { requestBody: 'x'.repeat(131073) },
    { requestBody: 'A'.repeat(131073), requestBodyBase64: true },
  ])('rejects malformed or oversized requests', (overrides) => {
    expect(() => requestToCurl(request(overrides))).toThrow();
  });
});
