use anyhow::{anyhow, bail, Context, Result};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use scope_core::Engine;
use scope_device::DeviceManager;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpAccess {
    pub session_ids: Vec<String>,
    pub allow_writes: bool,
    #[serde(default)]
    pub allow_rules: bool,
    #[serde(default)]
    pub allow_device_diagnostics: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeConfig {
    pub endpoint: String,
    pub token: String,
}

pub struct AppCore {
    pub engine: Engine,
    pub devices: DeviceManager,
    pub data_dir: PathBuf,
    pub bridge: BridgeConfig,
    pub access: Mutex<McpAccess>,
    pub mcp_binary: PathBuf,
    demo_started: tokio::sync::Mutex<bool>,
}

pub(crate) fn write_private(path: &Path, content: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

impl AppCore {
    pub async fn open(
        data_dir: PathBuf,
        resources: Option<PathBuf>,
        requested_port: u16,
    ) -> Result<Arc<Self>> {
        std::fs::create_dir_all(&data_dir)?;
        let engine = Engine::open(&data_dir)?;
        let mut devices = DeviceManager::new(data_dir.join("device"));
        // Bundled device resources are resolved independently from runtime state.
        if let Some(ref root) = resources {
            std::env::set_var("SCOPE_RESOURCES", root.join("resources"));
        }
        let _ = &mut devices;
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", requested_port))
            .await
            .context("MCP bridge port is busy; another Scope instance may be open")?;
        let port = listener.local_addr()?.port();
        let bridge = BridgeConfig {
            endpoint: format!("http://127.0.0.1:{port}/api"),
            token: uuid::Uuid::new_v4().to_string(),
        };
        write_private(
            &data_dir.join("mcp-bridge.json"),
            &serde_json::to_vec_pretty(&bridge)?,
        )?;
        let suffix = if cfg!(windows) {
            "scope-mcp.exe"
        } else {
            "scope-mcp"
        };
        let beside = std::env::current_exe()?.parent().unwrap().join(suffix);
        let bundled = resources
            .as_ref()
            .map(|p| p.join("resources/bin").join(suffix));
        let mcp_binary = bundled.filter(|p| p.is_file()).unwrap_or(beside);
        let app = Arc::new(Self {
            engine,
            devices,
            data_dir,
            bridge,
            access: Mutex::new(McpAccess::default()),
            mcp_binary,
            demo_started: tokio::sync::Mutex::new(false),
        });
        let router = Router::new()
            .route("/api", post(bridge_call))
            .with_state(app.clone());
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                eprintln!("Scope MCP bridge: {e}");
            }
        });
        Ok(app)
    }

    pub fn mcp_status(&self) -> Value {
        let access = self.access.lock().unwrap().clone();
        json!({"sessionIds":access.session_ids,"allowWrites":access.allow_writes,"allowRules":access.allow_rules,"allowDeviceDiagnostics":access.allow_device_diagnostics,"endpoint":self.bridge.endpoint})
    }

    pub fn mcp_config(&self) -> String {
        serde_json::to_string_pretty(&json!({"mcpServers":{"scope":{"command":self.mcp_binary,"args":["--config",self.data_dir.join("mcp-bridge.json")]}}})).unwrap()
    }

    pub fn bootstrap(&self) -> Result<Value> {
        Ok(
            json!({"sessions":self.engine.list_sessions()?,"proxy":self.engine.proxy_status(),"dataDir":self.data_dir,"version":env!("CARGO_PKG_VERSION"),"deviceStatus":self.devices.status(),"mcpConfig":self.mcp_config(),"mcpStatus":self.mcp_status()}),
        )
    }

    pub fn set_mcp_scope(
        &self,
        ids: Vec<String>,
        allow_writes: bool,
        allow_rules: bool,
        allow_device_diagnostics: bool,
    ) -> Result<Value> {
        let existing = self.engine.list_sessions()?;
        if ids.len() > 8 || ids.iter().any(|id| !existing.iter().any(|s| s.id == *id)) {
            bail!("Select up to eight existing sessions");
        }
        self.engine.record_audit("mcp.permissions", None)?;
        *self.access.lock().unwrap() = McpAccess {
            session_ids: ids,
            allow_writes,
            allow_rules,
            allow_device_diagnostics,
        };
        Ok(self.mcp_status())
    }

    pub async fn ensure_demo(&self) -> Result<()> {
        let mut started = self.demo_started.lock().await;
        if *started {
            return Ok(());
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:17891")
            .await
            .context("Local demo port 17891 is busy")?;
        let router = Router::new().route("/{*path}", get(demo_response).post(demo_response));
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        *started = true;
        Ok(())
    }

    pub async fn generate_demo(&self, session_id: String) -> Result<Value> {
        let routes = [
            "profile", "products", "products", "sign-in", "slow", "config",
        ];
        let proxy = self.engine.proxy_status();
        if proxy.running && proxy.session_id.as_deref() != Some(&session_id) {
            bail!("Stop the current capture before recording demo traffic into another session");
        }
        if proxy.request_breakpoint || proxy.response_breakpoint {
            bail!("Disable breakpoints before generating demo traffic");
        }
        if routes.iter().any(|route| {
            self.engine
                .has_pause_rule_for("GET", &format!("http://127.0.0.1:17891/api/{route}"))
        }) {
            bail!("Disable manual pause rules matching the local demo before generating demo traffic; send a request from your own client to edit it interactively");
        }
        self.ensure_demo().await?;
        let port = if proxy.running {
            proxy.port
        } else {
            self.engine
                .start_proxy(session_id.clone(), 17890)
                .await?
                .port
        };
        let client = reqwest::Client::builder()
            .no_proxy()
            .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))?)
            .timeout(Duration::from_secs(15))
            .build()?;
        for route in routes {
            client
                .get(format!("http://127.0.0.1:17891/api/{route}"))
                .header("X-Scope-Demo", "true")
                .send()
                .await?
                .bytes()
                .await?;
        }
        Ok(json!({"captured":6,"sessionId":session_id,"target":"http://127.0.0.1:17891"}))
    }
}

