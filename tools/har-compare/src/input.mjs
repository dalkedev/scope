import { queryIdentity } from './query.mjs';
import { parseJsonSafe } from '../json/parse.mjs';
import { AnalysisError } from './errors.mjs';
import { constants } from 'node:fs';
import { open } from 'node:fs/promises';
import { TextDecoder } from 'node:util';
import { fail, isObject, LIMITS } from './config.mjs';

const utf8 = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
/** Read at most limit + 1 bytes even if a file grows after stat; reject devices/FIFOs. */
export async function readJsonFile(path, limit, label) {
  let handle;
  try {
    handle = await open(path, constants.O_RDONLY | constants.O_NONBLOCK);
    const stat = await handle.stat();
    if (!stat.isFile() || stat.size > limit) fail('LIMIT', `${label} must be a regular file within its byte limit.`);
    const buffer = Buffer.alloc(limit + 1);
    let bytes = 0;
    while (bytes < buffer.length) {
      const chunk = await handle.read(buffer, bytes, buffer.length - bytes, null);
      if (!chunk.bytesRead) break;
      bytes += chunk.bytesRead;
    }
    if (bytes > limit) fail('LIMIT', `${label} exceeds its byte limit.`);
    const text = utf8.decode(buffer.subarray(0, bytes));
    return parseJson(text, label);
  } catch (error) {
    if (error instanceof AnalysisError) throw error;
    fail('INPUT', `${label} could not be read as UTF-8 JSON.`);
  } finally { await handle?.close(); }
}
/** Bound envelope nesting before allocating the parsed object graph, including unused HAR metadata. */
function boundDocumentNesting(text) {
  let quoted = false, depth = 0;
  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    if (quoted) {
      if (char === '\\') i++;
      else if (char === '"') quoted = false;
    } else if (char === '"') quoted = true;
    else if (char === '{' || char === '[') {
      if (++depth > 128) fail('LIMIT', 'JSON document exceeds the 128-level envelope nesting limit.');
    } else if (char === '}' || char === ']') depth--;
  }
}
function parseJson(text, label) {
  boundDocumentNesting(text);
  return parseJsonSafe(text.charCodeAt(0) === 0xfeff ? text.slice(1) : text, label);
}
function validateTree(value, budget, depth = 0) {
  if (depth > LIMITS.depth || ++budget.nodes > LIMITS.nodes) fail('LIMIT', 'JSON body exceeds depth or per-capture node limit.');
  if (typeof value === 'number' && (!Number.isFinite(value) || (Number.isInteger(value) && !Number.isSafeInteger(value)))) fail('NUMBER', 'JSON body contains an unsupported non-finite or unsafe integer.');
  if (value && typeof value === 'object') {
    for (const key of Object.keys(value)) {
      if (key.length > 1024) fail('LIMIT', 'JSON field name exceeds 1024 code units.');
      validateTree(value[key], budget, depth + 1);
    }
  }
}
function scopeEvidence(raw, index) {
  if (raw === undefined) return { entry: index };
  if (!isObject(raw)) fail('HAR', `Entry ${index}: invalid Snurpo extension.`);
  for (const key of ['completed', 'edited']) if (raw[key] !== undefined && typeof raw[key] !== 'boolean') fail('HAR', `Entry ${index}: invalid Snurpo flag.`);
  if (raw.completed === false || (raw.error !== undefined && raw.error !== null)) fail('INCOMPLETE', `Entry ${index}: incomplete or failed Snurpo capture.`);
  if (raw.provenance !== undefined && !['upstream', 'mock', 'blocked'].includes(raw.provenance)) fail('HAR', `Entry ${index}: invalid Snurpo provenance.`);
  const evidence = { entry: index };
  for (const key of ['provenance', 'edited']) if (raw[key] !== undefined) evidence[key] = raw[key];
  return evidence;
}
function body(response, method, index, budget) {
  const content = response.content;
  if (!isObject(content)) fail('BODY', `Entry ${index}: response content is unavailable.`);
  if (content.encoding !== undefined && content.encoding !== null && content.encoding !== 'base64') fail('ENCODING', `Entry ${index}: unsupported body encoding.`);
  if (content.size !== undefined && (!Number.isSafeInteger(content.size) || content.size < -1)) fail('BODY', `Entry ${index}: invalid declared content size.`);
  let decodedBytes;
  let text = content.text;
  if (text !== undefined && typeof text !== 'string') fail('BODY', `Entry ${index}: invalid response body.`);
  if (text !== undefined) {
    if (Buffer.byteLength(text) > Math.ceil(LIMITS.bodyBytes / 3) * 4) fail('LIMIT', `Entry ${index}: encoded body exceeds its byte limit.`);
    decodedBytes = Buffer.byteLength(text);
    if (content.encoding === 'base64') {
      // Buffer.from alone tolerates malformed base64. Require canonical padding/bits.
      if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(text)) fail('ENCODING', `Entry ${index}: malformed base64 body.`);
      const decoded = Buffer.from(text, 'base64');
      decodedBytes = decoded.length;
      if (decodedBytes > LIMITS.bodyBytes) fail('LIMIT', `Entry ${index}: body exceeds its decoded byte limit.`);
      if (decoded.toString('base64') !== text) fail('ENCODING', `Entry ${index}: non-canonical base64 body.`);
      try { text = utf8.decode(decoded); } catch { fail('ENCODING', `Entry ${index}: body is not valid UTF-8.`); }
    }
    if (Buffer.byteLength(text) > LIMITS.bodyBytes) fail('LIMIT', `Entry ${index}: body exceeds its decoded byte limit.`);
  }
  const noEntity = method === 'HEAD' || [204, 205, 304].includes(response.status);
  if (noEntity) {
    if (text) fail('BODY', `Entry ${index}: unexpected body for a bodyless response.`);
    return { kind: 'empty' };
  }
  if (text !== undefined && content.size !== undefined && content.size >= 0 && content.size !== decodedBytes) fail('INCOMPLETE', `Entry ${index}: declared content size differs from captured decoded bytes.`);
  if (typeof content.mimeType !== 'string') fail('BODY', `Entry ${index}: response MIME type is unavailable.`);
  const json = isJsonMime(content.mimeType);
  if (!json) {
    if (text === '' && content.size === 0) return { kind: 'empty' };
    fail('BODY', `Entry ${index}: only JSON or explicitly empty response bodies are supported.`);
  }
  if (text === undefined) fail('BODY', `Entry ${index}: JSON body was not captured.`);
  const value = parseJson(text, `Entry ${index} body`);
  validateTree(value, budget);
  return { kind: 'json', value };
}
export function harEntries(raw) {
  if (!isObject(raw) || !isObject(raw.log) || !Array.isArray(raw.log.entries)) fail('HAR', 'Expected a HAR object containing log.entries.');
  if (raw.log.entries.length > LIMITS.entries) fail('LIMIT', 'HAR exceeds 2000 entries.');
  return raw.log.entries;
}

