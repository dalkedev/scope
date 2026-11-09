use super::*;
use http_body_util::BodyExt;
use hudsucker::{
    certificate_authority::RcgenAuthority,
    hyper::{
        header::{HeaderName, HeaderValue, CONTENT_LENGTH, HOST},
        HeaderMap, Method, Request, Response, StatusCode,
    },
    rcgen::{Issuer, KeyPair},
    rustls::{self, crypto::aws_lc_rs},
    Body, HttpContext, HttpHandler, Proxy, RequestOrResponse,
};
use std::{
    io::BufReader,
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::{sync::oneshot, task::JoinHandle};

pub(crate) struct Running {
    pub port: u16,
    pub session: String,
    pub stop: Option<oneshot::Sender<()>>,
    pub task: JoinHandle<()>,
}
#[derive(Clone)]
struct Capture {
    engine: Engine,
    session: String,
    exchange: Option<Exchange>,
    start: Instant,
    matched_rules: Vec<Arc<TrafficRule>>,
}

/// Registers a replay independently of the proxy listener. Keep registration
/// until the final archive write, including future cancellation and early errors.
struct ReplayGuard {
    engine: Engine,
    exchange_id: String,
}
impl Drop for ReplayGuard {
    fn drop(&mut self) {
        // Same order as registration/deletion: replay ownership before the DB.
        let mut replays = self.engine.inner.active_replays.lock().unwrap();
        let db = self.engine.inner.db.lock().unwrap();
        if let Err(error) = db.execute(
            "UPDATE exchanges SET data=json_set(data,'$.completed',json('true'),'$.error','Replay interrupted before completion','$.durationMs',?2-json_extract(data,'$.startedAt')) WHERE id=?1 AND json_extract(data,'$.completed')=0",
            params![self.exchange_id, now()],
        ) {
            eprintln!("Cannot finalize interrupted replay: {error}");
        }
        replays.remove(&self.exchange_id);
    }
}

pub(crate) fn header_map(headers: &[Header]) -> Result<HeaderMap> {
    let mut map = HeaderMap::new();
    for h in headers {
        map.append(
            HeaderName::from_bytes(h.name.as_bytes()).context("Invalid header name")?,
            HeaderValue::from_bytes(h.value.as_bytes()).context("Invalid header value")?,
        );
    }
    Ok(map)
}
fn headers(map: &HeaderMap) -> Vec<Header> {
    map.iter()
        .map(|(k, v)| Header {
            name: k.to_string(),
            value: String::from_utf8_lossy(v.as_bytes()).into_owned(),
        })
        .collect()
}
fn normalize_headers(map: &mut HeaderMap, len: usize, body_allowed: bool) {
    let named: Vec<String> = map
        .get_all("connection")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(',').map(|x| x.trim().to_owned()))
        .collect();
    for name in named {
        map.remove(name.as_str());
    }
    for name in [
        "connection",
        "proxy-connection",
        "proxy-authenticate",
        "proxy-authorization",
        "keep-alive",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ] {
        map.remove(name);
    }
    map.remove(CONTENT_LENGTH);
    if body_allowed {
        map.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&len.to_string()).unwrap(),
        );
    }
}
async fn collect(mut body: Body) -> Result<Vec<u8>> {
    tokio::time::timeout(Duration::from_secs(120), async move {
        let mut bytes = Vec::new();
        while let Some(frame) = body.frame().await {
            let frame = frame.context("Body stream failed")?;
            if let Ok(data) = frame.into_data() {
                if bytes.len().saturating_add(data.len()) > CAPTURE_LIMIT_BYTES {
                    bail!(
                        "Body exceeds 16 MiB capture limit; transfer rejected and body not stored"
                    );
                }
                bytes.extend_from_slice(&data);
            }
        }
        Ok(bytes)
    })
    .await
    .context("Body capture timed out after 120 seconds")?
}
fn error_response(status: StatusCode, message: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .body(Body::from(format!("Scope: {message}")))
        .unwrap()
}
fn request_snapshot(e: &Exchange) -> Value {
    serde_json::json!({"method":e.method,"url":e.url,"headers":e.request_headers,"body":e.request_body,"bodyBase64":e.request_body_base64})
}
fn response_snapshot(e: &Exchange) -> Value {
    serde_json::json!({"status":e.status,"headers":e.response_headers,"body":e.response_body,"bodyBase64":e.response_body_base64})
}
fn has_request_edit(e: &EditMessage) -> bool {
    e.method.is_some() || e.url.is_some() || e.headers.is_some() || e.body.is_some()
}
fn has_response_edit(e: &EditMessage) -> bool {
    e.status.is_some() || e.headers.is_some() || e.body.is_some()
}