async fn demo_response(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path();
    if path.contains("slow") {
        tokio::time::sleep(Duration::from_millis(380)).await;
    }
    let (status, body) = if path.contains("sign-in") || path.contains("login") {
        (
            StatusCode::UNAUTHORIZED,
            json!({"error":"demo_invalid_token","message":"This deliberate demo response shows a failed sign-in."}),
        )
    } else if path.contains("products") {
        (
            StatusCode::OK,
            json!({"products":[{"id":"sku_001","name":"Field notebook","price":24},{"id":"sku_002","name":"Mechanical pencil","price":12}],"currency":"EUR"}),
        )
    } else if path.contains("error") {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error":"demo_server_failure","message":"A deliberate local error for testing Scope."}),
        )
    } else if path.contains("orders") {
        (
            StatusCode::OK,
            json!({"orders":[{"id":"order_001","status":"processing","total":36}],"count":1}),
        )
    } else if path.contains("profile") {
        (
            StatusCode::OK,
            json!({"id":"demo_user","name":"Scope demo","plan":"open-source"}),
        )
    } else {
        (
            StatusCode::OK,
            json!({"demo":true,"path":path,"message":"Generated by Scope's local test server","features":{"capture":true,"persist":true}}),
        )
    };
    (
        status,
        [("X-Scope-Demo", "true"), ("Cache-Control", "no-store")],
        Json(body),
    )
}

#[derive(Deserialize)]
struct BridgeRequest {
    command: String,
    #[serde(default)]
    args: Value,
}

fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing {key}"))
}

async fn bridge_call(
    State(app): State<Arc<AppCore>>,
    headers: HeaderMap,
    Json(call): Json<BridgeRequest>,
) -> impl IntoResponse {
    if headers.get("authorization").and_then(|s| s.to_str().ok())
        != Some(&format!("Bearer {}", app.bridge.token))
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"Unauthorized"})),
        );
    }
    let result = dispatch_mcp(app, call).await;
    match result {
        Ok(v) => (StatusCode::OK, Json(json!({"result":v}))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":e.to_string()})),
        ),
    }
}

