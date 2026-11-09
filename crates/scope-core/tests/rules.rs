//! End-to-end rule engine contracts: real sockets, TLS interception, persistence and cleanup.
use http_body_util::BodyExt;
use hudsucker::{
    hyper::{service::service_fn, Request, Response},
    hyper_util::{
        rt::{TokioExecutor, TokioIo},
        server::conn::auto,
    },
    rcgen::{CertificateParams, Issuer, KeyPair},
    rustls::{self, crypto::aws_lc_rs},
    Body,
};
use scope_core::{
    EditMessage, Engine, Header, PauseStage, SessionUpdate, TrafficProvenance, TrafficRule,
};
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    net::TcpListener,
    task::{JoinHandle, JoinSet},
};

async fn fixture(engine: &Engine, tls: bool) -> (String, Arc<AtomicUsize>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(AtomicUsize::new(0));
    let count = requests.clone();
    let acceptor = if tls {
        let key = KeyPair::generate().unwrap();
        let issuer = Issuer::from_ca_cert_pem(
            &std::fs::read_to_string(engine.ca_cert_path()).unwrap(),
            KeyPair::from_pem(
                &std::fs::read_to_string(engine.data_dir().join("scope-ca-key.pem")).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let cert = CertificateParams::new(vec!["localhost".into()])
            .unwrap()
            .signed_by(&key, &issuer)
            .unwrap();
        let config =
            rustls::ServerConfig::builder_with_provider(Arc::new(aws_lc_rs::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(
                    vec![cert.der().clone()],
                    rustls::pki_types::PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
                )
                .unwrap();
        Some(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
    } else {
        None
    };
    let task = tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let count = count.clone();
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let handler = service_fn(move |req: Request<hudsucker::hyper::body::Incoming>| {
                    let count = count.clone();
                    async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        let delayed = req.uri().path() == "/slow";
                        let path = req.uri().path().to_owned();
                        let method = req.method().to_string();
                        let saw = req
                            .headers()
                            .get("x-rule")
                            .map(|h| h.to_str().unwrap().to_owned())
                            .unwrap_or_default();
                        let body = req.into_body().collect().await.unwrap().to_bytes();
                        if delayed {
                            tokio::time::sleep(Duration::from_millis(300)).await;
                        }
                        Ok::<_, Infallible>(
                            Response::builder()
                                .header("x-upstream-path", path)
                                .header("x-upstream-method", method)
                                .header("x-saw-rule", saw)
                                .header("content-encoding", "identity")
                                .header("etag", "stale")
                                .body(Body::from(body))
                                .unwrap(),
                        )
                    }
                });
                let server = auto::Builder::new(TokioExecutor::new());
                if let Some(tls) = acceptor {
                    if let Ok(tls) = tls.accept(stream).await {
                        let _ = server.serve_connection(TokioIo::new(tls), handler).await;
                    }
                } else {
                    let _ = server.serve_connection(TokioIo::new(stream), handler).await;
                }
            });
        }
    });
    (
        format!(
            "{}://{}:{port}",
            if tls { "https" } else { "http" },
            if tls { "localhost" } else { "127.0.0.1" }
        ),
        requests,
        task,
    )
}
fn client(engine: &Engine, port: u16) -> reqwest::Client {
    client_with_timeout(engine, port, Duration::from_secs(5))
}
fn client_with_timeout(engine: &Engine, port: u16, timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}")).unwrap())
        .add_root_certificate(
            reqwest::Certificate::from_pem(&std::fs::read(engine.ca_cert_path()).unwrap()).unwrap(),
        )
        .timeout(timeout)
        .build()
        .unwrap()
}
fn rule(priority: i32, matcher: Value, action: Value) -> TrafficRule {
    serde_json::from_value(json!({"id":"","name":format!("Rule {priority}"),"enabled":true,"priority":priority,"matcher":matcher,"action":action})).unwrap()
}
#[tokio::test]
async fn mock_and_block_never_contact_upstream_over_http_and_https() {
    for tls in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::open(dir.path()).unwrap();
        let session = engine
            .create_session("Rules".into(), "test".into())
            .unwrap();
        let (url, requests, server) = fixture(&engine, tls).await;
        let mock = engine.save_rule(rule(1,json!({"pathPrefix":"/mock","method":"GET"}),json!({"type":"mockResponse","status":201,"headers":[{"name":"content-type","value":"application/octet-stream"},{"name":"content-length","value":"999"}],"body":"AP+A","bodyBase64":true}))).unwrap();
        engine
            .save_rule(rule(
                2,
                json!({"pathPrefix":"/blocked"}),
                json!({"type":"block","reason":"Policy test"}),
            ))
            .unwrap();
        let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
        let client = client(&engine, proxy.port);
        let response = client.get(format!("{url}/mock")).send().await.unwrap();
        assert_eq!(response.status(), 201);
        assert_eq!(response.content_length(), Some(3));
        assert_eq!(response.bytes().await.unwrap().as_ref(), &[0, 255, 128]);
        let blocked = client
            .post(format!("{url}/blocked"))
            .body("never sent")
            .send()
            .await
            .unwrap();
        assert_eq!(blocked.status(), 403);
        assert!(blocked.text().await.unwrap().contains("Policy test"));
        assert_eq!(requests.load(Ordering::SeqCst), 0);
        engine.stop_proxy().await.unwrap();
        let exchanges = engine.list_exchanges(&session.id, None, 10, 0).unwrap();
        assert!(exchanges.iter().all(|e| e.completed && e.error.is_none()));
        assert_eq!(exchanges[0].provenance, TrafficProvenance::Blocked);
        assert_eq!(exchanges[1].provenance, TrafficProvenance::Mock);
        assert_eq!(exchanges[1].applied_rules[0].rule_id, mock.id);
        let replay = engine.replay(&exchanges[1].id, None).await.unwrap();
        assert_eq!(replay.provenance, TrafficProvenance::Mock);
        assert_eq!(requests.load(Ordering::SeqCst), 0);
        server.abort();
    }
}
#[tokio::test]
async fn ordered_rewrites_keep_originals_and_normalize_binary_and_encoding_headers() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let session = engine
        .create_session("Rewrites".into(), "test".into())
        .unwrap();
    let (url, requests, server) = fixture(&engine, true).await;
    engine.save_rule(rule(20,json!({"pathPrefix":"/before"}),json!({"type":"rewriteRequest","body":"AP+A","bodyBase64":true,"headers":[{"name":"x-rule","value":"applied"},{"name":"content-length","value":"800"},{"name":"content-encoding","value":"gzip"}]}))).unwrap();
    engine.save_rule(rule(10,json!({"pathPrefix":"/before"}),json!({"type":"rewriteRequest","url":format!("{url}/after"),"method":"PATCH","body":"first"}))).unwrap();
    engine
        .save_rule(rule(
            30,
            json!({"pathPrefix":"/before"}),
            json!({"type":"rewriteResponse","status":202,"body":"reply"}),
        ))
        .unwrap();
    // Matching is frozen against incoming URL. This rule must not activate after /before is rewritten.
    engine
        .save_rule(rule(
            40,
            json!({"pathPrefix":"/after"}),
            json!({"type":"block"}),
        ))
        .unwrap();
    let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
    let client = client(&engine, proxy.port);
    let response = client
        .post(format!("{url}/before"))
        .body("original body")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 202);
    assert_eq!(response.headers()["x-upstream-path"], "/after");
    assert_eq!(response.headers()["x-upstream-method"], "PATCH");
    assert_eq!(response.headers()["x-saw-rule"], "applied");
    assert!(!response.headers().contains_key("content-encoding"));
    assert!(!response.headers().contains_key("etag"));
    assert_eq!(response.content_length(), Some(5));
    assert_eq!(response.text().await.unwrap(), "reply");
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    engine.stop_proxy().await.unwrap();
    let e = engine
        .list_exchanges(&session.id, None, 10, 0)
        .unwrap()
        .remove(0);
    assert_eq!(e.method, "PATCH");
    assert_eq!(e.request_body, "AP+A");
    assert!(e.request_body_base64);
    assert_eq!(
        e.original_request.as_ref().unwrap()["body"],
        "original body"
    );
    assert_eq!(e.original_response.as_ref().unwrap()["body"], "AP+A");
    assert_eq!(e.applied_rules.len(), 3);
    assert_eq!(e.applied_rules[0].name, "Rule 10");
    assert_eq!(e.applied_rules[1].name, "Rule 20");
    assert!(e
        .request_headers
        .iter()
        .any(|h| h.name == "content-length" && h.value == "3"));
    let archive = engine.export_session(&session.id).unwrap();
    let imported = engine.import_session(archive).unwrap();
    let imported = engine
        .list_exchanges(&imported.id, None, 10, 0)
        .unwrap()
        .remove(0);
    assert_eq!(imported.applied_rules.len(), 3);
    assert_eq!(imported.original_response, e.original_response);
    server.abort();
}
#[tokio::test]
async fn in_flight_state_updates_same_exchange_and_rule_updates_are_atomic_per_request() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let session = engine
        .create_session("Snapshot".into(), "test".into())
        .unwrap();
    let (url, _, server) = fixture(&engine, false).await;
    let saved = engine
        .save_rule(rule(
            1,
            json!({"pathPrefix":"/slow"}),
            json!({"type":"rewriteResponse","body":"old snapshot"}),
        ))
        .unwrap();
    let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
    let client = client(&engine, proxy.port);
    let firstclient = client.clone();
    let firsturl = url.clone();
    let request = tokio::spawn(async move {
        firstclient
            .get(format!("{firsturl}/slow"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    });
    let pending = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let rows = engine.list_exchanges(&session.id, None, 10, 0).unwrap();
            if !rows.is_empty() {
                break rows[0].clone();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(!pending.completed);
    assert!(pending.error.is_none());
    assert_eq!(pending.status, None);
    let mut changed = saved;
    changed.action =
        serde_json::from_value(json!({"type":"rewriteResponse","body":"new snapshot"})).unwrap();
    engine.save_rule(changed).unwrap();
    assert_eq!(request.await.unwrap(), "old snapshot");
    let completed = engine.get_exchange(&pending.id).unwrap();
    assert!(completed.completed);
    assert_eq!(completed.response_body, "old snapshot");
    assert_eq!(
        client
            .get(format!("{url}/slow"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "new snapshot"
    );
    engine.stop_proxy().await.unwrap();
    let replaying = engine.clone();
    let replay_id = completed.id;
    let replay = tokio::spawn(async move { replaying.replay(&replay_id, None).await.unwrap() });
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if engine
                .list_exchanges(&session.id, None, 10, 0)
                .unwrap()
                .iter()
                .any(|exchange| !exchange.completed)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(engine
        .delete_session(&session.id)
        .unwrap_err()
        .to_string()
        .contains("active replays"));
    assert!(replay.await.unwrap().completed);
    engine.delete_session(&session.id).unwrap();
    server.abort();
}
#[tokio::test]
async fn durable_rules_session_metadata_audit_and_active_delete_guard() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let session = engine
        .create_session("Before".into(), "test".into())
        .unwrap();
    let rule = engine
        .save_rule(rule(1, json!({}), json!({"type":"block"})))
        .unwrap();
    engine
        .update_session(
            &session.id,
            SessionUpdate {
                name: Some("After".into()),
                notes: Some("Keep this capture\nfor review".into()),
                tags: Some(vec![
                    "android".into(),
                    "regression".into(),
                    "android".into(),
                ]),
                pinned: Some(true),
            },
        )
        .unwrap();
    engine.start_proxy(session.id.clone(), 0).await.unwrap();
    assert!(engine
        .delete_session(&session.id)
        .unwrap_err()
        .to_string()
        .contains("Stop capture"));
    engine.stop_proxy().await.unwrap();
    drop(engine);
    let reopened = Engine::open(dir.path()).unwrap();
    let s = reopened.list_sessions().unwrap().remove(0);
    assert_eq!(s.name, "After");
    assert!(s.pinned);
    assert_eq!(s.tags, vec!["android", "regression"]);
    assert!(s.notes.contains("review"));
    assert_eq!(reopened.list_rules().unwrap()[0].id, rule.id);
    assert!(reopened.record_audit("secret token=123", None).is_err());
    assert!(reopened
        .record_audit("request.replay", Some("https://example.com/token"))
        .is_err());
    reopened.delete_rule(&rule.id).unwrap();
    reopened.delete_session(&session.id).unwrap();
    assert!(reopened.list_sessions().unwrap().is_empty());
    let audit = reopened.list_audit(100).unwrap();
    assert!(audit.iter().any(|entry| entry.action == "session.delete"));
    assert!(audit.iter().any(|entry| entry.action == "rule.delete"));
    assert!(audit
        .iter()
        .all(|entry| entry.target_id.as_deref() != Some("After")));
}
#[test]
fn invalid_rules_do_not_replace_last_good_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let mut saved = engine
        .save_rule(rule(
            1,
            json!({"host":"Example.COM"}),
            json!({"type":"mockResponse","status":200,"body":"ok"}),
        ))
        .unwrap();
    assert_eq!(saved.matcher.host.as_deref(), Some("example.com"));
    let old = saved.clone();
    saved.action = serde_json::from_value(json!({"type":"mockResponse","status":101})).unwrap();
    assert!(engine.save_rule(saved).is_err());
    for (matcher, action) in [
        (json!({"host":"*.example.com"}), json!({"type":"block"})),
        (json!({"pathPrefix":"wrong"}), json!({"type":"block"})),
        (
            json!({}),
            json!({"type":"rewriteRequest","url":"file:///etc/passwd"}),
        ),
        (
            json!({}),
            json!({"type":"rewriteResponse","headers":[{"name":"x-header","value":"bad\r\nvalue"}]}),
        ),
        (
            json!({}),
            json!({"type":"mockResponse","status":200,"body":"not-base64!","bodyBase64":true}),
        ),
        (json!({}), json!({"type":"rewriteRequest"})),
    ] {
        assert!(engine.save_rule(rule(2, matcher, action)).is_err());
    }
    let rules = engine.list_rules().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].id, old.id);
    assert_eq!(
        serde_json::to_value(&rules[0]).unwrap(),
        serde_json::to_value(old).unwrap()
    );
}
fn install_mock_rules(engine: &Engine) -> String {
    let rewrite = engine
        .save_rule(rule(
            1,
            json!({"pathPrefix":"/pause"}),
            json!({"type":"rewriteRequest","body":"request rule"}),
        ))
        .unwrap();
    engine
        .save_rule(rule(
            2,
            json!({"pathPrefix":"/pause"}),
            json!({"type":"mockResponse","status":201,"body":"mock body"}),
        ))
        .unwrap();
    engine
        .save_rule(rule(
            3,
            json!({"pathPrefix":"/pause"}),
            json!({"type":"rewriteResponse","body":"response rule"}),
        ))
        .unwrap();
    rewrite.id
}
#[tokio::test]
async fn rules_run_before_breakpoints() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let session = engine
        .create_session("Breakpoints".into(), "test".into())
        .unwrap();
    let rewrite_id = install_mock_rules(&engine);
    let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
    let client = client(&engine, proxy.port);
    engine.set_breakpoints(true, true);
    let firstclient = client.clone();
    let request = tokio::spawn(async move {
        firstclient
            .post("https://localhost:1/pause")
            .body("before")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    });
    async fn pending(engine: &Engine, stage: &str) -> scope_core::PendingExchange {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(p) = engine.pending().into_iter().find(|p| p.stage == stage) {
                    break p;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap()
    }
    let req = pending(&engine, "request").await;
    assert_eq!(req.body, "request rule");
    engine
        .resolve_breakpoint(&req.id, scope_core::EditMessage::continue_unchanged())
        .unwrap();
    let res = pending(&engine, "response").await;
    assert_eq!(res.status, Some(201));
    assert_eq!(res.body, "response rule");
    engine
        .resolve_breakpoint(
            &res.id,
            scope_core::EditMessage {
                action: "continue".into(),
                body: Some("manual wins".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(request.await.unwrap(), "manual wins");
    let e = engine.get_exchange(&req.exchange_id).unwrap();
    assert!(e.completed);
    assert_eq!(e.original_request.unwrap()["body"], "before");
    assert_eq!(e.original_response.unwrap()["body"], "mock body");
    assert_eq!(e.applied_rules.len(), 3);
    assert_eq!(e.provenance, TrafficProvenance::Mock);
    assert_eq!(e.applied_rules[0].rule_id, rewrite_id);
    let rows = engine.list_exchanges(&session.id, None, 100, 0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, req.exchange_id);

    // Keep the enabled -> disabled transition covered separately from the
    // persistence stress batch: no manual breakpoint resolution is needed.
    engine.set_breakpoints(false, false);
    let status = engine.proxy_status();
    assert!(!status.request_breakpoint);
    assert!(!status.response_breakpoint);
    let unpaused = client
        .get("http://localhost:1/pause?breakpoints=disabled")
        .send()
        .await
        .unwrap();
    assert_eq!(unpaused.status(), 201);
    assert_eq!(unpaused.text().await.unwrap(), "response rule");
    assert!(engine.pending().is_empty());
    let rows = engine.list_exchanges(&session.id, None, 100, 0).unwrap();
    assert_eq!(rows.len(), 2);
    let after_disable = rows.iter().find(|row| row.id != req.exchange_id).unwrap();
    assert!(after_disable.completed);
    assert!(after_disable.error.is_none());
    assert_eq!(after_disable.status, Some(201));
    assert_eq!(after_disable.provenance, TrafficProvenance::Mock);
    assert_eq!(after_disable.response_body, "response rule");
    assert_eq!(after_disable.applied_rules.len(), 3);
    assert_eq!(after_disable.applied_rules[0].rule_id, rewrite_id);
    engine.stop_proxy().await.unwrap();
}

#[tokio::test]
async fn concurrent_mock_requests_keep_all_evidence() {
    const CLIENTS: usize = 32;
    // This burst performs six synchronous SQLite saves per exchange (192
    // autocommit statements) on the current-thread test runtime. Give this
    // stress client its own budget; interactive fixtures keep their 5s limit.
    // This is CI allowance, not a measurement of Windows storage latency.
    const CLIENT_TIMEOUT: Duration = Duration::from_secs(30);
    const BATCH_TIMEOUT: Duration = Duration::from_secs(60);
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let session = engine
        .create_session("Concurrent rules".into(), "test".into())
        .unwrap();
    let rewrite_id = install_mock_rules(&engine);
    let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
    let client = client_with_timeout(&engine, proxy.port, CLIENT_TIMEOUT);
    let started = Instant::now();
    let deadline = tokio::time::Instant::now() + BATCH_TIMEOUT;
    let mut requests = JoinSet::new();
    for i in 0..CLIENTS {
        let client = client.clone();
        requests.spawn(async move {
            let started = Instant::now();
            let result = async {
                let response = client
                    .get(format!("http://localhost:1/pause?i={i}"))
                    .send()
                    .await?;
                let status = response.status();
                let body = response.text().await?;
                Ok::<_, reqwest::Error>((status, body))
            }
            .await;
            (i, started.elapsed(), result)
        });
    }
    let mut joined = 0;
    let mut completed = 0;
    let mut successful = 0;
    let mut failures = Vec::new();
    let mut timed_out = false;
    while !requests.is_empty() {
        let next = if timed_out {
            requests.join_next().await
        } else {
            match tokio::time::timeout_at(deadline, requests.join_next()).await {
                Ok(next) => next,
                Err(_) => {
                    // Tokio deadlines are cooperative: a synchronous save can
                    // delay observing this timer. Abort only on whole-batch
                    // failure, then join every remaining task before asserting.
                    timed_out = true;
                    requests.abort_all();
                    continue;
                }
            }
        };
        let Some(result) = next else { break };
        joined += 1;
        match result {
            Ok((i, elapsed, result)) => {
                completed += 1;
                match result {
                    Ok((status, body)) if status == 201 && body == "response rule" => {
                        successful += 1
                    }
                    Ok((status, body)) => failures.push(format!(
                        "client {i} after {elapsed:?}: status={status}, body={body:?}"
                    )),
                    Err(error) => failures.push(format!("client {i} after {elapsed:?}: {error:?}")),
                }
            }
            Err(error) => failures.push(format!("join failure: {error:?}")),
        }
    }
    engine.stop_proxy().await.unwrap();
    let elapsed = started.elapsed();
    eprintln!("32-client mock batch: elapsed={elapsed:?}, completed={completed}/{CLIENTS}, joined={joined}/{CLIENTS}, successful={successful}/{CLIENTS}, batch_timeout={timed_out}");
    assert!(!timed_out && failures.is_empty(), "mock batch failed after {elapsed:?}: completed={completed}/{CLIENTS}, joined={joined}/{CLIENTS}, successful={successful}/{CLIENTS}, client_budget={CLIENT_TIMEOUT:?}, batch_budget={BATCH_TIMEOUT:?}, batch_timeout={timed_out}, failures={failures:?}");
    assert_eq!(joined, CLIENTS);
    assert_eq!(successful, CLIENTS);
    let rows = engine.list_exchanges(&session.id, None, 100, 0).unwrap();
    assert_eq!(rows.len(), CLIENTS);
    assert!(rows.iter().all(|e| e.completed
        && e.error.is_none()
        && e.status == Some(201)
        && e.response_body == "response rule"
        && e.provenance == TrafficProvenance::Mock
        && e.applied_rules.len() == 3
        && e.applied_rules[0].rule_id == rewrite_id));
    let ids = rows
        .iter()
        .map(|e| &e.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), CLIENTS);
}
#[tokio::test]
async fn failed_and_interrupted_captures_are_completed_and_restart_recovers_inflight() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let session = engine
        .create_session("Failures".into(), "test".into())
        .unwrap();
    let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
    let client = client(&engine, proxy.port);
    let response = client
        .get("http://127.0.0.1:1/refused")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 502);
    let failed = engine
        .list_exchanges(&session.id, None, 10, 0)
        .unwrap()
        .remove(0);
    assert!(failed.completed);
    assert!(failed.error.unwrap().contains("Upstream"));
    engine.stop_proxy().await.unwrap();
    drop(engine);
    let db = rusqlite::Connection::open(dir.path().join("scope.sqlite3")).unwrap();
    db.execute(
        "UPDATE exchanges SET data=json_set(data,'$.completed',json('false'),'$.error',NULL)",
        [],
    )
    .unwrap();
    drop(db);
    let reopened = Engine::open(dir.path()).unwrap();
    let recovered = reopened
        .list_exchanges(&session.id, None, 10, 0)
        .unwrap()
        .remove(0);
    assert!(recovered.completed);
    assert!(recovered.error.unwrap().contains("interrupted"));
}

