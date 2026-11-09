use super::*;

// Catches silently accepting missing/reordered rule evidence or wrong request pairing.
#[test]
fn evidence_detector_rejects_corrupted_archive_rows() {
    let rules = ["a".to_string(), "b".to_string(), "c".to_string()];
    let session = "synthetic";
    let dir = tempfile::tempdir().unwrap();
    let engine = Engine::open(dir.path()).unwrap();
    let mut rows = Vec::new();
    for i in 0..32 {
        let mut e = engine.fresh_exchange(
            session,
            "GET".into(),
            format!("http://localhost:1/pause?i={i}"),
        );
        e.completed = true;
        e.provenance = TrafficProvenance::Mock;
        e.status = Some(201);
        e.request_headers.push(Header {
            name: "x-measure-index".into(),
            value: i.to_string(),
        });
        e.request_body = "request rule".into();
        e.response_body = "response rule".into();
        e.original_request = Some(
            serde_json::json!({"body":"", "method":"GET", "url":e.url, "headers":[{"name":"x-measure-index","value":i.to_string()}]}),
        );
        e.original_response = Some(serde_json::json!({"status":201,"body":"mock body"}));
        for (n, (action, stage)) in [
            ("rewriteRequest", "request"),
            ("mockResponse", "request"),
            ("rewriteResponse", "response"),
        ]
        .iter()
        .enumerate()
        {
            e.applied_rules.push(AppliedRule {
                rule_id: rules[n].clone(),
                name: format!("Rule {}", n + 1),
                action: action.to_string(),
                stage: stage.to_string(),
            });
        }
        rows.push(e);
    }
    assert!(valid_evidence(&rows, session, &rules));
    let mut swapped = rows.clone();
    swapped[0].original_request = rows[1].original_request.clone();
    swapped[1].original_request = rows[0].original_request.clone();
    assert!(!valid_evidence(&swapped, session, &rules));
    let mut missing = rows.clone();
    missing.pop();
    assert!(!valid_evidence(&missing, session, &rules));
    let mut corrupt = rows.clone();
    corrupt[0].applied_rules.swap(0, 1);
    assert!(!valid_evidence(&corrupt, session, &rules));
    let mut corrupt = rows.clone();
    corrupt[0].applied_rules[1].rule_id = "wrong".into();
    assert!(!valid_evidence(&corrupt, session, &rules));
    let mut corrupt = rows.clone();
    corrupt[0].completed = false;
    assert!(!valid_evidence(&corrupt, session, &rules));
    let mut corrupt = rows.clone();
    corrupt[0].url = rows[1].url.clone();
    assert!(!valid_evidence(&corrupt, session, &rules));
    let mut corrupt = rows.clone();
    corrupt[0].id = rows[1].id.clone();
    assert!(!valid_evidence(&corrupt, session, &rules));
    let mut corrupt = rows.clone();
    corrupt[0].original_response = None;
    assert!(!valid_evidence(&corrupt, session, &rules));
}

// Catches unbounded collection and falsely claiming complete measurements after overflow.
#[test]
fn sample_collection_caps_memory_and_counts_overflow() {
    let collector = SaveMetrics::default();
    collector.enabled.store(true, Ordering::Relaxed);
    for _ in 0..SAVE_CAP + 3 {
        collector.push(SaveSample::default());
    }
    let samples = collector.take();
    assert_eq!(samples.samples.len(), SAVE_CAP);
    assert_eq!(samples.dropped, 3);
}
