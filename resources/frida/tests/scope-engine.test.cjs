'use strict';
// Java/Frida doubles exercise Scope policy and API wiring, not Android execution.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
let engine;
try {
  const context = { module: { exports: {} } };
  vm.runInNewContext(fs.readFileSync(require('node:path').join(__dirname, '../scope-engine.js'), 'utf8'), context);
  engine = context.module.exports;
} catch (error) { if (error.code !== 'ENOENT') throw error; }
const api = () => { assert.ok(engine, 'Scope-owned Java engine is implemented'); return engine; };
const plain = value => JSON.parse(JSON.stringify(value));
function javaError(javaType, message) { const error = new Error(message); error.$h = { javaType }; return error; }

test('profiles default, deduplicate and reject unsupported or missing dependencies', () => {
  const { normalizeProfiles } = api();
  assert.deepEqual(plain(normalizeProfiles([])), ['proxy', 'certificate', 'unpinning']);
  assert.deepEqual(plain(normalizeProfiles(['certificate', 'proxy', 'certificate'])), ['proxy', 'certificate']);
  for (const value of [['root'], ['system-ca'], ['other'], ['unpinning'], 'proxy', [null]]) {
    assert.throws(() => normalizeProfiles(value), /profile|certificate/i);
  }
});

test('pins skip only a chain accepted by the isolated CA validator; preserve original rejection identity', () => {
  const { createPolicy } = api();
  const valid = {}, unrelated = {}, expired = {}, malformed = {};
  const rejection = new Error('original pin mismatch');
  const checked = [];
  const policy = createPolicy(chain => { checked.push(chain); if (chain !== valid) throw new Error('PKIX rejection'); }, { isPinRejection: error => error === rejection });
  let originalCalls = 0;
  const original = () => { originalCalls++; throw rejection; };
  assert.equal(policy.checkPins(valid, original), undefined);
  for (const chain of [unrelated, expired, malformed, null]) {
    assert.throws(() => policy.checkPins(chain, original), error => error === rejection);
  }
  assert.equal(originalCalls, 5);
  assert.equal(checked.length, 5);
});

test('trust delegates original decisions and preserves original errors for invalid Scope chains', () => {
  const { createPolicy } = api();
  const valid = {}, rejection = new Error('original trust rejection');
  const policy = createPolicy(chain => { if (chain !== valid) throw new Error('invalid'); }, { isCertificateRejection: error => error === rejection });
  assert.equal(policy.checkTrust({}, () => 'original acceptance'), 'original acceptance');
  assert.equal(policy.checkTrust(valid, () => { throw rejection; }), undefined);
  assert.throws(() => policy.checkTrust({}, () => { throw rejection; }), error => error === rejection);
});

test('missing requested capabilities fail transactionally and restore earlier hooks', () => {
  const { createEngine } = api();
  const method = { implementation: 'original' };
  const adapter = {
    installProxy(transaction) { transaction.hook(method, () => 'proxy'); return ['java-url']; },
    installCertificate() { throw new Error('SSLContext.init overload missing'); },
  };
  assert.throws(() => createEngine({ profiles: ['proxy', 'certificate'] }, adapter).install(), /SSLContext.init/);
  assert.equal(method.implementation, 'original');
  assert.throws(() => createEngine({ profiles: ['proxy'] }, { installProxy() { return []; } }).install(), /capabilit/i);
});

test('readiness reports exactly installed requested capabilities and rollback is explicit', () => {
  const { createEngine } = api();
  const method = { implementation: null };
  const instance = createEngine({ profiles: ['proxy'] }, {
    installProxy(transaction) { transaction.hook(method, () => 'proxy'); return ['java-url']; },
  });
  assert.deepEqual(plain(instance.install()), { kind: 'scope-hooks-ready', engine: 'scope-java-v1', profiles: ['proxy'], capabilities: { proxy: ['java-url'] } });
  instance.rollback();
  assert.equal(method.implementation, null);
});

