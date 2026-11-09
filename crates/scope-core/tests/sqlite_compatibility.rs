use rusqlite::{params, Connection};
use scope_core::{Engine, SessionUpdate};
use serde_json::json;

#[test]
fn linked_sqlite_includes_the_wal_page_cache_fix() {
    // RUSTSEC-2026-0459: pinning rusqlite alone must not silently select an older
    // bundled or system SQLite. Check the library used by a real connection.
    let db = Connection::open_in_memory().unwrap();
    let actual: String = db
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .unwrap();
    assert_eq!(actual, rusqlite::version());
    assert!(
        rusqlite::version_number() >= 3_054_000,
        "SQLite {actual} predates the 3.54.0 WAL page-cache fix"
    );
}

#[test]
fn legacy_scope_schema_with_committed_wal_survives_open_update_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scope.sqlite3");
    // Reproduce Scope's original on-disk schema before metadata/revision/rules
    // migrations. Keep this writer open so committed data actually lives in WAL
    // while the current Engine opens and reads the archive.
    let legacy_writer = Connection::open(&path).unwrap();
    legacy_writer
        .execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
             CREATE TABLE sessions(id TEXT PRIMARY KEY, name TEXT NOT NULL,
               source TEXT NOT NULL, created_at INTEGER NOT NULL);
             CREATE TABLE exchanges(id TEXT PRIMARY KEY, session_id TEXT NOT NULL
               REFERENCES sessions(id), started_at INTEGER NOT NULL, data TEXT NOT NULL);
             CREATE INDEX exchanges_session_time ON exchanges(session_id, started_at DESC);
             INSERT INTO sessions VALUES('legacy-session','Legacy fixture','desktop',1700000000000);",
        )
        .unwrap();
    let captured = json!({
        "id":"legacy-exchange", "sessionId":"legacy-session", "method":"POST",
        "url":"https://fixture.example.test/items", "status":200,
        "requestHeaders":[{"name":"x-fixture","value":"first"},{"name":"x-fixture","value":"second"}],
        "responseHeaders":[{"name":"content-type","value":"application/json"}],
        "requestBody":"synthetic request 🦀", "responseBody":"{\"ok\":true}",
        "requestBodyBase64":false, "responseBodyBase64":false,
        "startedAt":1700000000000_i64, "durationMs":17, "requestBytes":22,
        "responseBytes":11, "error":null, "edited":true,
        "originalRequest":{"method":"GET","url":"https://fixture.example.test/items"},
        "originalResponse":{"status":201,"body":"original fixture"}
    });
    legacy_writer
        .execute(
            "INSERT INTO exchanges VALUES(?1,?2,?3,?4)",
            params![
                "legacy-exchange",
                "legacy-session",
                1700000000000_i64,
                captured.to_string()
            ],
        )
        .unwrap();
    assert!(
        std::fs::metadata(dir.path().join("scope.sqlite3-wal"))
            .unwrap()
            .len()
            > 0
    );

    let engine = Engine::open(dir.path()).unwrap();
    let sessions = engine.list_sessions().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].exchange_count, 1);
    assert_eq!(sessions[0].revision, 0);
    let exchange = engine.get_exchange("legacy-exchange").unwrap();
    assert!(
        exchange.completed,
        "old archives default to completed captures"
    );
    assert_eq!(exchange.request_headers.len(), 2);
    assert_eq!(exchange.request_body, "synthetic request 🦀");
    assert_eq!(exchange.response_body, "{\"ok\":true}");
    assert_eq!(
        exchange.original_request,
        Some(captured["originalRequest"].clone())
    );
    assert_eq!(
        exchange.original_response,
        Some(captured["originalResponse"].clone())
    );
    engine
        .update_session(
            "legacy-session",
            SessionUpdate {
                notes: Some("Preserved after SQLite upgrade".into()),
                tags: Some(vec!["legacy".into(), "wal".into()]),
                pinned: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
    let archived = engine.export_session("legacy-session").unwrap();
    engine
        .create_session("New fixture".into(), "desktop".into())
        .unwrap();
    drop(engine);
    drop(legacy_writer);

    let reopened = Engine::open(dir.path()).unwrap();
    assert_eq!(reopened.list_sessions().unwrap().len(), 2);
    assert_eq!(reopened.export_session("legacy-session").unwrap(), archived);
    let check = Connection::open(path).unwrap();
    assert_eq!(
        check
            .query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(!check
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
}