async fn held_replay_fixture() -> (
    String,
    Arc<tokio::sync::Notify>,
    Arc<tokio::sync::Notify>,
    JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let (entered_server, release_server) = (entered.clone(), release.clone());
    let task = tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let (entered, release) = (entered_server.clone(), release_server.clone());
            tokio::spawn(async move {
                let handler = service_fn(move |req: Request<hudsucker::hyper::body::Incoming>| {
                    let (entered, release) = (entered.clone(), release.clone());
                    async move {
                        if req.uri().path() == "/held" {
                            entered.notify_one();
                            release.notified().await;
                        }
                        Ok::<_, Infallible>(Response::new(Body::from("complete")))
                    }
                });
                let _ = auto::Builder::new(TokioExecutor::new())
                    .serve_connection(TokioIo::new(stream), handler)
                    .await;
            });
        }
    });
    (base, entered, release, task)
}

#[tokio::test]
async fn stopping_proxy_preserves_live_replay_ownership_until_its_final_archive_write() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let session = engine
        .create_session("Replay ownership".into(), "test".into())
        .unwrap();
    let (base, entered, release, fixture_task) = held_replay_fixture().await;
    let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
    let seed_client = client(&engine, proxy.port);
    assert_eq!(
        seed_client
            .get(format!("{base}/seed"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "complete"
    );
    drop(seed_client);
    let original = engine
        .list_exchanges(&session.id, None, 10, 0)
        .unwrap()
        .remove(0);
    let replay_engine = engine.clone();
    let task = tokio::spawn(async move {
        replay_engine
            .replay(
                &original.id,
                Some(EditMessage {
                    action: "continue".into(),
                    url: Some(format!("{base}/held")),
                    ..Default::default()
                }),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    let replay_id = engine
        .list_exchanges(&session.id, None, 10, 0)
        .unwrap()
        .into_iter()
        .find(|exchange| !exchange.completed)
        .unwrap()
        .id;
    tokio::time::timeout(Duration::from_secs(6), engine.stop_proxy())
        .await
        .unwrap()
        .unwrap();
    assert!(
        !engine.get_exchange(&replay_id).unwrap().completed,
        "Proxy Stop finalized an independently running replay"
    );
    assert!(engine
        .delete_session(&session.id)
        .unwrap_err()
        .to_string()
        .contains("active replays"));

    // Deletion must depend on actual ownership even if another archive operation
    // changes the row's completion flag while the network operation is alive.
    let db = rusqlite::Connection::open(dir.path().join("scope.sqlite3")).unwrap();
    db.execute(
        "UPDATE exchanges SET data=json_set(data,'$.completed',json('true')) WHERE id=?1",
        [&replay_id],
    )
    .unwrap();
    drop(db);
    assert!(engine
        .delete_session(&session.id)
        .unwrap_err()
        .to_string()
        .contains("active replays"));
    release.notify_one();
    let replay = tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(replay.id, replay_id);
    assert!(replay.completed && replay.error.is_none());
    assert_eq!(
        engine.get_exchange(&replay_id).unwrap().response_body,
        "complete"
    );
    engine.delete_session(&session.id).unwrap();
    fixture_task.abort();
}

async fn rule_pending(engine: &Engine, stage: &str) -> scope_core::PendingExchange {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(pending) = engine.pending().into_iter().find(|p| p.stage == stage) {
                return pending;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("The matching endpoint must reach its manual breakpoint")
}

#[test]
fn pause_rule_contract_is_strict_durable_and_uses_the_capture_matcher() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let mut saved = Vec::new();
    for stage in [PauseStage::Request, PauseStage::Response, PauseStage::Both] {
        saved.push(
            engine
                .save_rule(rule(
                    1,
                    json!({"host":"EXAMPLE.COM", "method":"post", "pathPrefix":"/checkout"}),
                    json!({"type":"pause", "stage": stage}),
                ))
                .unwrap(),
        );
    }
    assert!(engine.has_pause_rule_for("POST", "https://example.com/checkout/confirm?currency=PLN"));
    assert!(!engine.has_pause_rule_for("GET", "https://example.com/checkout"));
    assert!(!engine.has_pause_rule_for("POST", "https://other.test/checkout"));
    assert!(!engine.has_pause_rule_for("POST", "https://example.com/account"));
    for action in [
        json!({"type":"pause"}),
        json!({"type":"pause", "stage":"invalid"}),
        json!({"type":"pause", "stage":"REQUEST"}),
        json!({"type":"pause", "stage":null}),
        json!({"type":"pause", "stage":{"request":null}}),
        json!({"type":"pause", "stage":["request"]}),
        json!({"type":"pause", "stage":1}),
        json!({"type":"pause", "stage":"request", "body":"not an automatic edit"}),
    ] {
        assert!(serde_json::from_value::<TrafficRule>(json!({
            "id":saved[0].id,"name":"Invalid replacement","enabled":true,
            "priority":1,"matcher":{},"action":action
        }))
        .is_err());
    }
    drop(engine);
    let reopened = Engine::open(dir.path()).unwrap();
    assert_eq!(reopened.list_rules().unwrap().len(), 3);
    for expected in saved {
        assert!(reopened.list_rules().unwrap().iter().any(|actual| {
            actual.id == expected.id
                && serde_json::to_value(&actual.action).unwrap()
                    == serde_json::to_value(&expected.action).unwrap()
        }));
    }
}

#[tokio::test]
async fn endpoint_pause_edits_real_http_and_https_once_per_stage_using_the_original_snapshot() {
    for tls in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::open(dir.path()).unwrap();
        let session = engine
            .create_session("Endpoint manual edits".into(), "test".into())
            .unwrap();
        let (url, requests, server) = fixture(&engine, tls).await;
        let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
        let client = client(&engine, proxy.port);
        // Positive control: the same proxy and upstream are live before testing withholding.
        assert_eq!(
            client
                .get(format!("{url}/control"))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        let matcher = json!({"method":"POST", "pathPrefix":"/endpoint"});
        let both = engine
            .save_rule(rule(
                1,
                matcher.clone(),
                json!({"type":"pause","stage":"both"}),
            ))
            .unwrap();
        let req_pause = engine
            .save_rule(rule(
                2,
                matcher.clone(),
                json!({"type":"pause","stage":"request"}),
            ))
            .unwrap();
        let res_pause = engine
            .save_rule(rule(
                3,
                matcher.clone(),
                json!({"type":"pause","stage":"response"}),
            ))
            .unwrap();
        engine.save_rule(rule(10, matcher.clone(), json!({"type":"rewriteRequest", "method":"PATCH", "url":format!("{url}/automatic"), "body":"automatic request"}))).unwrap();
        let response_rule = engine
            .save_rule(rule(
                20,
                matcher,
                json!({"type":"rewriteResponse", "status":202, "body":"automatic response"}),
            ))
            .unwrap();
        let rewritten_match = engine
            .save_rule(rule(
                30,
                json!({"pathPrefix":"/automatic"}),
                json!({"type":"pause","stage":"both"}),
            ))
            .unwrap();
        let manual_match = engine
            .save_rule(rule(
                31,
                json!({"method":"PUT","pathPrefix":"/edited"}),
                json!({"type":"pause","stage":"both"}),
            ))
            .unwrap();
        engine.set_breakpoints(true, true);
        let request_client = client.clone();
        let original_url = format!("{url}/endpoint");
        let request = tokio::spawn(async move {
            request_client
                .post(original_url)
                .body("original request")
                .send()
                .await
                .unwrap()
        });
        let pending_request = rule_pending(&engine, "request").await;
        assert_eq!(engine.pending().len(), 1);
        assert_eq!(pending_request.method, "PATCH");
        assert_eq!(pending_request.url, format!("{url}/automatic"));
        assert_eq!(pending_request.body, "automatic request");
        assert_eq!(
            requests.load(Ordering::SeqCst),
            1,
            "Nothing reached upstream while request is pending"
        );
        assert!(!request.is_finished());
        assert!(engine
            .resolve_breakpoint(
                &pending_request.id,
                EditMessage {
                    action: "continue".into(),
                    method: Some("CONNECT".into()),
                    ..Default::default()
                }
            )
            .is_err());
        assert_eq!(engine.pending()[0].id, pending_request.id);
        // Updates/deletions affect the next exchange, not the held snapshot.
        engine.delete_rule(&both.id).unwrap();
        engine.delete_rule(&req_pause.id).unwrap();
        engine.delete_rule(&res_pause.id).unwrap();
        engine.delete_rule(&response_rule.id).unwrap();
        engine
            .resolve_breakpoint(
                &pending_request.id,
                EditMessage {
                    action: "continue".into(),
                    method: Some("PUT".into()),
                    url: Some(format!("{url}/edited")),
                    headers: Some(vec![Header {
                        name: "x-rule".into(),
                        value: "manual request header".into(),
                    }]),
                    body: Some("upstream-edited".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(engine
            .resolve_breakpoint(&pending_request.id, EditMessage::continue_unchanged())
            .is_err());
        let pending_response = rule_pending(&engine, "response").await;
        assert_ne!(pending_response.id, pending_request.id);
        assert_eq!(pending_response.exchange_id, pending_request.exchange_id);
        assert_eq!(
            engine.pending().len(),
            1,
            "Global + multiple matching rules share one pause"
        );
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        assert_eq!(pending_response.status, Some(202));
        assert_eq!(pending_response.body, "automatic response");
        for (name, value) in [
            ("x-upstream-path", "/edited"),
            ("x-upstream-method", "PUT"),
            ("x-saw-rule", "manual request header"),
        ] {
            assert!(pending_response
                .headers
                .iter()
                .any(|h| h.name == name && h.value == value));
        }
        assert!(
            !request.is_finished(),
            "Client response is withheld until manual continuation"
        );
        let held = engine.get_exchange(&pending_request.exchange_id).unwrap();
        assert!(!held.completed);
        assert_eq!(held.status, Some(202));
        assert_eq!(held.response_body, "automatic response");
        let pauses: Vec<_> = held
            .applied_rules
            .iter()
            .filter(|r| r.action == "pause")
            .map(|r| (r.rule_id.as_str(), r.stage.as_str()))
            .collect();
        assert_eq!(
            pauses,
            vec![
                (both.id.as_str(), "request"),
                (req_pause.id.as_str(), "request"),
                (both.id.as_str(), "response"),
                (res_pause.id.as_str(), "response")
            ]
        );
        assert!(held
            .applied_rules
            .iter()
            .all(|r| r.rule_id != rewritten_match.id && r.rule_id != manual_match.id));
        assert!(engine
            .resolve_breakpoint(
                &pending_response.id,
                EditMessage {
                    action: "continue".into(),
                    status: Some(101),
                    ..Default::default()
                }
            )
            .is_err());
        assert_eq!(engine.pending()[0].id, pending_response.id);
        engine
            .resolve_breakpoint(
                &pending_response.id,
                EditMessage {
                    action: "continue".into(),
                    status: Some(201),
                    headers: Some(vec![Header {
                        name: "x-client".into(),
                        value: "manual response header".into(),
                    }]),
                    body: Some("client-edited".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(3), request)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response.status(), 201);
        assert_eq!(response.headers()["x-client"], "manual response header");
        assert_eq!(response.text().await.unwrap(), "client-edited");
        assert!(engine.pending().is_empty());
        let completed = engine.get_exchange(&pending_request.exchange_id).unwrap();
        assert!(completed.completed && completed.error.is_none());
        assert_eq!(
            completed.original_request.as_ref().unwrap()["body"],
            "original request"
        );
        assert_eq!(
            completed.original_response.as_ref().unwrap()["body"],
            "upstream-edited"
        );
        engine.set_breakpoints(false, false);
        // The deleted snapshot no longer pauses or rewrites the next response.
        let next = client
            .post(format!("{url}/endpoint"))
            .body("next")
            .send()
            .await
            .unwrap();
        assert_eq!(next.status(), 200);
        assert_eq!(next.text().await.unwrap(), "automatic request");
        assert!(engine.pending().is_empty());
        engine.stop_proxy().await.unwrap();
        server.abort();
    }
}

#[tokio::test]
async fn endpoint_pause_includes_mock_and_blocked_responses_and_replay_never_pauses() {
    for terminal in [
        json!({"type":"mockResponse","status":201,"body":"mock"}),
        json!({"type":"block","reason":"fixture"}),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::open(dir.path()).unwrap();
        let session = engine
            .create_session("Terminal pauses".into(), "test".into())
            .unwrap();
        let (url, requests, server) = fixture(&engine, false).await;
        let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
        let client = client(&engine, proxy.port);
        assert_eq!(
            client
                .get(format!("{url}/control"))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        let matcher = json!({"pathPrefix":"/terminal"});
        engine
            .save_rule(rule(0, matcher.clone(), terminal.clone()))
            .unwrap();
        engine.save_rule(rule(1, matcher.clone(), json!({"type":"rewriteResponse","status":202,"body":"after automatic response rule"}))).unwrap();
        let pause = engine
            .save_rule(rule(99, matcher, json!({"type":"pause","stage":"both"})))
            .unwrap();
        let request_url = format!("{url}/terminal");
        let request = tokio::spawn(async move { client.get(request_url).send().await.unwrap() });
        let pending_request = rule_pending(&engine, "request").await;
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        engine
            .resolve_breakpoint(&pending_request.id, EditMessage::continue_unchanged())
            .unwrap();
        let pending_response = rule_pending(&engine, "response").await;
        assert_eq!(pending_response.status, Some(202));
        assert_eq!(pending_response.body, "after automatic response rule");
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        assert!(!request.is_finished());
        engine
            .resolve_breakpoint(&pending_response.id, EditMessage::continue_unchanged())
            .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(3), request)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response.status(), 202);
        assert_eq!(
            response.text().await.unwrap(),
            "after automatic response rule"
        );
        let captured = engine.get_exchange(&pending_request.exchange_id).unwrap();
        assert_eq!(
            captured.provenance,
            if terminal["type"] == "block" {
                TrafficProvenance::Blocked
            } else {
                TrafficProvenance::Mock
            }
        );
        assert_eq!(
            captured
                .applied_rules
                .iter()
                .filter(|r| r.rule_id == pause.id)
                .map(|r| r.stage.as_str())
                .collect::<Vec<_>>(),
            vec!["request", "response"]
        );
        engine.set_breakpoints(true, true);
        let replay =
            tokio::time::timeout(Duration::from_secs(3), engine.replay(&captured.id, None))
                .await
                .unwrap()
                .unwrap();
        assert!(replay.completed && replay.error.is_none());
        assert_eq!(replay.provenance, captured.provenance);
        assert_eq!(replay.response_body, "after automatic response rule");
        assert!(replay.applied_rules.iter().all(|r| r.action != "pause"));
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        assert!(engine.pending().is_empty());
        engine.stop_proxy().await.unwrap();
        server.abort();
    }
}

#[tokio::test]
async fn endpoint_pause_respects_stage_disabled_nonmatching_and_upstream_replay() {
    for stage in ["request", "response"] {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::open(dir.path()).unwrap();
        let session = engine
            .create_session("Selective pauses".into(), "test".into())
            .unwrap();
        let (url, requests, server) = fixture(&engine, false).await;
        let pause = engine
            .save_rule(rule(
                1,
                json!({"method":"POST","pathPrefix":"/matching"}),
                json!({"type":"pause","stage":stage}),
            ))
            .unwrap();
        let mut disabled = rule(
            2,
            json!({"pathPrefix":"/disabled"}),
            json!({"type":"pause","stage":"both"}),
        );
        disabled.enabled = false;
        engine.save_rule(disabled).unwrap();
        assert!(!engine.has_pause_rule_for("GET", &format!("{url}/matching")));
        assert!(!engine.has_pause_rule_for("POST", &format!("{url}/disabled")));
        assert!(engine.has_pause_rule_for("POST", &format!("{url}/matching")));
        let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
        let client = client(&engine, proxy.port);
        for path in ["/disabled", "/other", "/matching"] {
            assert_eq!(
                client
                    .get(format!("{url}{path}"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                200
            );
        }
        assert_eq!(requests.load(Ordering::SeqCst), 3);
        assert!(engine.pending().is_empty());
        assert!(engine
            .list_exchanges(&session.id, None, 10, 0)
            .unwrap()
            .iter()
            .all(|e| e.applied_rules.is_empty()));
        let request_url = format!("{url}/matching");
        let request =
            tokio::spawn(
                async move { client.post(request_url).body("echo").send().await.unwrap() },
            );
        let pending = rule_pending(&engine, stage).await;
        assert_eq!(engine.pending().len(), 1);
        assert_eq!(
            requests.load(Ordering::SeqCst),
            if stage == "request" { 3 } else { 4 }
        );
        engine
            .resolve_breakpoint(&pending.id, EditMessage::continue_unchanged())
            .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(3), request)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response.text().await.unwrap(), "echo");
        let captured = engine.get_exchange(&pending.exchange_id).unwrap();
        assert_eq!(captured.applied_rules.len(), 1);
        assert_eq!(captured.applied_rules[0].rule_id, pause.id);
        assert_eq!(captured.applied_rules[0].stage, stage);
        engine.set_breakpoints(true, true);
        let replay =
            tokio::time::timeout(Duration::from_secs(3), engine.replay(&captured.id, None))
                .await
                .unwrap()
                .unwrap();
        assert_eq!(replay.response_body, "echo");
        assert!(replay.error.is_none() && replay.applied_rules.is_empty());
        assert_eq!(requests.load(Ordering::SeqCst), 5);
        assert!(engine.pending().is_empty());
        engine.stop_proxy().await.unwrap();
        server.abort();
    }
}

#[tokio::test]
async fn endpoint_pause_abort_and_proxy_stop_clear_pending_and_retain_stage_evidence() {
    for stage in ["request", "response"] {
        for stop in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let engine = Engine::open(dir.path()).unwrap();
            let session = engine
                .create_session("Pause cancellation".into(), "test".into())
                .unwrap();
            let (url, requests, server) = fixture(&engine, false).await;
            engine
                .save_rule(rule(
                    1,
                    json!({"pathPrefix":"/cancel"}),
                    json!({"type":"pause","stage":stage}),
                ))
                .unwrap();
            engine.save_rule(rule(2, json!({"pathPrefix":"/cancel"}), json!({"type":"rewriteResponse","status":202,"body":"persisted before response pause"}))).unwrap();
            let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
            let client = client(&engine, proxy.port);
            assert_eq!(
                client
                    .get(format!("{url}/control"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                200
            );
            let request_client = client.clone();
            let request_url = format!("{url}/cancel");
            let request = tokio::spawn(async move { request_client.get(request_url).send().await });
            let pending = rule_pending(&engine, stage).await;
            if stop {
                tokio::time::timeout(Duration::from_secs(8), engine.stop_proxy())
                    .await
                    .unwrap()
                    .unwrap();
            } else {
                engine
                    .resolve_breakpoint(
                        &pending.id,
                        EditMessage {
                            action: "abort".into(),
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
            let result = tokio::time::timeout(Duration::from_secs(3), request)
                .await
                .unwrap()
                .unwrap();
            if let Ok(response) = result {
                assert_eq!(response.status(), 502);
                assert!(response
                    .text()
                    .await
                    .unwrap()
                    .contains("aborted at breakpoint"));
            } else {
                assert!(
                    stop,
                    "Explicit abort returns an HTTP failure through the running proxy"
                );
            }
            assert!(engine.pending().is_empty());
            assert!(engine
                .resolve_breakpoint(&pending.id, EditMessage::continue_unchanged())
                .is_err());
            let captured = engine.get_exchange(&pending.exchange_id).unwrap();
            assert!(captured.completed && captured.error.is_some());
            assert_eq!(
                captured
                    .applied_rules
                    .iter()
                    .filter(|r| r.action == "pause")
                    .map(|r| r.stage.as_str())
                    .collect::<Vec<_>>(),
                vec![stage]
            );
            assert_eq!(
                requests.load(Ordering::SeqCst),
                if stage == "request" { 1 } else { 2 }
            );
            if stage == "response" {
                assert_eq!(captured.status, Some(202));
                assert_eq!(captured.response_body, "persisted before response pause");
                assert!(captured
                    .applied_rules
                    .iter()
                    .any(|r| r.action == "rewriteResponse"));
            }
            if !stop {
                assert_eq!(
                    client
                        .get(format!("{url}/control-after-abort"))
                        .send()
                        .await
                        .unwrap()
                        .status(),
                    200
                );
                engine.stop_proxy().await.unwrap();
            }
            server.abort();
        }
    }
}

#[tokio::test]
async fn endpoint_pause_timeout_fails_closed_at_either_stage_and_retains_evidence() {
    for stage in ["request", "response"] {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::open(dir.path()).unwrap();
        let session = engine
            .create_session("Pause expiry".into(), "test".into())
            .unwrap();
        let (url, requests, server) = fixture(&engine, false).await;
        let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
        let client = client_with_timeout(&engine, proxy.port, Duration::from_secs(120));
        assert_eq!(
            client
                .get(format!("{url}/control"))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        engine
            .save_rule(rule(
                1,
                json!({"pathPrefix":"/expire"}),
                json!({"type":"pause","stage":"both"}),
            ))
            .unwrap();
        engine
            .save_rule(rule(
                2,
                json!({"pathPrefix":"/expire"}),
                json!({"type":"rewriteResponse","status":202,"body":"retained after expiry"}),
            ))
            .unwrap();
        let request_client = client.clone();
        let request_url = format!("{url}/expire");
        let request =
            tokio::spawn(async move { request_client.get(request_url).send().await.unwrap() });
        let mut pending = rule_pending(&engine, "request").await;
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        if stage == "response" {
            engine
                .resolve_breakpoint(&pending.id, EditMessage::continue_unchanged())
                .unwrap();
            pending = rule_pending(&engine, "response").await;
            assert_eq!(pending.body, "retained after expiry");
            assert_eq!(requests.load(Ordering::SeqCst), 2);
        }
        // Freeze only once the real socket request is positively observed waiting;
        // advance the existing production timer instead of sleeping for a minute.
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(
            scope_core::BREAKPOINT_TIMEOUT_SECONDS + 1,
        ))
        .await;
        tokio::time::resume();
        let response = tokio::time::timeout(Duration::from_secs(3), request)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response.status(), 502);
        assert!(response
            .text()
            .await
            .unwrap()
            .contains(&format!("{stage} breakpoint expired")));
        assert_eq!(
            requests.load(Ordering::SeqCst),
            if stage == "request" { 1 } else { 2 }
        );
        assert!(engine.pending().is_empty());
        let expired = engine.get_exchange(&pending.exchange_id).unwrap();
        assert!(
            expired.completed
                && expired
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("breakpoint expired")
        );
        assert_eq!(
            expired
                .applied_rules
                .iter()
                .filter(|r| r.action == "pause")
                .map(|r| r.stage.as_str())
                .collect::<Vec<_>>(),
            if stage == "request" {
                vec!["request"]
            } else {
                vec!["request", "response"]
            }
        );
        if stage == "response" {
            assert_eq!(expired.status, Some(202));
            assert_eq!(expired.response_body, "retained after expiry");
            assert!(expired
                .applied_rules
                .iter()
                .any(|r| r.action == "rewriteResponse"));
        }
        assert!(engine
            .resolve_breakpoint(&pending.id, EditMessage::continue_unchanged())
            .is_err());
        assert_eq!(
            client
                .get(format!("{url}/control-after-expiry"))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        engine.stop_proxy().await.unwrap();
        server.abort();
    }
}

#[tokio::test]
async fn cancelled_or_invalid_replays_release_ownership_and_finalize_interrupted_rows() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let session = engine
        .create_session("Replay cancellation".into(), "test".into())
        .unwrap();
    let (base, entered, release, fixture_task) = held_replay_fixture().await;
    let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
    let seed_client = client(&engine, proxy.port);
    seed_client
        .get(format!("{base}/seed"))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    drop(seed_client);
    let original = engine
        .list_exchanges(&session.id, None, 10, 0)
        .unwrap()
        .remove(0);
    assert!(engine
        .replay(
            &original.id,
            Some(EditMessage {
                action: "invalid".into(),
                ..Default::default()
            })
        )
        .await
        .is_err());
    let replay_engine = engine.clone();
    let task = tokio::spawn(async move {
        replay_engine
            .replay(
                &original.id,
                Some(EditMessage {
                    action: "continue".into(),
                    url: Some(format!("{base}/held")),
                    ..Default::default()
                }),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    let replay_id = engine
        .list_exchanges(&session.id, None, 10, 0)
        .unwrap()
        .into_iter()
        .find(|exchange| !exchange.completed)
        .unwrap()
        .id;
    tokio::time::timeout(Duration::from_secs(6), engine.stop_proxy())
        .await
        .unwrap()
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let replay = engine.get_exchange(&replay_id).unwrap();
    assert!(replay.completed);
    assert!(replay.error.unwrap().contains("Replay interrupted"));
    engine.delete_session(&session.id).unwrap();
    release.notify_one();
    fixture_task.abort();
}
