/* Snurpo-owned Java interception engine. Uses the documented Frida Java API.
 * Coverage is deliberately limited to java.net.URL, SSLContext and OkHttp3.
 * Method rollback cannot revoke already-created clients: the worker must end
 * its spawned process on Stop or failure. No hostname verifier is replaced.
 */
(function (root) {
  'use strict';
  const PROFILES = ['proxy', 'certificate', 'unpinning'];
  const REQUIRED = { proxy: 'java-url', certificate: 'sslcontext-scope-ca', unpinning: 'okhttp3-certificate-pinner' };

  function normalizeProfiles(profiles) {
    if (!Array.isArray(profiles) || profiles.some(p => !PROFILES.includes(p))) {
      throw new Error('Unsupported hook profile; supported: proxy, certificate, unpinning');
    }
    const selected = profiles.length ? PROFILES.filter(p => profiles.includes(p)) : PROFILES.slice();
    if (selected.includes('unpinning') && !selected.includes('certificate')) {
      throw new Error('unpinning profile requires certificate');
    }
    return selected;
  }

  function createPolicy(validateScopeChain, rejectionTypes = {}) {
    function isScope(chain, context) {
      try { validateScopeChain(chain, context); return true; } catch (_) { return false; }
    }
    return {
      checkPins(chain, original) {
        try { return original(); } catch (rejection) {
          if (!rejectionTypes.isPinRejection || !rejectionTypes.isPinRejection(rejection) || !isScope(chain)) throw rejection;
        }
      },
      checkTrust(chain, original, context) {
        try { return original(); } catch (rejection) {
          if (!rejectionTypes.isCertificateRejection || !rejectionTypes.isCertificateRejection(rejection) || !isScope(chain, context)) throw rejection;
        }
      }
    };
  }

  function createEngine(config, adapter) {
    const profiles = normalizeProfiles(config.profiles);
    const undo = [];
    const transaction = {
      hook(method, implementation) {
        const previous = method.implementation;
        // Register undo first: a failing setter can have partially installed.
        undo.push(() => { method.implementation = previous; });
        method.implementation = implementation;
      }
    };
    function rollback() {
      const errors = [];
      while (undo.length) {
        try { undo.pop()(); } catch (error) { errors.push(String(error)); }
      }
      if (errors.length) throw new Error('Hook rollback failed: ' + errors.join('; ').slice(0, 1500));
    }
    let installed = false;
    return {
      rollback,
      install() {
        if (installed) throw new Error('Engine already installed');
        const capabilities = {};
        try {
          for (const profile of profiles) {
            const name = 'install' + profile[0].toUpperCase() + profile.slice(1);
            const result = adapter[name](transaction);
            if (!Array.isArray(result) || !result.includes(REQUIRED[profile])) {
              throw new Error('Missing installed capability for profile ' + profile);
            }
            capabilities[profile] = result;
          }
          installed = true;
          return { kind: 'scope-hooks-ready', engine: 'scope-java-v1', profiles, capabilities };
        } catch (error) {
          try { rollback(); } catch (cleanupError) {
            throw new Error(String(error) + '; ' + String(cleanupError));
          }
          throw error;
        }
      }
    };
  }

  function createJavaAdapter(Java, config) {
    let trust;
    function use(name) {
      try { return Java.use(name); } catch (error) {
        throw new Error('Required Java class unavailable: ' + name + ': ' + String(error).slice(0, 300));
      }
    }
    function overload(klass, method, signature) {
      if (!klass[method]) throw new Error('Required method unavailable: ' + method);
      try { return klass[method].overload(...signature); } catch (error) {
        throw new Error('Required overload unavailable: ' + method + '(' + signature.join(', ') + '): ' + String(error).slice(0, 300));
      }
    }
    function trustState() {
      if (trust) return trust;
      const X509 = use('javax.net.ssl.X509TrustManager');
      const Certificate = use('java.security.cert.X509Certificate');
      const Factory = use('javax.net.ssl.TrustManagerFactory');
      const Store = use('java.security.KeyStore');
      const bytes = use('java.lang.String').$new(config.caPem).getBytes('UTF-8');
      const stream = use('java.io.ByteArrayInputStream').$new(bytes);
      const ca = Java.cast(use('java.security.cert.CertificateFactory').getInstance('X.509').generateCertificate(stream), Certificate);
      ca.checkValidity();
      if (ca.getBasicConstraints() < 0) throw new Error('Snurpo certificate must be a CA');
      const store = Store.getInstance(Store.getDefaultType());
      store.load(null, null); // Empty, never the Android/system trust store.
      store.setCertificateEntry('scope', ca);
      const factory = Factory.getInstance(Factory.getDefaultAlgorithm());
      factory.init(store);
      const managers = factory.getTrustManagers();
      let manager;
      for (let i = 0; i < managers.length; i++) {
        if (X509.class.isInstance(managers[i])) { manager = Java.cast(managers[i], X509); break; }
      }
      if (!manager) throw new Error('No X509 trust manager for isolated Snurpo CA');
      let Extended;
      try { Extended = Java.use('javax.net.ssl.X509ExtendedTrustManager'); } catch (_) { /* Android before API 24. */ }
      trust = { manager, ca, Factory, X509, Certificate, Extended };
      return trust;
    }
    function certificates(chain) {
      if (!chain) throw new Error('Peer chain is empty');
      const values = typeof chain.toArray === 'function' ? chain.toArray() : chain;
      if (!values.length) throw new Error('Peer chain is empty');
      const { Certificate } = trustState();
      const result = [];
      for (let i = 0; i < values.length; i++) {
        if (!Certificate.class.isInstance(values[i])) throw new Error('Peer chain contains a non-X509 certificate');
        result.push(Java.cast(values[i], Certificate));
      }
      return result;
    }
    function validateScopeChain(chain, context) {
      const state = trustState();
      const values = certificates(chain);
      if (values[0].getBasicConstraints() >= 0) throw new Error('Peer chain must start with a server leaf certificate');
      // Check every supplied certificate, including trust anchors. Trust manager
      // performs path construction, signature, usage and algorithm validation.
      state.ca.checkValidity();
      values.forEach(cert => cert.checkValidity());
      // Frida's PublicKey wrapper may not expose the inherited Key interface.
      const authType = context ? context.authType : String(Java.cast(values[0].getPublicKey(), use('java.security.Key')).getAlgorithm());
      const peer = Java.array('java.security.cert.X509Certificate', values);
      if (context && context.type) {
        if (!state.Extended || !state.Extended.class.isInstance(state.manager)) throw new Error('Isolated Snurpo manager cannot preserve extended TLS context');
        const manager = Java.cast(state.manager, state.Extended);
        manager.checkServerTrusted.overload('[Ljava.security.cert.X509Certificate;', 'java.lang.String', context.type).call(manager, peer, authType, context.transport);
      } else {
        state.manager.checkServerTrusted(peer, authType);
      }
    }
    function isJavaException(error, className) {
      if (!error || !error.$h) return false;
      try {
        // The retained bridge attaches the original JNI Throwable handle to
        // Java invocation errors. Inspect that object, never message text.
        return use(className).class.isInstance(Java.cast(error.$h, use('java.lang.Throwable')));
      } catch (_) { return false; }
    }
    const policy = createPolicy(validateScopeChain, {
      isCertificateRejection: error => isJavaException(error, 'java.security.cert.CertificateException'),
      isPinRejection: error => isJavaException(error, 'javax.net.ssl.SSLPeerUnverifiedException')
    });

    function installCertificate(transaction) {
      const state = trustState();
      const init = overload(use('javax.net.ssl.SSLContext'), 'init', ['[Ljavax.net.ssl.KeyManager;', '[Ljavax.net.ssl.TrustManager;', 'java.security.SecureRandom']);
      function wrapperClass(extended) {
        const methods = {
          getAcceptedIssuers: {
            returnType: '[Ljava.security.cert.X509Certificate;', argumentTypes: [],
            implementation() {
              const original = this.delegate.value.getAcceptedIssuers();
              return Java.array('java.security.cert.X509Certificate', Array.from(original).concat([state.ca]));
            }
          }
        };
        for (const name of ['checkClientTrusted', 'checkServerTrusted']) {
          const implementations = [];
          for (const type of extended ? [null, 'java.net.Socket', 'javax.net.ssl.SSLEngine'] : [null]) {
            implementations.push({
              returnType: 'void',
              argumentTypes: ['[Ljava.security.cert.X509Certificate;', 'java.lang.String'].concat(type ? [type] : []),
              implementation(chain, authType, transport) {
                const delegate = Java.cast(this.delegate.value, extended ? state.Extended : state.X509);
                const original = () => type ? delegate[name].overload('[Ljava.security.cert.X509Certificate;', 'java.lang.String', type).call(delegate, chain, authType, transport) : delegate[name](chain, authType);
                if (name === 'checkClientTrusted') return original();
                return policy.checkTrust(chain, original, { authType, type, transport });
              }
            });
          }
          methods[name] = implementations;
        }
        return Java.registerClass({
          name: extended ? 'dev.scope.runtime.ScopeExtendedTrustManager' : 'dev.scope.runtime.ScopeTrustManager',
          ...(extended ? { superClass: state.Extended } : { implements: [state.X509] }),
          fields: { delegate: 'javax.net.ssl.X509TrustManager' },
          methods
        });
      }
      const BasicWrapper = wrapperClass(false);
      const ExtendedWrapper = state.Extended && state.Extended.class.isInstance(state.manager) ? wrapperClass(true) : null;
      transaction.hook(init, function (keys, managers, random) {
        if (managers === null) {
          const defaults = state.Factory.getInstance(state.Factory.getDefaultAlgorithm());
          // Select the KeyStore overload explicitly; null otherwise ambiguous.
          defaults.init.overload('java.security.KeyStore').call(defaults, null);
          managers = defaults.getTrustManagers();
        }
        let count = 0;
        const wrapped = Array.from(managers, manager => {
          if (!state.X509.class.isInstance(manager)) return manager;
          const extended = state.Extended && state.Extended.class.isInstance(manager);
          if (extended && !ExtendedWrapper) throw new Error('Cannot preserve X509ExtendedTrustManager context with this Snurpo manager');
          const wrapper = (extended ? ExtendedWrapper : BasicWrapper).$new();
          wrapper.delegate.value = Java.cast(manager, state.X509);
          count++;
          return wrapper;
        });
        if (!count) throw new Error('SSLContext has no X509 trust manager to extend');
        return init.call(this, keys, Java.array('javax.net.ssl.TrustManager', wrapped), random);
      });
      return ['sslcontext-scope-ca'];
    }

    function installProxy(transaction) {
      const URL = use('java.net.URL');
      const Proxy = use('java.net.Proxy');
      const address = use('java.net.InetSocketAddress').$new(config.proxyHost, config.proxyPort);
      const proxy = Proxy.$new(use('java.net.Proxy$Type').HTTP.value, address);
      const plain = overload(URL, 'openConnection', []);
      const explicit = overload(URL, 'openConnection', ['java.net.Proxy']);
      function http(url) { return ['http', 'https'].includes(String(url.getProtocol())); }
      transaction.hook(plain, function () { return http(this) ? explicit.call(this, proxy) : plain.call(this); });
      transaction.hook(explicit, function (selected) { return explicit.call(this, http(this) ? proxy : selected); });
      const capabilities = ['java-url'];
      let Builder;
      try { Builder = Java.use('okhttp3.OkHttpClient$Builder'); } catch (_) { return capabilities; }
      const build = overload(Builder, 'build', []);
      const setProxy = overload(Builder, 'proxy', ['java.net.Proxy']);
      transaction.hook(build, function () { setProxy.call(this, proxy); return build.call(this); });
      capabilities.push('okhttp3-proxy');
      return capabilities;
    }

    function installUnpinning(transaction) {
      trustState();
      const Pinner = use('okhttp3.CertificatePinner');
      // Normal Kotlin connections may lose the raw chain during original-manager
      // cleaning before check$okhttp sees it. Public overloads on the same class
      // cannot establish coverage. Refuse this configuration transactionally.
      if (Pinner['check$okhttp']) throw new Error('Unsupported lazy OkHttp configuration: check$okhttp; use the ordinary Java OkHttp 3.14.9 acceptance target');
      let count = 0;
      if (Pinner.check) {
        for (const method of Pinner.check.overloads) {
          const args = method.argumentTypes.map(type => type.className);
          const ordinary = args.length === 2 && args[0] === 'java.lang.String' && ['java.util.List', '[Ljava.security.cert.Certificate;'].includes(args[1]);
          if (!ordinary) throw new Error('Unsupported OkHttp pinner overload: check(' + args.join(', ') + ')');
          transaction.hook(method, function (host, peer) {
            return policy.checkPins(peer, () => method.call(this, host, peer));
          });
          count++;
        }
      }
      if (!count) throw new Error('No supported okhttp3.CertificatePinner overloads');
      return ['okhttp3-certificate-pinner'];
    }
    return { validateScopeChain, installProxy, installCertificate, installUnpinning };
  }

  function start(config) {
    try {
      normalizeProfiles(config.profiles);
      if (config.proxyHost !== '127.0.0.1' || !Number.isInteger(config.proxyPort) || config.proxyPort < 1 || config.proxyPort > 65535) throw new Error('Invalid loopback proxy configuration');
      if (!root.Java || !root.Java.available) throw new Error('Frida Java runtime is unavailable');
      root.Java.perform(() => {
        try { root.send(createEngine(config, createJavaAdapter(root.Java, config)).install()); }
        catch (error) { root.send({ kind: 'scope-hooks-error', engine: 'scope-java-v1', message: String(error).slice(0, 2000) }); }
      });
    } catch (error) { root.send({ kind: 'scope-hooks-error', engine: 'scope-java-v1', message: String(error).slice(0, 2000) }); }
  }
  const api = { normalizeProfiles, createPolicy, createEngine, createJavaAdapter, start };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else root.ScopeJavaEngine = api;
})(globalThis);