function javaFixture() {
  const calls = [];
  const ca = { checkValidity() { calls.push('ca-valid'); }, getBasicConstraints() { return 1; } };
  const scopeTrust = { checkServerTrusted(chain, auth) { calls.push(['validate', chain, auth]); if (chain.some(c => c.unrelated)) throw new Error('bad signature'); } };
  const x509 = { class: { isInstance(value) { return value === scopeTrust; } } };
  const store = { load(stream, pass) { calls.push(['empty-store', stream, pass]); }, setCertificateEntry(name, cert) { calls.push(['anchor', name, cert]); } };
  const classes = {
    'java.lang.Throwable': {},
    'java.security.cert.CertificateException': { class: { isInstance: value => value.javaType === 'certificate' } },
    'javax.net.ssl.SSLPeerUnverifiedException': { class: { isInstance: value => value.javaType === 'pin' } },
    'java.lang.String': { $new(value) { return { getBytes() { return value; } }; } },
    'java.io.ByteArrayInputStream': { $new() { return {}; } },
    'java.security.cert.CertificateFactory': { getInstance() { return { generateCertificate() { return ca; } }; } },
    'java.security.KeyStore': { getDefaultType() { return 'BKS'; }, getInstance() { return store; } },
    'java.security.Key': {},
    'javax.net.ssl.TrustManagerFactory': { getDefaultAlgorithm() { return 'PKIX'; }, getInstance() { return { init(value) { calls.push(['tm-init', value]); }, getTrustManagers() { return [scopeTrust]; } }; } },
    'javax.net.ssl.X509TrustManager': x509,
    'java.security.cert.X509Certificate': { class: { isInstance(value) { return Boolean(value && value.checkValidity); } } },
  };
  const Java = {
    use(name) { if (!classes[name]) throw new Error('Missing class ' + name); return classes[name]; },
    cast(value, type) {
      if (type === classes['java.security.Key']) {
        calls.push(['key-cast', value]);
        return { getAlgorithm() { return value.$h.algorithm; } };
      }
      return value;
    },
    array(type, value) { return value; }
  };
  return { Java, calls, ca, scopeTrust, store, classes };
}
// Model the observed Frida PublicKey wrapper: Key exposes the inherited method.
function leaf(options = {}) { return { checkValidity() { if (options.expired) throw new Error('expired'); }, getBasicConstraints() { return -1; }, getPublicKey() { return { $h: { algorithm: 'RSA' } }; }, ...options }; }

test('Java validator uses empty keystore with only Scope CA and validates peer signatures and validity', () => {
  const { createJavaAdapter } = api();
  const fixture = javaFixture();
  const adapter = createJavaAdapter(fixture.Java, { caPem: 'public certificate', proxyHost: '127.0.0.1', proxyPort: 17890 });
  const good = leaf();
  adapter.validateScopeChain([good]);
  assert.deepEqual(fixture.calls.find(call => Array.isArray(call) && call[0] === 'empty-store'), ['empty-store', null, null]);
  assert.deepEqual(fixture.calls.filter(call => Array.isArray(call) && call[0] === 'anchor'), [['anchor', 'scope', fixture.ca]]);
  assert.deepEqual(plain(fixture.calls.find(call => Array.isArray(call) && call[0] === 'validate')), ['validate', [{}], 'RSA']);
  assert.throws(() => adapter.validateScopeChain([leaf({ unrelated: true }), fixture.ca]), /bad signature/);
  assert.throws(() => adapter.validateScopeChain([leaf({ expired: true })]), /expired/);
  assert.throws(() => adapter.validateScopeChain([fixture.ca]), /leaf/);
  assert.throws(() => adapter.validateScopeChain([]), /empty|chain/i);
  assert.throws(() => adapter.validateScopeChain([{}]), /certificate/i);
});