async fn dispatch_mcp(app: Arc<AppCore>, call: BridgeRequest) -> Result<Value> {
    let access = app.access.lock().unwrap().clone();
    let args = &call.args;
    if call.command == "list_sessions" {
        return Ok(redact(serde_json::to_value(
            app.engine
                .list_sessions()?
                .into_iter()
                .filter(|s| access.session_ids.contains(&s.id))
                .collect::<Vec<_>>(),
        )?));
    }
    if call.command == "status" {
        return Ok(json!({"connected":true,"access":access,"version":env!("CARGO_PKG_VERSION")}));
    }
    if matches!(
        call.command.as_str(),
        "list_rules" | "save_rule" | "delete_rule"
    ) {
        if !access.allow_rules {
            bail!("Global rule access is disabled. Enable it separately in Scope's MCP panel.");
        }
        let value = match call.command.as_str() {
            "list_rules" => serde_json::to_value(app.engine.list_rules()?)?,
            "save_rule" => {
                anyhow::ensure!(access.allow_writes, "MCP writes are disabled in Scope");
                let rule =
                    serde_json::from_value(args.get("rule").cloned().context("Missing rule")?)?;
                serde_json::to_value(app.engine.save_rule(rule)?)?
            }
            _ => {
                anyhow::ensure!(access.allow_writes, "MCP writes are disabled in Scope");
                app.engine.delete_rule(required(args, "id")?)?;
                json!({"ok":true})
            }
        };
        // Rules can contain replacement secrets and URL matchers. Never expose raw definitions.
        return Ok(redact_rule(value));
    }
    if call.command == "device_diagnostics" {
        anyhow::ensure!(
            access.allow_device_diagnostics,
            "Device diagnostics access is disabled in Scope's MCP panel"
        );
        app.engine.record_audit("mcp.diagnostics", None)?;
        return Ok(redact(serde_json::to_value(
            app.devices
                .diagnostics(
                    args.get("serial")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                )
                .await?,
        )?));
    }
    let session_id = if call.command == "get_request" || call.command == "replay_request" {
        app.engine.get_exchange(required(args, "id")?)?.session_id
    } else {
        required(args, "sessionId")?.to_string()
    };
    if !access.session_ids.contains(&session_id) {
        bail!("Session is not shared with MCP. Select it in Scope's MCP panel first.");
    }
    let value = match call.command.as_str() {
        "search_requests" => {
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(50)
                .min(200) as usize;
            let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
            let needle = args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            let mut rows = Vec::new();
            let mut skipped = 0;
            if limit > 0 {
                app.engine
                    .visit_exchanges(&session_id, 10_000, |exchange| {
                        // Match only the representation the caller may actually read;
                        // searching raw secret text would create a membership oracle.
                        let mut row = redact_exchange(serde_json::to_value(exchange)?);
                        let matches = needle.is_empty()
                            || serde_json::to_string(&row)?
                                .to_lowercase()
                                .contains(&needle);
                        if matches {
                            if skipped < offset {
                                skipped += 1;
                            } else {
                                if let Some(object) = row.as_object_mut() {
                                    object.remove("requestBody");
                                    object.remove("responseBody");
                                }
                                rows.push(row);
                            }
                        }
                        Ok(rows.len() < limit)
                    })?;
            }
            Value::Array(rows)
        }
        "get_request" => serde_json::to_value(app.engine.get_exchange(required(args, "id")?)?)?,
        "analyze_session" => app.engine.analysis(&session_id)?,
        "infer_openapi" => app.engine.infer_openapi(&session_id)?,
        "compare_sessions" => {
            let right = required(args, "rightSessionId")?;
            if !access.session_ids.contains(&right.to_string()) {
                bail!("Both sessions must be shared in Scope");
            }
            app.engine.compare_sessions(&session_id, right)?
        }
        "replay_request" => {
            if !access.allow_writes {
                bail!("MCP replay is disabled. Enable write access in Scope first.");
            }
            let edit = args
                .get("edit")
                .cloned()
                .filter(|v| !v.is_null())
                .map(serde_json::from_value)
                .transpose()?;
            serde_json::to_value(app.engine.replay(required(args, "id")?, edit).await?)?
        }
        _ => bail!("Unknown MCP command"),
    };
    Ok(match call.command.as_str() {
        "get_request" | "replay_request" => redact_exchange(value),
        // Each row was already sanitized before matching and pagination.
        "search_requests" => value,
        // Engine generates this typed document without payload values. Recursing
        // through schema property names would corrupt credential-named schemas.
        "infer_openapi" => value,
        _ => redact(value),
    })
}