/** Request-only identity for discovery and selection; raw query/userinfo stay internal. */
export function requestIdentity(item, index) {
  if (!isObject(item) || !isObject(item.request)) fail('HAR', `Entry ${index}: missing request.`);
  const { request } = item;
  if (typeof request.method !== 'string' || !/^[!#$%&'*+.^_`|~0-9A-Za-z-]{1,32}$/u.test(request.method)) fail('HAR', `Entry ${index}: invalid method.`);
  const method = request.method.toUpperCase();
  if (typeof request.url !== 'string' || request.url.length > 4096 || !request.url.isWellFormed() || /[\u0000-\u0020\u007f]/u.test(request.url)) fail('HAR', `Entry ${index}: invalid URL.`);
  let url;
  try { url = new URL(request.url); } catch { fail('HAR', `Entry ${index}: invalid URL.`); }
  if (!['http:', 'https:'].includes(url.protocol)) fail('HAR', `Entry ${index}: only HTTP(S) URLs are supported.`);
  return { method, url, query: queryIdentity(url.search) };
}

export function isJsonMime(mimeType) {
  if (typeof mimeType !== 'string') return false;
  return /^application\/(?:json|[a-z0-9!#$&^_.+-]+\+json)$/u.test(mimeType.split(';', 1)[0].trim().toLowerCase());
}

export function parseHar(raw, config = {}) {
  const rawEntries = harEntries(raw);
  const budget = { nodes: 0 };
  const entries = [], matchedSelectors = new Set();
  rawEntries.forEach((item, offset) => {
    const index = offset + 1;
    const { method, url, query } = requestIdentity(item, index);
    if (config.includeOperations) {
      const selector = config.includeOperations.findIndex(s => s.method === method && s.origin === url.origin && s.path === url.pathname);
      if (selector === -1) return;
      matchedSelectors.add(selector);
    }
    const response = item.response;
    if (!isObject(response)) fail('HAR', `Entry ${index}: missing response.`);
    if (!Number.isInteger(response.status) || response.status < 100 || response.status > 599) fail('HAR', `Entry ${index}: missing or invalid HTTP status.`);
    if (response.status < 200) fail('INCOMPLETE', `Entry ${index}: no supported final HTTP response was captured.`);
    const key = JSON.stringify([method, url.origin, url.pathname, query]);
    entries.push({ key, method, origin: url.origin, path: url.pathname, hasQuery: url.search !== '', status: response.status, body: body(response, method, index, budget), evidence: scopeEvidence(item._scope, index) });
  });
  return { entries, totalEntries: rawEntries.length, matchedSelectors };
}