test('real adapter fails closed on missing Java classes and missing OkHttp pinner', () => {
  const { createJavaAdapter, createEngine } = api();
  const fixture = javaFixture();
  const adapter = createJavaAdapter(fixture.Java, { caPem: 'public certificate', proxyHost: '127.0.0.1', proxyPort: 17890 });
  assert.throws(() => createEngine({ profiles: ['proxy'] }, adapter).install(), /java.net|Missing class/);
  assert.throws(() => adapter.installUnpinning({ hook() {} }), /okhttp3.CertificatePinner/);
});

function method(signature, original = () => undefined) {
  return { argumentTypes: signature.map(className => ({ className })), implementation: null, call(receiver, ...args) { return original.apply(receiver, args); } };
}
function dispatcher(...methods) {
  return { overloads: methods, overload(...signature) {
    const found = methods.find(m => JSON.stringify(m.argumentTypes.map(t => t.className)) === JSON.stringify(signature));
    if (!found) throw new Error('overload missing');
    return found;
  } };
}
function tlsFixture(extended = false) {
  const f = javaFixture();
  const calls = [];
  const rejection = javaError('certificate', 'original rejected');
  const original = { extended, getAcceptedIssuers() { return []; }, checkClientTrusted(...args) { calls.push(['client', ...args]); }, checkServerTrusted(...args) { calls.push(['server', ...args]); throw rejection; } };
  for (const name of ['checkClientTrusted', 'checkServerTrusted']) {
    const dynamic = original[name];
    Object.assign(dynamic, dispatcher(...[null, 'java.net.Socket', 'javax.net.ssl.SSLEngine'].map(type => method(['[Ljava.security.cert.X509Certificate;', 'java.lang.String'].concat(type ? [type] : []), dynamic))));
  }
  f.classes['javax.net.ssl.X509TrustManager'].class.isInstance = value => value === f.scopeTrust || value === original;
  f.classes['javax.net.ssl.X509ExtendedTrustManager'] = { class: { isInstance: value => Boolean(value.extended) } };
  f.scopeTrust.extended = extended;
  let wrapped;
  const init = method(['[Ljavax.net.ssl.KeyManager;', '[Ljavax.net.ssl.TrustManager;', 'java.security.SecureRandom'], function (_keys, managers) { wrapped = managers[0]; });
  f.classes['javax.net.ssl.SSLContext'] = { init: dispatcher(init) };
  let definition;
  f.Java.registerClass = spec => {
    definition = spec;
    return { $new() {
      const object = { delegate: { value: null } };
      for (const [name, entry] of Object.entries(spec.methods)) {
        object[name] = Array.isArray(entry) ? (...args) => entry.find(m => m.argumentTypes.length === args.length && (args.length !== 3 || m.argumentTypes[2] === args[2].type)).implementation.apply(object, args) : (typeof entry === 'function' ? entry : entry.implementation).bind(object);
      }
      return object;
    } };
  };
  return { ...f, init, original, calls, rejection, get wrapped() { return wrapped; }, get definition() { return definition; } };
}

test('SSLContext adds Scope trust while preserving client checks, original errors and issuer inputs', () => {
  const { createJavaAdapter, createEngine } = api();
  const f = tlsFixture();
  createEngine({ profiles: ['certificate'] }, createJavaAdapter(f.Java, { caPem: 'public certificate' })).install();
  f.init.implementation(null, [f.original], null);
  const valid = [leaf()];
  assert.equal(f.wrapped.checkServerTrusted(valid, 'RSA'), undefined);
  assert.throws(() => f.wrapped.checkServerTrusted([leaf({ unrelated: true })], 'RSA'), e => e === f.rejection);
  f.wrapped.checkClientTrusted(valid, 'RSA');
  assert.equal(f.calls.at(-1)[0], 'client');
  assert.equal(f.calls.at(-1)[1], valid);
  assert.equal(f.wrapped.getAcceptedIssuers()[0], f.ca);
});

