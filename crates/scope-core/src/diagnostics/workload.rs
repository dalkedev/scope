use super::*;
use serde_json::json;
use tokio::task::JoinSet;

pub(super) async fn run(samples: usize, workers: usize) -> Value {
    let mut cases = Vec::new();
    let mut passed = true;
    for tls in [false, true] {
        for sample in 0..samples {
            let case = batch(tls, sample).await;
            passed &= case["status"] == "PASS";
            cases.push(case);
        }
    }
    // Known blocking case exercises both probes on an actual current-thread runtime.
    // On the two-worker runtime the other worker can progress: control intentionally NOT RUN.
    let timer_control = if workers == 0 {
        let probes = probes::Probes::start().await;
        let control_start = Instant::now();
        std::thread::sleep(Duration::from_millis(300));
        let control_end = Instant::now();
        let report = probes.finish(control_start, control_end).await;
        let detected = report.detects_control();
        passed &= detected;
        json!({"status":if detected {"PASS"} else {"FAIL"}, "injected_block_ms":300,"detector_threshold_ms":100,"probes":report})
    } else {
        json!({"status":"NOTRUN","reason":"intentional blocking control requires current-thread runtime"})
    };
    json!({"schema":1,"status":if passed {"PASS"} else {"FAIL"},"workers":workers,
        "profile":if cfg!(debug_assertions) {"debug"} else {"release"},
        "runtime":if workers==0 {"current_thread"} else {"multi_thread"},
        "cases":cases,"timer_network_positive_control":timer_control,
        "not_run":{"queue_wait":"no writer queue exists", "disk_flush_count":"not observed", "sqlite_busy_wait_separate":"included in statement elapsed", "production_performance":"synthetic test-only harness", "gui_android":"outside harness"}})
}

