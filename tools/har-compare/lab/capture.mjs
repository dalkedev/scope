import { scenarios, errorScenarios } from './fixture.mjs';
import { jsonBoundaryBodies, scopeBoundaryScenarios, queryBoundaryCases } from './boundary-data.mjs';

const utf8 = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

function validateOrigin(origin) {
  const url = new URL(origin);
  if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || url.origin !== origin) throw new Error('Capture accepts only a lab loopback origin and known synthetic scenario.');
}
function har(entries) {
  return { log: { version: '1.2', creator: { name: 'Snurpo synthetic Node lab (not browser capture)', version: '0.1.0' }, entries } };
}
async function record(origin, path, headers) {
  const started = new Date().toISOString();
  const response = await fetch(origin + path, { headers, signal: AbortSignal.timeout(5000), redirect: 'error' });
  const bytes = Buffer.from(await response.arrayBuffer());
  let text, encoding;
  try { text = utf8.decode(bytes); }
  catch { text = bytes.toString('base64'); encoding = 'base64'; }
  return {
    startedDateTime: started, time: 0,
    request: { method: 'GET', url: origin + path, httpVersion: 'HTTP/1.1', headers: Object.entries(headers).map(([name, value]) => ({ name, value })), cookies: [], queryString: [...new URL(origin + path).searchParams].map(([name, value]) => ({ name, value })), headersSize: -1, bodySize: 0 },
    response: { status: response.status, statusText: response.statusText, httpVersion: 'HTTP/1.1', headers: [...response.headers].map(([name, value]) => ({ name, value })), cookies: [], content: { mimeType: response.headers.get('content-type'), text, ...(encoding ? { encoding } : {}), size: bytes.length }, redirectURL: '', headersSize: -1, bodySize: bytes.length },
    cache: {}, timings: { send: 0, wait: 0, receive: 0 },
  };
}
/** Records HTTP exchanges from Node fetch; deliberately not called browser capture. */
export async function captureWithNode(origin, scenario) {
  validateOrigin(origin);
  if (![...scenarios, ...errorScenarios].includes(scenario)) throw new Error('Capture accepts only a lab loopback origin and known synthetic scenario.');
  const entries = [];
  for (const path of ['/scenario/' + scenario, '/fixture.css', '/fixture.svg', '/api/user']) entries.push(await record(origin, path, path === '/api/user' ? { 'x-scope-scenario': scenario } : {}));
  return har(entries);
}
/** Fixed eight-seed corpus; each response is fetched, never synthesized after capture. */
export async function captureGenerated(origin, phase) {
  validateOrigin(origin);
  if (!['baseline', 'candidate'].includes(phase)) throw new Error('Generated capture requires a known synthetic phase.');
  const entries = [];
  for (let seed = 0; seed < 8; seed++) {
    for (let occurrence = 0; occurrence < 2; occurrence++) {
      const variant = phase === 'baseline' || occurrence === 0 ? 'baseline' : ['baseline', 'missing', 'type', 'value'][seed % 4];
      const query = phase === 'baseline' ? `seed=${seed}&lab=1&private=SYNTHETIC_QUERY_SECRET` : `lab=1&seed=${seed}&private=SYNTHETIC_QUERY_SECRET`;
      const entry = await record(origin, '/api/generated?' + query, { 'x-scope-variant': variant, 'x-scope-phase': phase, Authorization: 'Bearer SYNTHETIC_HEADER_SECRET' });
      entry._lab = { seed, phase, occurrence, unrelatedMetadata: '[brackets] and \\"quotes\\" are data' };
      entries.push(entry);
    }
  }
  return har(entries);
}

export async function capturePayloads(origin, phase) {
  validateOrigin(origin);
  if (!['baseline', 'candidate'].includes(phase)) throw new Error('Payload capture requires a known synthetic phase.');
  return har([await record(origin, '/api/payloads?token=SYNTHETIC_PAYLOAD_QUERY_SECRET', { 'x-scope-phase': phase, Authorization: 'Bearer SYNTHETIC_PAYLOAD_HEADER_SECRET' })]);
}

/** Raw JSON boundary variants remain response bytes, including invalid JSON. */
export async function captureBoundary(origin, name, phase) {
  validateOrigin(origin);
  if (!['baseline', 'candidate'].includes(phase)) throw new Error('Boundary capture requires a known synthetic phase.');
  const scoped = scopeBoundaryScenarios.includes(name);
  if (!scoped && !Object.hasOwn(jsonBoundaryBodies, name)) throw new Error('Boundary capture requires a known bounded case.');
  const paths = scoped ? ['/api/boundary/health', '/api/boundary/billing'] : ['/api/boundary/json/' + name];
  const entries = [];
  for (const path of paths) entries.push(await record(origin, path + '?private=SYNTHETIC_BOUNDARY_QUERY_SECRET', { 'x-scope-phase': phase, 'x-scope-boundary': name, Authorization: 'Bearer SYNTHETIC_BOUNDARY_HEADER_SECRET' }));
  return har(entries);
}

/** Preserves request percent bytes while recording the actual HTTP response. */
export async function captureQuery(origin, name, phase) {
  validateOrigin(origin);
  if (!['baseline', 'candidate'].includes(phase)) throw new Error('Query capture requires a known synthetic phase.');
  const item = queryBoundaryCases.find(item => item.name === name);
  if (!item) throw new Error('Query capture requires a known bounded case.');
  return har([await record(origin, '/api/query?' + item[phase === 'baseline' ? 'before' : 'after'], { Authorization: 'Bearer SYNTHETIC_QUERY_HEADER_SECRET' })]);
}