test('extended trust preserves socket and engine context and scope context rejection', () => {
  const { createJavaAdapter, createEngine } = api();
  const f = tlsFixture(true);
  const contexts = [];
  const basicValidation = f.scopeTrust.checkServerTrusted;
  f.scopeTrust.checkServerTrusted = dispatcher(
    method(['[Ljava.security.cert.X509Certificate;', 'java.lang.String'], basicValidation),
    method(['[Ljava.security.cert.X509Certificate;', 'java.lang.String', 'java.net.Socket'], (_chain, _auth, context) => { contexts.push(context); throw new Error('hostname mismatch'); }),
    method(['[Ljava.security.cert.X509Certificate;', 'java.lang.String', 'javax.net.ssl.SSLEngine'], (_chain, _auth, context) => { contexts.push(context); })
  );
  createEngine({ profiles: ['certificate'] }, createJavaAdapter(f.Java, { caPem: 'public certificate' })).install();
  f.init.implementation(null, [f.original], null);
  assert.equal(f.definition.superClass, f.classes['javax.net.ssl.X509ExtendedTrustManager']);
  const socket = { type: 'java.net.Socket' }, tlsEngine = { type: 'javax.net.ssl.SSLEngine' };
  assert.throws(() => f.wrapped.checkServerTrusted([leaf()], 'RSA', socket), e => e === f.rejection);
  assert.equal(f.wrapped.checkServerTrusted([leaf()], 'RSA', tlsEngine), undefined);
  assert.equal(contexts[0], socket);
  assert.equal(contexts[1], tlsEngine);
  assert.equal(f.calls[0][3], socket);
  assert.equal(f.calls[1][3], tlsEngine);
});

test('URL proxy hooks redirect HTTP only and rollback restores original implementations', () => {
  const { createJavaAdapter, createEngine } = api();
  const f = javaFixture();
  const scopeProxy = {};
  const plainMethod = method([], () => 'original non-http');
  const explicit = method(['java.net.Proxy'], selected => selected);
  f.classes['java.net.URL'] = { openConnection: dispatcher(plainMethod, explicit) };
  f.classes['java.net.Proxy'] = { $new(_type, address) { assert.equal(address.host, '127.0.0.1'); assert.equal(address.port, 17890); return scopeProxy; } };
  f.classes['java.net.Proxy$Type'] = { HTTP: { value: 'HTTP' } };
  f.classes['java.net.InetSocketAddress'] = { $new(host, port) { return { host, port }; } };
  const instance = createEngine({ profiles: ['proxy'] }, createJavaAdapter(f.Java, { proxyHost: '127.0.0.1', proxyPort: 17890 }));
  instance.install();
  assert.equal(plainMethod.implementation.call({ getProtocol: () => 'https' }), scopeProxy);
  assert.equal(plainMethod.implementation.call({ getProtocol: () => 'file' }), 'original non-http');
  const custom = {};
  assert.equal(explicit.implementation.call({ getProtocol: () => 'file' }, custom), custom);
  instance.rollback();
  assert.equal(plainMethod.implementation, null);
  assert.equal(explicit.implementation, null);
});

test('OkHttp actual hook preserves unrelated pin exception and rejects unsupported overload transactionally', () => {
  const { createJavaAdapter, createEngine } = api();
  const f = javaFixture();
  const rejection = javaError('pin', 'pin mismatch');
  const check = method(['java.lang.String', 'java.util.List'], () => { throw rejection; });
  f.classes['okhttp3.CertificatePinner'] = { check: dispatcher(check) };
  const adapter = createJavaAdapter(f.Java, { caPem: 'public certificate' });
  // Certificate profile orchestration is tested separately; install the actual pin adapter here.
  const instance = createEngine({ profiles: ['certificate', 'unpinning'] }, { installCertificate: () => ['sslcontext-scope-ca'], installUnpinning: adapter.installUnpinning });
  instance.install();
  assert.equal(check.implementation('example.test', [leaf()]), undefined);
  assert.throws(() => check.implementation('example.test', [leaf({ unrelated: true }), f.ca]), e => e === rejection);
  instance.rollback();
  f.classes['okhttp3.CertificatePinner'].check.overloads.push(method(['java.lang.String', 'unknown.Chain']));
  assert.throws(() => instance.install(), /already installed/);
  const second = createEngine({ profiles: ['certificate', 'unpinning'] }, { installCertificate: () => ['sslcontext-scope-ca'], installUnpinning: adapter.installUnpinning });
  assert.throws(() => second.install(), /Unsupported OkHttp/);
  assert.equal(check.implementation, null);
});