impl Engine {
    pub fn proxy_status(&self) -> ProxyStatus {
        let active = self.inner.running.lock().unwrap();
        let running = active.as_ref().filter(|p| !p.task.is_finished());
        ProxyStatus {
            running: running.is_some(),
            port: running.map(|p| p.port).unwrap_or(0),
            session_id: running.map(|p| p.session.clone()),
            request_breakpoint: self.inner.request_breakpoint.load(Ordering::SeqCst),
            response_breakpoint: self.inner.response_breakpoint.load(Ordering::SeqCst),
            ca_cert_path: self.ca_cert_path().to_string_lossy().into_owned(),
        }
    }
    pub async fn start_proxy(&self, session_id: String, port: u16) -> Result<ProxyStatus> {
        let _lifecycle = self.inner.lifecycle.lock().await;
        let mut running = self.inner.running.lock().unwrap();
        self.session(&session_id)?;
        if running.as_ref().is_some_and(|p| !p.task.is_finished()) {
            bail!("Proxy already running; stop it before changing the session or port");
        }
        let listener = std::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))
            .context("Cannot bind localhost proxy")?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let listener = tokio::net::TcpListener::from_std(listener)?;
        let key = KeyPair::from_pem(&fs::read_to_string(
            self.inner.dir.join("scope-ca-key.pem"),
        )?)?;
        let cert = fs::read_to_string(self.ca_cert_path())?;
        let issuer = Issuer::from_ca_cert_pem(&cert, key)?;
        let ca = RcgenAuthority::new(issuer, 256, aws_lc_rs::default_provider());
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        // Trust this installation's CA for controlled local TLS fixtures. Public servers
        // still undergo normal certificate and hostname validation.
        for cert in rustls_pemfile::certs(&mut BufReader::new(cert.as_bytes())) {
            roots.add(cert?)?;
        }
        let config =
            rustls::ClientConfig::builder_with_provider(Arc::new(aws_lc_rs::default_provider()))
                .with_safe_default_protocol_versions()?
                .with_root_certificates(roots)
                .with_no_client_auth();
        let connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(config)
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build();
        let (tx, rx) = oneshot::channel();
        let proxy = Proxy::builder()
            .with_listener(listener)
            .with_ca(ca)
            .with_http_connector(connector)
            .with_http_handler(Capture {
                engine: self.clone(),
                session: session_id.clone(),
                exchange: None,
                start: Instant::now(),
                matched_rules: vec![],
            })
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .build()?;
        let engine = self.clone();
        let session = session_id.clone();
        let task = tokio::spawn(async move {
            if let Err(error) = proxy.start().await {
                eprintln!("Scope proxy failed: {error}");
                engine.mark_incomplete(
                    &session,
                    &format!("Proxy stopped after transport failure: {error}"),
                );
            }
        });
        *running = Some(Running {
            port,
            session: session_id,
            stop: Some(tx),
            task,
        });
        drop(running);
        Ok(self.proxy_status())
    }
    pub async fn stop_proxy(&self) -> Result<()> {
        let _lifecycle = self.inner.lifecycle.lock().await;
        let active = self.inner.running.lock().unwrap().take();
        let pending = std::mem::take(&mut *self.inner.pending.lock().unwrap());
        for (_, (_, tx)) in pending {
            let _ = tx.send(EditMessage::abort());
        }
        if let Some(mut active) = active {
            if let Some(tx) = active.stop.take() {
                let _ = tx.send(());
            }
            if tokio::time::timeout(Duration::from_secs(3), &mut active.task)
                .await
                .is_err()
            {
                active.task.abort();
                let _ = active.task.await;
            }
            self.mark_incomplete(&active.session, "Proxy stopped before capture completed");
        }
        Ok(())
    }
    fn mark_incomplete(&self, session: &str, reason: &str) {
        let replays = self.inner.active_replays.lock().unwrap();
        if let Ok(exchanges) = self.all_exchanges(session) {
            for mut e in exchanges {
                if !e.completed && !replays.contains_key(&e.id) {
                    e.error = Some(reason.into());
                    e.completed = true;
                    e.duration_ms = now() - e.started_at;
                    let _ = self.save(&e);
                }
            }
        }
    }
    async fn pause(&self, e: &Exchange, stage: &str) -> Result<EditMessage> {
        let (tx, rx) = oneshot::channel();
        let id = uid();
        let request = stage == "request";
        let p = PendingExchange {
            id: id.clone(),
            exchange_id: e.id.clone(),
            stage: stage.into(),
            method: e.method.clone(),
            url: e.url.clone(),
            status: e.status,
            headers: if request {
                e.request_headers.clone()
            } else {
                e.response_headers.clone()
            },
            body: if request {
                e.request_body.clone()
            } else {
                e.response_body.clone()
            },
            body_base64: if request {
                e.request_body_base64
            } else {
                e.response_body_base64
            },
            created_at: now(),
        };
        self.inner
            .pending
            .lock()
            .unwrap()
            .insert(id.clone(), (p, tx));
        let result =
            tokio::time::timeout(Duration::from_secs(BREAKPOINT_TIMEOUT_SECONDS), rx).await;
        self.inner.pending.lock().unwrap().remove(&id);
        match result { Ok(Ok(edit)) if edit.action=="continue"=>Ok(edit),Ok(_)=>bail!("{stage} aborted at breakpoint"),Err(_)=>bail!("{stage} breakpoint expired after {BREAKPOINT_TIMEOUT_SECONDS} seconds; transfer aborted") }
    }
    pub async fn replay(&self, id: &str, edit: Option<EditMessage>) -> Result<Exchange> {
        // Register while holding the same ownership lock used by deletion. A
        // deletion either wins before this lookup, or observes an active replay.
        let (old, mut e, _replay) = {
            let mut replays = self.inner.active_replays.lock().unwrap();
            let old = self.get_exchange(id)?;
            let e = self.fresh_exchange(&old.session_id, old.method.clone(), old.url.clone());
            replays.insert(e.id.clone(), e.session_id.clone());
            let guard = ReplayGuard {
                engine: self.clone(),
                exchange_id: e.id.clone(),
            };
            (old, e, guard)
        };
        e.request_headers = old.request_headers.clone();
        e.request_body = old.request_body.clone();
        e.request_body_base64 = old.request_body_base64;
        e.request_bytes = old.request_bytes;
        if let Some(edit) = edit {
            edit.validate()?;
            if edit.action == "abort" {
                bail!("Replay aborted");
            }
            apply_request_edit(&mut e, &edit)?;
        }
        let rules = self.matching_rules(&e.method, &e.url);
        let terminal = apply_request_rules(&mut e, &rules)?;
        let bytes = decode_body(&e.request_body, e.request_body_base64)?;
        let mut map = header_map(&e.request_headers)?;
        normalize_headers(&mut map, bytes.len(), true);
        map.remove(HOST);
        e.request_headers = headers(&map);
        e.request_bytes = bytes.len() as i64;
        self.save(&e)?;
        self.record_audit("request.replay", Some(&e.id))?;
        let result = if let Some(edit) = terminal {
            e.status = edit.status;
            e.response_headers = edit.headers.unwrap_or_default();
            e.response_body = edit.body.unwrap_or_default();
            e.response_body_base64 = edit.body_base64.unwrap_or(false);
            Ok(())
        } else {
            self.send_replay(&mut e).await
        };
        let result = result
            .and_then(|_| apply_response_rules(&mut e, &rules))
            .and_then(|_| normalize_exchange_response(&mut e));
        e.completed = true;
        e.duration_ms = now() - e.started_at;
        e.error = result.err().map(|err| format!("Replay failed: {err:#}"));
        self.save(&e)?;
        Ok(e)
    }
    async fn send_replay(&self, e: &mut Exchange) -> Result<()> {
        validate_url(&e.url)?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(120))
            .add_root_certificate(reqwest::Certificate::from_pem(&fs::read(
                self.ca_cert_path(),
            )?)?)
            .build()?;
        let body = decode_body(&e.request_body, e.request_body_base64)?;
        if body.len() > CAPTURE_LIMIT_BYTES {
            bail!("Replay body exceeds 16 MiB capture limit");
        }
        let mut map = header_map(&e.request_headers)?;
        normalize_headers(&mut map, body.len(), true);
        map.remove(HOST);
        e.request_headers = headers(&map);
        e.request_bytes = body.len() as i64;
        let mut response = client
            .request(e.method.parse()?, &e.url)
            .headers(map)
            .body(body)
            .send()
            .await?;
        e.status = Some(response.status().as_u16());
        e.response_headers = headers(response.headers());
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len().saturating_add(chunk.len()) > CAPTURE_LIMIT_BYTES {
                bail!("Replay response exceeds 16 MiB capture limit; body not stored");
            }
            bytes.extend_from_slice(&chunk);
        }
        e.response_bytes = bytes.len() as i64;
        (e.response_body, e.response_body_base64) = encode_body(&bytes);
        Ok(())
    }
}
fn apply_request_edit(e: &mut Exchange, edit: &EditMessage) -> Result<()> {
    if !has_request_edit(edit) {
        return Ok(());
    }
    if e.original_request.is_none() {
        e.original_request = Some(request_snapshot(e));
    }
    e.edited = true;
    if let Some(method) = &edit.method {
        e.method = method.clone();
    }
    if let Some(url) = &edit.url {
        e.url = url.clone();
    }
    if let Some(headers) = &edit.headers {
        e.request_headers = headers.clone();
    }
    if let Some(body) = &edit.body {
        e.request_body = body.clone();
        e.request_body_base64 = edit.body_base64.unwrap_or(false);
        remove_representation_headers(&mut e.request_headers);
    }
    e.request_bytes = decode_body(&e.request_body, e.request_body_base64)?.len() as i64;
    Ok(())
}
fn remove_representation_headers(headers: &mut Vec<Header>) {
    headers.retain(|h| {
        !["content-encoding", "content-md5", "digest", "etag"]
            .iter()
            .any(|name| h.name.eq_ignore_ascii_case(name))
    });
}
fn apply_response_edit(e: &mut Exchange, edit: &EditMessage) -> Result<()> {
    if !has_response_edit(edit) {
        return Ok(());
    }
    if e.original_response.is_none() {
        e.original_response = Some(response_snapshot(e));
    }
    e.edited = true;
    if let Some(status) = edit.status {
        e.status = Some(status);
    }
    if let Some(headers) = &edit.headers {
        e.response_headers = headers.clone();
    }
    if let Some(body) = &edit.body {
        e.response_body = body.clone();
        e.response_body_base64 = edit.body_base64.unwrap_or(false);
        remove_representation_headers(&mut e.response_headers);
    }
    e.response_bytes = decode_body(&e.response_body, e.response_body_base64)?.len() as i64;
    Ok(())
}
fn apply_request_rules(
    e: &mut Exchange,
    rules: &[Arc<TrafficRule>],
) -> Result<Option<EditMessage>> {
    for rule in rules {
        match &rule.action {
            RuleAction::RewriteRequest { .. } => {
                apply_request_edit(e, &rule.action.edit())?;
                e.applied_rules.push(rule.evidence("request"));
            }
            RuleAction::MockResponse { .. } | RuleAction::Block { .. } => {
                e.provenance = if matches!(rule.action, RuleAction::Block { .. }) {
                    TrafficProvenance::Blocked
                } else {
                    TrafficProvenance::Mock
                };
                e.applied_rules.push(rule.evidence("request"));
                return Ok(Some(rule.action.edit()));
            }
            _ => {}
        }
    }
    Ok(None)
}
fn apply_response_rules(e: &mut Exchange, rules: &[Arc<TrafficRule>]) -> Result<()> {
    for rule in rules {
        if matches!(rule.action, RuleAction::RewriteResponse { .. }) {
            apply_response_edit(e, &rule.action.edit())?;
            e.applied_rules.push(rule.evidence("response"));
        }
    }
    Ok(())
}
/// Only live captures call this: replays apply automatic actions without
/// claiming that a manual pause happened. Keep the original matched snapshot,
/// including pause rules after a terminal mock/block action in rule order.
fn record_pause_rules(e: &mut Exchange, rules: &[Arc<TrafficRule>], stage: PauseStage) -> bool {
    let mut pause = false;
    for rule in rules {
        if matches!(rule.action, RuleAction::Pause { stage: configured } if configured.includes(stage))
        {
            e.applied_rules.push(rule.evidence(match stage {
                PauseStage::Request => "request",
                PauseStage::Response => "response",
                PauseStage::Both => unreachable!("A capture pauses at one stage at a time"),
            }));
            pause = true;
        }
    }
    pause
}
fn normalize_exchange_response(e: &mut Exchange) -> Result<()> {
    let mut bytes = decode_body(&e.response_body, e.response_body_base64)?;
    let status = e.status.unwrap_or(502);
    let allowed = e.method != "HEAD" && !matches!(status, 100..=199 | 204 | 304);
    if !allowed {
        bytes.clear();
        e.response_body.clear();
        e.response_body_base64 = false;
    }
    let mut map = header_map(&e.response_headers)?;
    normalize_headers(&mut map, bytes.len(), allowed);
    e.response_headers = headers(&map);
    e.response_bytes = bytes.len() as i64;
    Ok(())
}
impl Capture {
    fn fail(&mut self, message: String) -> Response<Body> {
        if let Some(e) = self.exchange.as_mut() {
            e.error = Some(message.clone());
            e.completed = true;
            e.duration_ms = self.start.elapsed().as_millis() as i64;
            if let Err(error) = self.engine.save(e) {
                eprintln!("Capture persistence failed: {error}");
            }
        }
        error_response(StatusCode::BAD_GATEWAY, &message)
    }
    async fn request(&mut self, req: Request<Body>) -> Result<RequestOrResponse> {
        self.start = Instant::now();
        let (mut parts, body) = req.into_parts();
        self.exchange = Some(self.engine.fresh_exchange(
            &self.session,
            parts.method.to_string(),
            parts.uri.to_string(),
        ));
        let e = self.exchange.as_mut().unwrap();
        e.request_headers = headers(&parts.headers);
        self.matched_rules = self.engine.matching_rules(&e.method, &e.url);
        self.engine.save(e)?;
        let bytes = collect(body).await?;
        (e.request_body, e.request_body_base64) = encode_body(&bytes);
        e.request_bytes = bytes.len() as i64;
        self.engine.save(e)?;
        let terminal = apply_request_rules(e, &self.matched_rules)?;
        let rule_pause = record_pause_rules(e, &self.matched_rules, PauseStage::Request);
        self.engine.save(e)?;
        if rule_pause || self.engine.inner.request_breakpoint.load(Ordering::SeqCst) {
            let edit = self.engine.pause(e, "request").await?;
            apply_request_edit(e, &edit)?;
        }
        parts.method = e.method.parse()?;
        parts.uri = e.url.parse()?;
        parts.headers = header_map(&e.request_headers)?;
        let bytes = decode_body(&e.request_body, e.request_body_base64)?;
        normalize_headers(&mut parts.headers, bytes.len(), true);
        if let Some(authority) = parts.uri.authority() {
            parts
                .headers
                .insert(HOST, HeaderValue::from_str(authority.as_str())?);
        }
        e.request_headers = headers(&parts.headers);
        self.engine.save(e)?;
        if let Some(edit) = terminal {
            let response = Response::builder()
                .status(edit.status.unwrap_or(403))
                .body(Body::from(decode_body(
                    edit.body.as_deref().unwrap_or(""),
                    edit.body_base64.unwrap_or(false),
                )?))?;
            let (mut parts, body) = response.into_parts();
            parts.headers = header_map(edit.headers.as_deref().unwrap_or(&[]))?;
            return Ok(self
                .response(Response::from_parts(parts, body))
                .await?
                .into());
        }
        Ok(Request::from_parts(parts, Body::from(bytes)).into())
    }
    async fn response(&mut self, res: Response<Body>) -> Result<Response<Body>> {
        let (mut parts, body) = res.into_parts();
        let e = self
            .exchange
            .as_mut()
            .context("Response without matching request")?;
        e.status = Some(parts.status.as_u16());
        e.response_headers = headers(&parts.headers);
        let bytes = collect(body).await?;
        (e.response_body, e.response_body_base64) = encode_body(&bytes);
        e.response_bytes = bytes.len() as i64;
        self.engine.save(e)?;
        apply_response_rules(e, &self.matched_rules)?;
        let rule_pause = record_pause_rules(e, &self.matched_rules, PauseStage::Response);
        if rule_pause || self.engine.inner.response_breakpoint.load(Ordering::SeqCst) {
            self.engine.save(e)?;
            let edit = self.engine.pause(e, "response").await?;
            apply_response_edit(e, &edit)?;
        }
        let mut bytes = decode_body(&e.response_body, e.response_body_base64)?;
        parts.status = StatusCode::from_u16(e.status.unwrap_or(502))?;
        parts.headers = header_map(&e.response_headers)?;
        let allowed = e.method != "HEAD" && !matches!(parts.status.as_u16(), 100..=199 | 204 | 304);
        if !allowed {
            bytes.clear();
            e.response_body.clear();
            e.response_body_base64 = false;
        }
        normalize_headers(&mut parts.headers, bytes.len(), allowed);
        e.response_headers = headers(&parts.headers);
        e.response_bytes = bytes.len() as i64;
        e.error = None;
        e.completed = true;
        e.duration_ms = self.start.elapsed().as_millis() as i64;
        self.engine.save(e)?;
        Ok(Response::from_parts(parts, Body::from(bytes)))
    }
}
impl HttpHandler for Capture {
    async fn handle_request(
        &mut self,
        _ctx: &HttpContext,
        req: Request<Body>,
    ) -> RequestOrResponse {
        if req.method() == Method::CONNECT {
            return req.into();
        }
        if req.headers().get("upgrade").is_some() {
            return error_response(
                StatusCode::NOT_IMPLEMENTED,
                "WebSocket capture is not implemented in this release",
            )
            .into();
        }
        match self.request(req).await {
            Ok(req) => req,
            Err(err) => self.fail(format!("{err:#}")).into(),
        }
    }
    async fn handle_response(&mut self, _ctx: &HttpContext, res: Response<Body>) -> Response<Body> {
        match self.response(res).await {
            Ok(res) => res,
            Err(err) => self.fail(format!("{err:#}")),
        }
    }
    async fn handle_error(
        &mut self,
        _ctx: &HttpContext,
        err: hudsucker::hyper_util::client::legacy::Error,
    ) -> Response<Body> {
        self.fail(format!("Upstream request failed: {err:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn breakpoint_expiry_removes_pending_and_aborts() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::open(dir.path()).unwrap();
        let session = engine
            .create_session("Expiry".into(), "test".into())
            .unwrap();
        let exchange =
            engine.fresh_exchange(&session.id, "GET".into(), "https://example.com/".into());
        let waiting = engine.clone();
        let task = tokio::spawn(async move { waiting.pause(&exchange, "request").await });
        tokio::task::yield_now().await;
        assert_eq!(engine.pending().len(), 1);
        tokio::time::advance(Duration::from_secs(BREAKPOINT_TIMEOUT_SECONDS + 1)).await;
        let error = task.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("expired"));
        assert!(engine.pending().is_empty());
    }
}