fn sensitive(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace(['-', '_'], "");
    [
        "authorization",
        "proxyauthorization",
        "cookie",
        "setcookie",
        "password",
        "passwd",
        "secret",
        "token",
        "accesstoken",
        "refreshtoken",
        "apikey",
        "xapikey",
        "xauth",
        "xaccesskey",
        "xsessionid",
        "sessionid",
        "accesskey",
        "privatekey",
        "signingkey",
    ]
    .iter()
    .any(|k| key == *k || key.ends_with("token") || key.ends_with("secret"))
}

fn sanitized_url(raw: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(raw) else {
        return "[URL omitted]".into();
    };
    let pairs: Vec<_> = url
        .query_pairs()
        .map(|(k, v)| {
            let value = if sensitive(&k) {
                "[redacted]".into()
            } else {
                v.into_owned()
            };
            (k.into_owned(), value)
        })
        .collect();
    if url.query().is_some() {
        url.query_pairs_mut().clear().extend_pairs(pairs);
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_fragment(None);
    url.to_string()
}

fn sanitized_body(raw: &str, encoded: bool, limit: Option<usize>) -> String {
    if encoded {
        return "[body omitted: binary/base64 content]".into();
    }
    if limit.is_some_and(|max| raw.len() > max) {
        return "[body omitted: exceeds 64 KiB; inspect in Scope]".into();
    }
    if raw.is_empty() {
        return String::new();
    }
    match serde_json::from_str::<Value>(raw) {
        Ok(value) => redact(value).to_string(),
        Err(_) => "[body omitted: non-JSON content]".into(),
    }
}

// Rule definitions are global configuration, independent of session sharing.
// Names/matchers can also contain secrets, so reads return an inventory rather
// than redacted definitions that an agent might accidentally write back.
fn redact_rule(value: Value) -> Value {
    match value {
        Value::Array(rows) => Value::Array(rows.into_iter().map(redact_rule).collect()),
        Value::Object(map) if map.contains_key("matcher") => json!({
            "id":map.get("id"), "enabled":map.get("enabled"), "priority":map.get("priority"),
            "actionType":map.get("action").and_then(|a| a.get("type")),
            "definitionOmitted":true,
        }),
        other => other,
    }
}

/// A lossy copy for sharing, never the default export. Known credentials are
/// masked and non-JSON/binary bodies are removed. This is not a secret scanner.
pub fn sanitize_archive(mut value: Value) -> Value {
    let native_session_ids: Vec<_> = value
        .get("exchanges")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    row.get("sessionId")
                        .and_then(Value::as_str)
                        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(entries) = value
        .pointer_mut("/log/entries")
        .and_then(Value::as_array_mut)
    {
        for entry in entries {
            for (path, encoded_key) in [
                ("/request/postData", "_encoding"),
                ("/response/content", "encoding"),
            ] {
                if let Some(body) = entry.pointer_mut(path).and_then(Value::as_object_mut) {
                    let text = body.get("text").and_then(Value::as_str).unwrap_or("");
                    let clean = sanitized_body(
                        text,
                        body.get(encoded_key).and_then(Value::as_str) == Some("base64"),
                        None,
                    );
                    body.insert("text".into(), json!(clean));
                    body.remove(encoded_key);
                    if body.contains_key("size") {
                        body.insert("size".into(), json!(clean.len()));
                    }
                }
            }
            for side in ["request", "response"] {
                if let Some(map) = entry.get_mut(side).and_then(Value::as_object_mut) {
                    map.insert("cookies".into(), json!([]));
                    map.insert("bodySize".into(), json!(-1));
                }
            }
        }
    }
    if let Some(rows) = value.get_mut("exchanges").and_then(Value::as_array_mut) {
        for row in rows {
            if let Some(map) = row.as_object_mut() {
                for (body, encoded, bytes) in [
                    ("requestBody", "requestBodyBase64", "requestBytes"),
                    ("responseBody", "responseBodyBase64", "responseBytes"),
                ] {
                    let clean = sanitized_body(
                        map.get(body).and_then(Value::as_str).unwrap_or(""),
                        map.get(encoded).and_then(Value::as_bool).unwrap_or(false),
                        None,
                    );
                    map.insert(bytes.into(), json!(clean.len()));
                    map.insert(body.into(), json!(clean));
                    map.insert(encoded.into(), json!(false));
                }
            }
        }
    }
    // Shared archives omit arbitrary free text in session metadata.
    if let Some(session) = value.get_mut("session").and_then(Value::as_object_mut) {
        session.insert("name".into(), json!("Sanitized capture"));
        session.insert("notes".into(), json!(""));
        session.insert("tags".into(), json!([]));
        session.insert("source".into(), json!("sanitized-export"));
    }
    if let Some(pages) = value
        .pointer_mut("/log/pages")
        .and_then(Value::as_array_mut)
    {
        for page in pages {
            page["title"] = json!("Sanitized capture");
        }
    }
    value = redact(value);
    if let Some(rows) = value.get_mut("exchanges").and_then(Value::as_array_mut) {
        for (row, id) in rows.iter_mut().zip(native_session_ids) {
            if let Some(id) = id {
                row["sessionId"] = json!(id);
            }
        }
    }
    value["_scopeSharing"] = json!({"sanitized":true,"lossy":true,"warning":"Known credential fields are masked. Binary/non-JSON bodies and session notes are omitted. Review URLs, identifiers and application-specific fields before sharing."});
    value
}

// Typed exchange metadata is supplied by Engine, never inferred from body keys.
// The recursive payload redactor intentionally continues masking sessionId.
fn redact_exchange(value: Value) -> Value {
    let session_id = value
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
        .map(str::to_owned);
    let mut value = redact(value);
    if let Some(id) = session_id {
        value["sessionId"] = json!(id);
    }
    value
}

pub fn redact(mut value: Value) -> Value {
    match &mut value {
        Value::Array(values) => {
            for v in values {
                *v = redact(v.take());
            }
        }
        Value::Object(map) => {
            // Original snapshots remain available in the desktop. Do not expose a
            // second, differently-shaped body path that could bypass redaction.
            map.remove("originalRequest");
            map.remove("originalResponse");
            // Rule names are arbitrary global configuration and can contain secrets.
            // Preserve shape so a sanitized native archive remains importable.
            if let Some(rules) = map.get_mut("appliedRules").and_then(Value::as_array_mut) {
                for rule in rules {
                    if let Some(rule) = rule.as_object_mut() {
                        rule.insert("name".into(), json!("[rule name omitted]"));
                    }
                }
            }
            // Normalize body encoding flags together with the replacement text.
            for (body, encoding) in [
                ("requestBody", "requestBodyBase64"),
                ("responseBody", "responseBodyBase64"),
            ] {
                if let Some(raw) = map.get(body).and_then(Value::as_str) {
                    let clean = sanitized_body(
                        raw,
                        map.get(encoding).and_then(Value::as_bool).unwrap_or(false),
                        Some(65536),
                    );
                    map.insert(body.into(), json!(clean));
                    map.insert(encoding.into(), json!(false));
                }
            }
            if map
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(sensitive)
                && map.contains_key("value")
            {
                map.insert("value".into(), json!("[redacted]"));
            }
            if map.get("name").and_then(Value::as_str).is_some_and(|s| {
                s.eq_ignore_ascii_case("location") || s.eq_ignore_ascii_case("referer")
            }) {
                if let Some(raw) = map.get("value").and_then(Value::as_str) {
                    map.insert("value".into(), json!(sanitized_url(raw)));
                }
            }
            for (key, v) in map.iter_mut() {
                if sensitive(key) {
                    *v = json!("[redacted]");
                } else if key == "requestBody" || key == "responseBody" {
                    // Already sanitized above with the original encoding flag.
                } else if key == "error" && v.is_string() {
                    *v = json!("[error detail omitted; inspect in Scope]");
                } else if matches!(key.as_str(), "url" | "redirectURL") {
                    if let Some(s) = v.as_str() {
                        if !s.is_empty() {
                            *v = json!(sanitized_url(s));
                        }
                    }
                } else {
                    *v = redact(v.take());
                }
            }
        }
        _ => {}
    }
    // Return bounded individual bodies to agents, with an explicit omission marker.
    if let Value::Object(ref mut map) = value {
        for key in ["requestBody", "responseBody"] {
            if map
                .get(key)
                .and_then(Value::as_str)
                .is_some_and(|s| s.len() > 65536)
            {
                map.insert(
                    key.into(),
                    json!("[body omitted from MCP: exceeds 64 KiB; inspect in Scope]"),
                );
            }
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo_rejects_matching_manual_pause_before_starting_capture() {
        let dir = std::env::temp_dir().join(format!("scope-demo-pause-{}", uuid::Uuid::new_v4()));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let app = AppCore::open(dir.clone(), None, 0).await.unwrap();
            let session = app
                .engine
                .create_session("fixture".into(), "test".into())
                .unwrap();
            app.engine
                .save_rule(
                    serde_json::from_value(json!({
                        "id":"", "name":"Pause demo products", "enabled":true, "priority":10,
                        "matcher":{"host":"127.0.0.1", "pathPrefix":"/api/products"},
                        "action":{"type":"pause", "stage":"response"}
                    }))
                    .unwrap(),
                )
                .unwrap();
            let error = app.generate_demo(session.id.clone()).await.unwrap_err();
            assert!(error
                .to_string()
                .contains("manual pause rules matching the local demo"));
            assert!(!app.engine.proxy_status().running);
            assert!(app.engine.pending().is_empty());
            assert!(app
                .engine
                .list_exchanges(&session.id, None, 10, 0)
                .unwrap()
                .is_empty());
        });
        drop(runtime);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn redacts_credentials_and_keeps_evidence() {
        let v = redact(
            json!({"url":"https://example.com/path?token=private&q=ok","requestHeaders":[{"name":"Authorization","value":"Bearer private"}],"responseBody":"{\"access_token\":\"private\",\"count\":3}","originalRequest":{"body":"{\"token\":\"private\"}"},"originalResponse":{"body":"private"}}),
        );
        assert!(!v.to_string().contains("private"));
        assert!(v["responseBody"].as_str().unwrap().contains("3"));
        assert!(v["url"].as_str().unwrap().contains("q=ok"));
        assert!(v.get("originalRequest").is_none());
    }

    #[test]
    fn sanitized_archives_cover_har_and_native_body_paths() {
        let native = sanitize_archive(json!({"format":"scope-session","version":1,
            "session":{"name":"private","notes":"private","tags":["private"]},
            "exchanges":[{"url":"https://user:private@example.test/?api_key=private",
                "requestHeaders":[{"name":"Authorization","value":"private"}],
                "requestBody":"{\"password\":\"private\",\"count\":3}","requestBodyBase64":false,
                "responseBody":"cHJpdmF0ZQ==","responseBodyBase64":true,
                "error":"failed URL private","originalRequest":{"body":"private"}}]}));
        assert!(!native.to_string().contains("private"));
        assert!(!native.to_string().contains("cHJpdmF0ZQ=="));
        assert_eq!(native["exchanges"][0]["responseBodyBase64"], false);
        assert_eq!(native["_scopeSharing"]["lossy"], true);
        let har = sanitize_archive(json!({"log":{"pages":[{"title":"private"}],"entries":[{
            "request":{"url":"https://example.test/?token=private","cookies":[{"value":"private"}],
                "postData":{"text":"{\"token\":\"private\"}"},"queryString":[{"name":"token","value":"private"}]},
            "response":{"content":{"text":"private","encoding":"base64","size":7},
                "headers":[{"name":"Location","value":"https://example.test/?token=private"}]},
            "_scope":{"originalResponse":{"body":"private"}}}]}}));
        assert!(!har.to_string().contains("private"));
        assert!(har
            .pointer("/log/entries/0/response/content/encoding")
            .is_none());
    }

    #[test]
    fn redaction_hides_global_rule_names_and_session_credentials() {
        let value = redact(
            json!({"appliedRules":[{"ruleId":"opaque","name":"secret-global-rule-name","action":"mockResponse","stage":"request"}],"requestHeaders":[{"name":"X-Session-Id","value":"secret-session"},{"name":"X-Access-Key","value":"secret-access"}],"requestBody":"{\"session_id\":\"secret-json-session\",\"private_key\":\"secret-private-key\",\"public\":1}"}),
        );
        assert!(!value.to_string().contains("secret"));
        assert_eq!(value["appliedRules"][0]["name"], "[rule name omitted]");
        assert!(value["requestBody"].as_str().unwrap().contains("public"));
    }

    #[test]
    fn opaque_scope_session_metadata_survives_without_whitelisting_payload_credentials() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let exchange = json!({"sessionId":session_id,"requestBody":"{\"sessionId\":\"body-session-secret\",\"nested\":{\"session_id\":\"nested-session-secret\"}}","responseBody":"{\"sessionId\":\"response-session-secret\"}"});
        let clean = redact_exchange(exchange.clone());
        assert_eq!(clean["sessionId"], session_id);
        assert!(!clean.to_string().contains("session-secret"));
        let raw_payload = redact(json!({"sessionId":session_id}));
        assert_eq!(raw_payload["sessionId"], "[redacted]");
        let archive = sanitize_archive(json!({"format":"scope-session","exchanges":[exchange]}));
        assert_eq!(archive["exchanges"][0]["sessionId"], session_id);
        assert!(!archive.to_string().contains("session-secret"));
    }

    #[test]
    fn mcp_scope_permissions_and_authenticated_bridge() {
        let dir = std::env::temp_dir().join(format!("scope-bridge-{}", uuid::Uuid::new_v4()));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
        let app = AppCore::open(dir.clone(), None, 0).await.unwrap();
        let session = app.engine.create_session("fixture".into(), "test".into()).unwrap();
        let call = |command: &str, args: Value| BridgeRequest { command: command.into(), args };
        assert!(dispatch_mcp(app.clone(), call("analyze_session", json!({"sessionId":session.id}))).await.is_err());
        assert!(dispatch_mcp(app.clone(), call("list_rules", json!({}))).await.is_err());
        assert!(dispatch_mcp(app.clone(), call("device_diagnostics", json!({}))).await.is_err());
        app.set_mcp_scope(vec![session.id.clone()], false, true, false).unwrap();
        assert!(dispatch_mcp(app.clone(), call("analyze_session", json!({"sessionId":session.id}))).await.is_ok());
        assert!(dispatch_mcp(app.clone(), call("infer_openapi", json!({"sessionId":session.id}))).await.is_ok());
        let definition = json!({"id":"","name":"secret-name","enabled":true,"priority":0,"matcher":{"host":"example.test"},"action":{"type":"mockResponse","status":200,"body":"secret-value"}});
        assert!(dispatch_mcp(app.clone(), call("save_rule", json!({"rule":definition}))).await.is_err());
        app.set_mcp_scope(vec![session.id.clone()], true, true, false).unwrap();
        let created = dispatch_mcp(app.clone(), call("save_rule", json!({"rule":definition}))).await.unwrap();
        assert!(created["id"].is_string());
        assert!(!created.to_string().contains("secret"));
        let proxy = app.engine.start_proxy(session.id.clone(), 0).await.unwrap();
        // The mock never contacts example.test; exercise verified TLS through this
        // temporary engine's CA instead of transmitting fixture credentials over HTTP.
        let capturing = reqwest::Client::builder().no_proxy().proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{}",proxy.port)).unwrap())
            .add_root_certificate(reqwest::Certificate::from_pem(&std::fs::read(app.engine.ca_cert_path()).unwrap()).unwrap())
            .build().unwrap();
        let response = capturing.post("https://example.test/visible?api_key=hidden-query-credential").header("authorization","Bearer hidden-header-credential").header("content-type","application/json").body("{\"token\":\"hidden-body-credential\",\"public\":\"visible-body-evidence\"}").send().await.unwrap();
        assert_eq!(response.status(),200);let _=response.bytes().await.unwrap();app.engine.stop_proxy().await.unwrap();
        for query in ["hidden-query-credential","hidden-header-credential","hidden-body-credential","secret-name","secret-value"] {
            let results=dispatch_mcp(app.clone(),call("search_requests",json!({"sessionId":session.id,"query":query}))).await.unwrap();assert_eq!(results,json!([]),"Raw credential was searchable: {query}");
        }
        let results=dispatch_mcp(app.clone(),call("search_requests",json!({"sessionId":session.id,"query":"visible-body-evidence"}))).await.unwrap();assert_eq!(results.as_array().unwrap().len(),1);assert_eq!(results[0]["sessionId"],session.id);assert!(results[0].get("requestBody").is_none());
        let exchange_id=results[0]["id"].as_str().unwrap();
        let full=dispatch_mcp(app.clone(),call("get_request",json!({"id":exchange_id}))).await.unwrap();assert_eq!(full["sessionId"],session.id);
        let replay=dispatch_mcp(app.clone(),call("replay_request",json!({"id":exchange_id}))).await.unwrap();assert_eq!(replay["sessionId"],session.id);let schema=dispatch_mcp(app.clone(),call("infer_openapi",json!({"sessionId":session.id}))).await.unwrap();assert_eq!(schema["x-scope"]["sessionId"],session.id);assert_eq!(schema["paths"]["/visible"]["post"]["requestBody"]["content"]["application/json"]["schema"]["properties"]["token"]["type"],"string");assert!(!results.to_string().contains("hidden-"));assert!(!results.to_string().contains("secret-name"));
        let listed = dispatch_mcp(app.clone(), call("list_rules", json!({}))).await.unwrap();
        assert!(!listed.to_string().contains("secret"));
        assert!(!listed.to_string().contains("example.test"));
        let id = created["id"].as_str().unwrap();
        dispatch_mcp(app.clone(), call("delete_rule", json!({"id":id}))).await.unwrap();
        app.set_mcp_scope(vec![], false, false, false).unwrap();
        assert!(dispatch_mcp(app.clone(), call("analyze_session", json!({"sessionId":session.id}))).await.is_err());
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let response = client.post(&app.bridge.endpoint).json(&json!({"command":"status"})).send().await.unwrap();
        assert_eq!(response.status(), 401);
        let response = client.post(&app.bridge.endpoint).bearer_auth(&app.bridge.token).json(&json!({"command":"status"})).send().await.unwrap();
        assert_eq!(response.status(), 200);
        let audit = app.engine.list_audit(100).unwrap();
        assert!(!serde_json::to_string(&audit).unwrap().contains("secret"));
        });
        // Stopping the runtime drops the bridge router and its retained AppCore,
        // releasing SQLite handles before Windows attempts directory removal.
        drop(runtime);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
