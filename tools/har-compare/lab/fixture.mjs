import { createServer } from 'node:http';
import { payloads } from './payloads.mjs';
import { jsonBoundaryBodies, scopeBoundaryScenarios, queryBoundaryCases } from './boundary-data.mjs';

export const scenarios = Object.freeze(['baseline', 'missing-field', 'http-500', 'timestamp']);
export const errorScenarios = Object.freeze(['http-500-html', 'empty-json']);
const knownScenarios = [...scenarios, ...errorScenarios];

/** A disposable, synthetic, loopback-only origin; no capture archive is opened. */
export async function startFixture() {
  const requests = [];
  const server = createServer((request, response) => {
    const url = new URL(request.url, 'http://127.0.0.1');
    const path = url.pathname;
    requests.push({ method: request.method, path, ...(path === '/api/query' ? { rawTarget: request.url } : {}) });
    const send = (status, type, body) => {
      response.writeHead(status, { 'content-type': type, 'cache-control': 'no-store' });
      response.end(body);
    };
    if (path === '/api/query') {
      if (!queryBoundaryCases.some(item => ['/api/query?' + item.before, '/api/query?' + item.after].includes(request.url))) return send(400, 'application/json', '{"error":"Unknown bounded query"}');
      return send(200, 'application/json', '{"value":"SYNTHETIC_QUERY_BODY_SECRET"}');
    }
    if (path.startsWith('/api/boundary/json/')) {
      const name = path.slice('/api/boundary/json/'.length), phase = request.headers['x-scope-phase'];
      if (!Object.hasOwn(jsonBoundaryBodies, name)) return send(404, 'application/json', '{"error":"Unknown bounded JSON case"}');
      if (!['baseline', 'candidate'].includes(phase)) return send(400, 'application/json', '{"error":"Unknown synthetic phase"}');
      return send(200, 'application/json', jsonBoundaryBodies[name][phase === 'baseline' ? 0 : 1]);
    }
    if (['/api/boundary/health', '/api/boundary/billing'].includes(path)) {
      const phase = request.headers['x-scope-phase'], scenario = request.headers['x-scope-boundary'];
      if (!['baseline', 'candidate'].includes(phase) || !scopeBoundaryScenarios.includes(scenario)) return send(400, 'application/json', '{"error":"Unknown bounded scope case"}');
      const stamp = phase === 'baseline' ? 'SYNTHETIC_STAMP_BEFORE_SECRET' : 'SYNTHETIC_STAMP_AFTER_SECRET';
      const body = { timestamp: stamp, meta: { timestamp: stamp } };
      const altered = phase === 'candidate' && path.endsWith('/health');
      if (altered && scenario === 'scoped-missing') delete body.timestamp;
      if (altered && scenario === 'scoped-parent') body.meta = null;
      return send(altered && scenario === 'scoped-http-500' ? 500 : 200, 'application/json', JSON.stringify(body));
    }
    if (path === '/api/payloads') {
      const phase = request.headers['x-scope-phase'];
      if (!['baseline', 'candidate'].includes(phase)) return send(400, 'application/json', '{"error":"Unknown synthetic phase"}');
      const body = phase === 'baseline' ? Object.fromEntries(payloads.map((payload, index) => [payload.key, `SYNTHETIC_PAYLOAD_BODY_SECRET_${index}`])) : {};
      return send(200, 'application/json', JSON.stringify(body));
    }
    if (path === '/api/generated') {
      const rawSeed = url.searchParams.get('seed'), seed = Number(rawSeed);
      const variant = request.headers['x-scope-variant'], phase = request.headers['x-scope-phase'];
      if (!/^[0-7]$/u.test(rawSeed || '') || url.searchParams.getAll('seed').length !== 1 || !['baseline', 'missing', 'type', 'value'].includes(variant) || !['baseline', 'candidate'].includes(phase)) return send(400, 'application/json', '{"error":"Invalid bounded synthetic variant"}');
      const key = ['role', 'a/b', 'tilde~key', '日本語', 'emoji-🧪', '<img src=x onerror=scopeLabExecuted=true>', '__proto__', 'constructor'][seed];
      const body = { record: { [key]: `SYNTHETIC_PRIVATE_VALUE_${seed}` }, noise: phase === 'baseline' ? '2026-11-03T00:00:00Z' : '2026-11-03T00:00:01Z' };
      if (variant === 'missing') delete body.record[key];
      if (variant === 'type') body.record[key] = seed;
      if (variant === 'value') body.record[key] = `SYNTHETIC_CHANGED_VALUE_${seed}`;
      return send(200, seed % 2 ? 'application/vnd.scope-lab+json; charset=utf-8' : 'application/json', JSON.stringify(body));
    }
    if (path === '/api/user') {
      const scenario = request.headers['x-scope-scenario'];
      if (!knownScenarios.includes(scenario)) return send(400, 'application/json', '{"error":"Unknown synthetic scenario"}');
      if (scenario === 'http-500-html') return send(500, 'text/html; charset=utf-8', '<h1>Synthetic server error</h1>');
      if (scenario === 'empty-json') return send(200, 'application/json', '');
      const body = { user: { name: 'Synthetic Alice', role: 'reader' }, timestamp: '2026-11-03T00:00:00Z' };
      if (scenario === 'missing-field') delete body.user.role;
      if (scenario === 'timestamp') body.timestamp = '2026-11-03T00:00:01Z';
      return send(scenario === 'http-500' ? 500 : 200, 'application/json', JSON.stringify(body));
    }
    if (path === '/fixture.css') return send(200, 'text/css', 'body{font:18px system-ui;margin:32px;background:#eef2f6}pre{white-space:pre-wrap}');
    if (path === '/fixture.svg') return send(200, 'image/svg+xml', '<svg xmlns="http://www.w3.org/2000/svg" width="96" height="32"><rect width="96" height="32" fill="#27856d"/></svg>');
    const scenario = path.slice('/scenario/'.length);
    if (path.startsWith('/scenario/') && knownScenarios.includes(scenario)) {
      return send(200, 'text/html; charset=utf-8', `<!doctype html><html lang="en"><meta charset="utf-8"><link rel="stylesheet" href="/fixture.css"><title>Snurpo synthetic browser fixture</title><h1>Snurpo local lab</h1><p>Synthetic scenario: ${scenario}</p><img src="/fixture.svg" alt="Synthetic fixture"><pre id="response">Loading</pre><script>fetch('/api/user',{headers:{'x-scope-scenario':${JSON.stringify(scenario)}}}).then(async r=>{document.querySelector('#response').textContent=JSON.stringify({status:r.status,body:await r.json()},null,2);document.documentElement.dataset.ready='true'}).catch(()=>{document.documentElement.dataset.ready='error'})</script></html>`);
    }
    send(404, 'text/plain', 'Synthetic fixture route not found');
  });
  server.requestTimeout = 5000;
  server.headersTimeout = 5000;
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  const { port } = server.address();
  return {
    origin: `http://127.0.0.1:${port}`,
    requests,
    close: () => new Promise((resolve, reject) => { server.close(error => error ? reject(error) : resolve()); server.closeAllConnections(); }),
  };
}
