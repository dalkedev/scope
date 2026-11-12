import { test } from 'node:test';
import assert from 'node:assert/strict';
import { startFixture } from '../fixture.mjs';
import { captureWithNode, captureGenerated, capturePayloads } from '../capture.mjs';

test('Node capture records actual localhost API and non-JSON HTTP bodies', async t => {
  const fixture = await startFixture(); t.after(() => fixture.close());
  const har = await captureWithNode(fixture.origin, 'missing-field');
  assert.equal(har?.log?.entries?.length, 4, 'one document, CSS, SVG and API response must be captured');
  assert.equal(har.log.creator.name, 'Snurpo synthetic Node lab (not browser capture)');
  assert.deepEqual(har.log.entries.map(e => e.response.content.mimeType), ['text/html; charset=utf-8', 'text/css', 'image/svg+xml', 'application/json']);
  const entry = har.log.entries[3];
  assert.equal(entry.request.url, fixture.origin + '/api/user');
  assert.equal(entry.response.status, 200);
  assert.equal(Object.hasOwn(JSON.parse(entry.response.content.text).user, 'role'), false);
  assert.equal(entry.response.content.size, Buffer.byteLength(entry.response.content.text));
});

test('capture rejects noncanonical or off-host destinations before HTTP traffic', async () => {
  for (const origin of ['https://127.0.0.1:1234', 'http://example.invalid', 'http://localhost:1234', 'http://127.0.0.1:1234/path', 'http://user:pass@127.0.0.1:1234', 'http://127.0.0.1:1234/']) {
    await assert.rejects(() => captureWithNode(origin, 'baseline'), /lab loopback origin/);
  }
  await assert.rejects(() => captureWithNode('http://127.0.0.1:1234', 'not-a-fixture'), /known synthetic scenario/);
});


test('bounded seed capture records real repeated responses with reordered query parameters', async t => {
  const fixture = await startFixture(); t.after(() => fixture.close());
  const before = await captureGenerated(fixture.origin, 'baseline');
  const after = await captureGenerated(fixture.origin, 'candidate');
  assert.equal(before?.log?.entries?.length, 16, 'eight seeds need two real responses each');
  assert.equal(after.log.entries.length, 16);
  assert.equal(fixture.requests.length, 32);
  assert.ok(before.log.entries[0].request.url.includes('?seed=0&lab=1'));
  assert.ok(after.log.entries[0].request.url.includes('?lab=1&seed=0'));
  assert.deepEqual(JSON.parse(after.log.entries[3].response.content.text).record, {});
  assert.equal(JSON.parse(before.log.entries[3].response.content.text).record['a/b'], 'SYNTHETIC_PRIVATE_VALUE_1');
});


test('payload recorder preserves hostile JSON keys in real HTTP response bytes', async t => {
  const fixture = await startFixture(); t.after(() => fixture.close());
  const har = await capturePayloads(fixture.origin, 'baseline');
  assert.equal(har?.log?.entries?.length, 1, 'payload recorder must capture one real HTTP exchange');
  const entry = har.log.entries[0];
  assert.equal(entry.response.content.size, Buffer.byteLength(entry.response.content.text));
  assert.equal(Object.keys(JSON.parse(entry.response.content.text)).length, 12);
});