test('opaque PublicKey wrappers require the Key interface for direct validation and ordinary pinners', () => {
  const { createJavaAdapter, createEngine } = api();
  const f = javaFixture();
  const valid = leaf();
  assert.equal(typeof valid.getPublicKey().getAlgorithm, 'undefined');
  const adapter = createJavaAdapter(f.Java, { caPem: 'public certificate' });
  adapter.validateScopeChain([valid]);
  assert.equal(f.calls.filter(call => Array.isArray(call) && call[0] === 'key-cast').length, 1);
  assert.equal(f.calls.find(call => Array.isArray(call) && call[0] === 'validate')[2], 'RSA');

  for (const signature of ['java.util.List', '[Ljava.security.cert.Certificate;']) {
    const p = javaFixture();
    const rejection = javaError('pin', 'original pin rejection');
    const check = method(['java.lang.String', signature], () => { throw rejection; });
    p.classes['okhttp3.CertificatePinner'] = { check: dispatcher(check) };
    const pinAdapter = createJavaAdapter(p.Java, { caPem: 'public certificate' });
    createEngine({ profiles: ['certificate', 'unpinning'] }, {
      installCertificate: () => ['sslcontext-scope-ca'],
      installUnpinning: pinAdapter.installUnpinning,
    }).install();
    const peer = values => signature === 'java.util.List' ? { toArray: () => values } : values;
    assert.equal(check.implementation('example.test', peer([valid])), undefined);
    assert.throws(() => check.implementation('example.test', peer([leaf({ unrelated: true }), p.ca])), error => error === rejection);
    assert.equal(p.calls.filter(call => Array.isArray(call) && call[0] === 'key-cast').length, 2);
  }
});

test('recognized lazy OkHttp refuses readiness and rolls back certificate and ordinary hooks', () => {
  const { createJavaAdapter, createEngine } = api();
  const f = tlsFixture();
  const ordinary = method(['java.lang.String', 'java.util.List']);
  const lazy = method(['java.lang.String', 'kotlin.jvm.functions.Function0']);
  f.classes['kotlin.jvm.functions.Function0'] = {};
  f.classes['okhttp3.CertificatePinner'] = { check: dispatcher(ordinary), 'check$okhttp': dispatcher(lazy) };
  const adapter = createJavaAdapter(f.Java, { caPem: 'public certificate' });
  const engine = createEngine({ profiles: ['certificate', 'unpinning'] }, adapter);
  // A lazy Handshake supplier may already have discarded the raw Scope chain.
  // Public overloads on that class do not prove normal connection coverage.
  assert.throws(() => engine.install(), /Unsupported lazy OkHttp/);
  assert.equal(f.init.implementation, null);
  assert.equal(ordinary.implementation, null);
  assert.equal(lazy.implementation, null);
});

test('extended accepted issuers registration explicitly implements the X509 array descriptor', () => {
  const { createJavaAdapter, createEngine } = api();
  const f = tlsFixture(true);
  createEngine({ profiles: ['certificate'] }, createJavaAdapter(f.Java, { caPem: 'public certificate' })).install();
  const descriptor = f.definition.methods.getAcceptedIssuers;
  assert.equal(descriptor.returnType, '[Ljava.security.cert.X509Certificate;');
  assert.deepEqual(plain(descriptor.argumentTypes), []);
});

