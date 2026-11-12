import type { Exchange } from './types';
import { validateHeaders, validateMethod, validateUrl } from './form-utils';

const LIMIT = 128 * 1024 - 1;
const TOO_LARGE =
  'This request is too large for a cURL command (128 KiB limit). Use HAR export or Compose replay.';
const quote = (value: string) => `'${value.replace(/'/g, `'"'"'`)}'`;
const transportHeaders = new Set([
  'host',
  'content-length',
  'connection',
  'proxy-connection',
  'proxy-authenticate',
  'proxy-authorization',
  'keep-alive',
  'te',
  'trailer',
  'transfer-encoding',
  'upgrade',
]);

/** A local export only: never executes cURL or changes the source exchange. */
export function requestToCurl(exchange: Exchange): string {
  // Bound input before validation, decoding, quoting or octal expansion.
  let inputSize = exchange.url.length + exchange.method.length + exchange.requestBody.length;
  if (inputSize > LIMIT || exchange.requestHeaders.length > 1024) throw new Error(TOO_LARGE);
  for (const header of exchange.requestHeaders) {
    inputSize += header.name.length + header.value.length;
    if (inputSize > LIMIT) throw new Error(TOO_LARGE);
  }
  if (exchange.method === 'HEAD' && exchange.requestBody !== '')
    throw new Error('HEAD requests with bodies cannot be exported as cURL. Use Compose replay.');
  validateMethod(exchange.method);
  validateUrl(exchange.url);
  if (!/^https?:\/\//i.test(exchange.url) || /[\x00-\x20\x7f\\]/.test(exchange.url))
    throw new Error('The request URL contains unsupported whitespace or control characters.');
  validateHeaders(exchange.requestHeaders);
  if (exchange.requestHeaders.some((h) => /[\x00-\x08\x0b-\x1f\x7f]/.test(h.value)))
    throw new Error('Header values contain unsupported control characters.');
  if (/[\uD800-\uDFFF]/u.test(exchange.requestBody))
    throw new Error('The request body contains invalid Unicode.');

  const omitted = new Set(transportHeaders);
  for (const header of exchange.requestHeaders) {
    if (header.name.toLowerCase() === 'connection') {
      for (const name of header.value.split(',')) omitted.add(name.trim().toLowerCase());
    }
  }
  const headers = exchange.requestHeaders.filter((h) => !omitted.has(h.name.toLowerCase()));
  const args = [
    "curl --disable --globoff --path-as-is --proto '=http,https'",
    `--request ${quote(exchange.method)}`,
  ];
  if (exchange.method === 'HEAD') args.push('--head');
  for (const header of headers) {
    args.push(
      `--header ${quote(/^[ \t]*$/.test(header.value) ? `${header.name};` : `${header.name}: ${header.value}`)}`,
    );
  }
  // cURL otherwise adds these headers, which may not be in the capture.
  for (const name of ['Accept', 'User-Agent', 'Content-Type', 'Expect']) {
    if (!headers.some((h) => h.name.toLowerCase() === name.toLowerCase()))
      args.push(`--header ${quote(`${name}:`)}`);
  }

  if (
    exchange.requestBody === '' &&
    exchange.requestHeaders.some((h) =>
      ['content-length', 'transfer-encoding'].includes(h.name.toLowerCase()),
    )
  ) {
    args.push("--header 'Content-Length: 0'");
  }
  let prefix = '';
  if (exchange.requestBody !== '') {
    let body = exchange.requestBody;
    let binary = exchange.requestBodyBase64;
    if (binary) {
      if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(body))
        throw new Error('The request body is not valid base64.');
      body = atob(body);
      if (btoa(body) !== exchange.requestBody)
        throw new Error('The request body is not valid base64.');
    } else if (body.includes('\0')) {
      body = Array.from(new TextEncoder().encode(body), (byte) => String.fromCharCode(byte)).join(
        '',
      );
      binary = true;
    }
    if (binary) {
      if (body.length * 5 > LIMIT) throw new Error(TOO_LARGE);
      // POSIX printf %b accepts \0ddd on Linux and macOS, including NUL bytes.
      const octal = Array.from(
        body,
        (char) => `\\0${char.charCodeAt(0).toString(8).padStart(3, '0')}`,
      ).join('');
      prefix = `printf '%b' ${quote(octal)} | `;
    } else {
      prefix = `printf '%s' ${quote(body)} | `;
    }
    args.push('--data-binary @-');
  }
  args.push(`--url ${quote(exchange.url)}`);
  const command = prefix + args.join(' \\\n  ');
  if (new TextEncoder().encode(command).length > LIMIT) throw new Error(TOO_LARGE);
  return command;
}