fn install_rules(engine: &Engine) -> [String; 3] {
    let actions = [
        json!({"type":"rewriteRequest","body":"request rule"}),
        json!({"type":"mockResponse","status":201,"body":"mock body"}),
        json!({"type":"rewriteResponse","body":"response rule"}),
    ];
    std::array::from_fn(|i| {
        engine
            .save_rule(
                serde_json::from_value(
                    json!({"id":"","name":format!("Rule {}",i+1),"enabled":true,"priority":i+1,
            "matcher":{"pathPrefix":"/pause"},"action":actions[i]}),
                )
                .unwrap(),
            )
            .unwrap()
            .id
    })
}
fn sqlite_settings(engine: &Engine) -> Value {
    let db = engine.inner.db.lock().unwrap();
    let text = |pragma: &str| db.query_row(pragma, [], |r| r.get::<_, String>(0)).unwrap();
    let integer = |pragma: &str| db.query_row(pragma, [], |r| r.get::<_, i64>(0)).unwrap();
    let mut stmt = db.prepare("PRAGMA compile_options").unwrap();
    let options: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    json!({"version":rusqlite::version(),"journal_mode":text("PRAGMA journal_mode"),
        "synchronous":integer("PRAGMA synchronous"),"busy_timeout_ms":integer("PRAGMA busy_timeout"),
        "page_size":integer("PRAGMA page_size"),"cache_size":integer("PRAGMA cache_size"),
        "wal_autocheckpoint":integer("PRAGMA wal_autocheckpoint"),"compile_options":options})
}
async fn batch(tls: bool, sample: usize) -> Value {
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let session = engine
        .create_session("Synthetic measurement".into(), "test".into())
        .unwrap();
    let rules = install_rules(&engine);
    let settings = sqlite_settings(&engine);
    let revision_before = engine.session(&session.id).unwrap().revision;
    let proxy = engine.start_proxy(session.id.clone(), 0).await.unwrap();
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{}", proxy.port)).unwrap())
        .add_root_certificate(
            reqwest::Certificate::from_pem(&std::fs::read(engine.ca_cert_path()).unwrap()).unwrap(),
        )
        .timeout(Duration::from_secs(30))
        .http1_only()
        .build()
        .unwrap();
    let probes = probes::Probes::start().await;
    engine
        .inner
        .save_metrics
        .enabled
        .store(true, Ordering::Relaxed);
    let began = Instant::now();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let barrier = Arc::new(tokio::sync::Barrier::new(CLIENTS + 1));
    let mut clients = JoinSet::new();
    for i in 0..CLIENTS {
        let (client, barrier) = (client.clone(), barrier.clone());
        clients.spawn(async move {
            barrier.wait().await;
            let started = Instant::now();
            let response = async {
                let response = client
                    .get(format!(
                        "{}://localhost:1/pause?i={i}",
                        if tls { "https" } else { "http" }
                    ))
                    .header("x-measure-index", i.to_string())
                    .send()
                    .await?;
                let status = response.status();
                let body = response.bytes().await?;
                Ok::<_, reqwest::Error>(status == 201 && body.as_ref() == b"response rule")
            }
            .await;
            (ns(started.elapsed()), response.unwrap_or(false))
        });
    }
    barrier.wait().await;
    let mut timed_out = false;
    let mut elapsed = Vec::with_capacity(CLIENTS);
    let mut successful = 0;
    let mut joined = 0;
    while !clients.is_empty() {
        let next = if timed_out {
            clients.join_next().await
        } else {
            match tokio::time::timeout_at(deadline, clients.join_next()).await {
                Ok(next) => next,
                Err(_) => {
                    timed_out = true;
                    clients.abort_all();
                    continue;
                }
            }
        };
        if let Some(next) = next {
            joined += 1;
            if let Ok((ns, ok)) = next {
                elapsed.push(ns);
                successful += usize::from(ok);
            }
        }
    }
    let batch_end = Instant::now();
    let batch_ns = ns(batch_end.duration_since(began));
    engine
        .inner
        .save_metrics
        .enabled
        .store(false, Ordering::Relaxed);
    let saves = engine.inner.save_metrics.take();
    let probe_report = probes.finish(began, batch_end).await;
    let shutdown = Instant::now();
    engine.stop_proxy().await.unwrap();
    let shutdown_ns = ns(shutdown.elapsed());
    drop(client);
    let rows = engine
        .list_exchanges(&session.id, None, CLIENTS + 1, 0)
        .unwrap();
    let evidence = valid_evidence(&rows, &session.id, &rules);
    let revision_delta = engine.session(&session.id).unwrap().revision - revision_before;
    let stored = rows.len();
    let original_ids = rows.iter().map(|r| r.id.clone()).collect::<BTreeSet<_>>();
    drop(rows);
    drop(engine);
    let reopened = Engine::open(dir.path()).unwrap();
    let rows = reopened
        .list_exchanges(&session.id, None, CLIENTS + 1, 0)
        .unwrap();
    let reopened_ok = valid_evidence(&rows, &session.id, &rules)
        && rows.iter().map(|r| r.id.clone()).collect::<BTreeSet<_>>() == original_ids;
    // Real persisted corruption in this disposable synthetic archive, after baseline readback.
    // Detect both row loss and lost rule evidence through the same archive validator.
    let db = reopened.inner.db.lock().unwrap();
    if let Some(row) = rows.first() {
        db.execute(
            "UPDATE exchanges SET data=json_remove(data,'$.appliedRules[1]') WHERE id=?1",
            [&row.id],
        )
        .unwrap();
    }
    drop(db);
    let corruption_detected = !valid_evidence(
        &reopened
            .list_exchanges(&session.id, None, CLIENTS + 1, 0)
            .unwrap(),
        &session.id,
        &rules,
    );
    // Restore the original persisted evidence before an independent row-loss control.
    if let Some(row) = rows.first() {
        reopened.save(row).unwrap();
    }
    let restored_ok = valid_evidence(
        &reopened
            .list_exchanges(&session.id, None, CLIENTS + 1, 0)
            .unwrap(),
        &session.id,
        &rules,
    );
    let db = reopened.inner.db.lock().unwrap();
    if let Some(row) = rows.first() {
        db.execute("DELETE FROM exchanges WHERE id=?1", [&row.id])
            .unwrap();
    }
    drop(db);
    let loss_detected = !valid_evidence(
        &reopened
            .list_exchanges(&session.id, None, CLIENTS + 1, 0)
            .unwrap(),
        &session.id,
        &rules,
    );
    let save_ok =
        saves.dropped == 0 && saves.samples.len() == 192 && saves.samples.iter().all(|s| s.success);
    let wall_budget_exceeded =
        batch_ns > 60_000_000_000 || elapsed.iter().any(|&n| n > 30_000_000_000);
    let passed = !wall_budget_exceeded
        && !timed_out
        && joined == 32
        && successful == 32
        && evidence
        && reopened_ok
        && save_ok
        && revision_delta == 192
        && corruption_detected
        && restored_ok
        && loss_detected
        && probe_report.covered();
    json!({"status":if passed {"PASS"} else {"FAIL"},"sample":sample,"transport":if tls {"TLS"} else {"HTTP"},
        "clients":32,"joined":joined,"successful":successful,"client_elapsed_ns":elapsed,"batch_elapsed_ns":batch_ns,
        "shutdown_elapsed_ns":shutdown_ns,"cooperative_batch_timeout":timed_out,"wall_budget_exceeded":wall_budget_exceeded,"client_timeout_s":30,"batch_timeout_s":60,
        "persisted_rows":stored,"evidence_valid":evidence,"reopened_evidence_valid":reopened_ok,"revision_delta":revision_delta,
        "evidence_corruption_control_detected":corruption_detected,"evidence_loss_control_detected":loss_detected,"control_baseline_restored":restored_ok,
        "save_cap":SAVE_CAP,"saves":saves,"probes":probe_report,"sqlite":settings,
        "client_configuration":{"http":"1.1 only","pool":"shared client clones","tls":"fresh archive and client per batch; interception CA trusted; no external upstream","barrier":32},
        "storage":"fresh temporary file-backed archive; no PRAGMA overrides"})
}