test('valid Scope chain cannot suppress unrelated trust or ordinary pinner errors', () => {
  const { createJavaAdapter, createEngine } = api();
  const unrelated = new Error('internal programming failure');
  const f = tlsFixture();
  f.original.checkServerTrusted = () => { throw unrelated; };
  createEngine({ profiles: ['certificate'] }, createJavaAdapter(f.Java, { caPem: 'public certificate' })).install();
  f.init.implementation(null, [f.original], null);
  assert.throws(() => f.wrapped.checkServerTrusted([leaf()], 'RSA'), e => e === unrelated);
  for (const chainType of ['java.util.List', '[Ljava.security.cert.Certificate;']) {
    const p = javaFixture();
    const check = method(['java.lang.String', chainType], () => { throw unrelated; });
    p.classes['okhttp3.CertificatePinner'] = { check: dispatcher(check) };
    const adapter = createJavaAdapter(p.Java, { caPem: 'public certificate' });
    createEngine({ profiles: ['certificate', 'unpinning'] }, { installCertificate: () => ['sslcontext-scope-ca'], installUnpinning: adapter.installUnpinning }).install();
    assert.throws(() => check.implementation('test', [leaf()]), e => e === unrelated);
  }
});

test('nullable extended delegation selects exact original Socket and SSLEngine overloads', () => {
  const { createJavaAdapter, createEngine } = api();
  const f = tlsFixture(true);
  const selected = [];
  for (const name of ['checkClientTrusted', 'checkServerTrusted']) {
    const legacy = f.original[name];
    const dynamic = (...args) => legacy(...args); // Baseline dynamic dispatcher cannot distinguish null.
    Object.assign(dynamic, dispatcher(...['java.net.Socket', 'javax.net.ssl.SSLEngine'].map(type => method(['[Ljava.security.cert.X509Certificate;', 'java.lang.String', type], (_chain, _auth, transport) => { assert.equal(transport, null); selected.push([name, type]); }))));
    f.original[name] = dynamic;
  }
  createEngine({ profiles: ['certificate'] }, createJavaAdapter(f.Java, { caPem: 'public certificate' })).install();
  f.init.implementation(null, [f.original], null);
  for (const name of ['checkClientTrusted', 'checkServerTrusted']) {
    for (const type of ['java.net.Socket', 'javax.net.ssl.SSLEngine']) {
      f.definition.methods[name].find(entry => entry.argumentTypes[2] === type).implementation.call(f.wrapped, [leaf({ unrelated: true })], 'RSA', null);
    }
  }
  assert.deepEqual(selected, [['checkClientTrusted', 'java.net.Socket'], ['checkClientTrusted', 'javax.net.ssl.SSLEngine'], ['checkServerTrusted', 'java.net.Socket'], ['checkServerTrusted', 'javax.net.ssl.SSLEngine']]);
});


test('the same peer and CA are revalidated after an earlier accepted fallback', () => {
  const f = tlsFixture();
  api().createEngine({ profiles: ['certificate'] }, api().createJavaAdapter(f.Java, { caPem: 'public certificate' })).install();
  f.init.implementation(null, [f.original], null);
  let expired = false;
  const chain = [leaf({ checkValidity() { if (expired) throw Error('now expired'); } })];
  f.wrapped.checkServerTrusted(chain, 'RSA');
  expired = true;
  assert.throws(() => f.wrapped.checkServerTrusted(chain, 'RSA'), error => error === f.rejection);
  expired = false;
  f.ca.checkValidity = () => { throw Error('CA now expired'); };
  assert.throws(() => f.wrapped.checkServerTrusted(chain, 'RSA'), error => error === f.rejection);
  assert.equal(f.calls.filter(call => call[0] === 'server').length, 3);
});
