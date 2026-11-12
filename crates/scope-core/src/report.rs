use super::*;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

fn percentile(values: &[i64], p: f64) -> i64 {
    if values.is_empty() {
        return 0;
    }
    let mut values = values.to_vec();
    values.sort_unstable();
    values[((values.len() as f64 * p).ceil() as usize)
        .saturating_sub(1)
        .min(values.len() - 1)]
}
fn endpoint(e: &Exchange) -> (String, String, String) {
    match url::Url::parse(&e.url) {
        Ok(url) => (
            e.method.clone(),
            url.host_str().unwrap_or("").into(),
            url.path()
                .split('/')
                .map(|part| {
                    if !part.is_empty()
                        && (part.chars().all(|c| c.is_ascii_digit())
                            || Uuid::parse_str(part).is_ok())
                    {
                        ":id"
                    } else {
                        part
                    }
                })
                .collect::<Vec<_>>()
                .join("/"),
        ),
        Err(_) => (e.method.clone(), String::new(), e.url.clone()),
    }
}
fn key(e: &Exchange) -> String {
    let (m, h, p) = endpoint(e);
    format!("{m} {h}{p}")
}
fn shape(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                out.insert(format!("{path}:{}", kind(v)));
                shape(v, &path, out);
            }
        }
        Value::Array(values) => {
            for v in values.iter().take(50) {
                shape(v, &format!("{prefix}[]"), out);
            }
        }
        _ => {}
    }
}
fn kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
fn response_shape(exchanges: &[&Exchange]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for e in exchanges {
        if !e.response_body_base64 {
            if let Ok(value) = serde_json::from_str::<Value>(&e.response_body) {
                shape(&value, "", &mut out);
            }
        }
    }
    out
}
fn average(values: &[i64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().map(|n| *n as f64).sum::<f64>() / values.len() as f64
    }
}
fn observation(kind: &str, title: String, detail: &str, evidence: &[&Exchange]) -> Value {
    json!({"kind":kind,"title":title,"detail":detail,"count":evidence.len(),"exchangeIds":evidence.iter().take(50).map(|e|&e.id).collect::<Vec<_>>(),"evidenceTruncated":evidence.len()>50})
}
fn sensitive_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase().replace(['-', '_'], "");
    [
        "authorization",
        "cookie",
        "token",
        "apikey",
        "password",
        "secret",
        "sessionid",
    ]
    .iter()
    .any(|key| name.contains(key))
}
fn has_sensitive_headers(headers: &[Header]) -> bool {
    headers.iter().any(|header| sensitive_name(&header.name))
}
fn has_sensitive_query(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| url.query_pairs().any(|(name, _)| sensitive_name(&name)))
}
impl Engine {
    pub fn analysis(&self, session_id: &str) -> Result<Value> {
        let exchanges = self.all_exchanges(session_id)?;
        let completed: Vec<_> = exchanges.iter().filter(|e| e.completed).collect();
        let failed =
            |e: &&Exchange| e.error.is_some() || e.status.is_some_and(|status| status >= 400);
        let durations: Vec<_> = completed.iter().map(|e| e.duration_ms).collect();
        let mut hosts: BTreeMap<String, usize> = BTreeMap::new();
        let mut codes: BTreeMap<u16, usize> = BTreeMap::new();
        let mut groups: BTreeMap<String, Vec<&Exchange>> = BTreeMap::new();
        for exchange in &exchanges {
            *hosts.entry(endpoint(exchange).1).or_default() += 1;
            if exchange.completed {
                *codes.entry(exchange.status.unwrap_or(0)).or_default() += 1;
            }
            groups.entry(key(exchange)).or_default().push(exchange);
        }
        let endpoints: Vec<_> = groups.values().map(|items| {
            let (method,host,path) = endpoint(items[0]);
            let finished: Vec<_> = items.iter().copied().filter(|e| e.completed).collect();
            let times: Vec<_> = finished.iter().map(|e| e.duration_ms).collect();
            let mut statuses = BTreeMap::new();
            for item in &finished { *statuses.entry(item.status.unwrap_or(0)).or_insert(0) += 1; }
            json!({"method":method,"host":host,"path":path,"count":items.len(),"completedCount":finished.len(),"inFlightCount":items.len()-finished.len(),"errorCount":finished.iter().filter(|e|failed(e)).count(),"mockCount":items.iter().filter(|e|e.provenance==TrafficProvenance::Mock).count(),"blockedCount":items.iter().filter(|e|e.provenance==TrafficProvenance::Blocked).count(),"averageDurationMs":average(&times),"p95DurationMs":percentile(&times,0.95),"totalBytes":items.iter().map(|e|e.request_bytes+e.response_bytes).sum::<i64>(),"statuses":statuses})
        }).collect();
        let mut insights = Vec::new();
        let errors: Vec<_> = completed.iter().copied().filter(|e| failed(e)).collect();
        if !errors.is_empty() {
            insights.push(observation("error", format!("{} failed request{}",errors.len(),if errors.len()==1 {""} else {"s"}),"Completed HTTP 4xx/5xx responses and capture or transport errors were observed. Mocked and blocked traffic is included and marked on each exchange.",&errors));
        }
        let slow: Vec<_> = completed
            .iter()
            .copied()
            .filter(|e| e.duration_ms > 1000)
            .collect();
        if !slow.is_empty() {
            insights.push(observation("latency",format!("{} request{} exceeded one second",slow.len(),if slow.len()==1 {""} else {"s"}),"Duration includes breakpoint waiting and body buffering. It is not pure server latency.",&slow));
        }
        for (name, items) in groups
            .iter()
            .filter(|(_, items)| items.iter().filter(|e| e.completed).count() >= 3)
            .take(12)
        {
            let finished: Vec<_> = items.iter().copied().filter(|e| e.completed).collect();
            insights.push(observation("repeat",format!("{} calls to {name}",finished.len()),"Repeated completed requests were observed. Polling, retries and user actions are possible explanations; capture alone does not establish a bug.",&finished));
        }
        let cleartext: Vec<_> = completed
            .iter()
            .copied()
            .filter(|e| {
                e.url.starts_with("http://")
                    && (has_sensitive_headers(&e.request_headers) || has_sensitive_query(&e.url))
            })
            .collect();
        if !cleartext.is_empty() {
            insights.push(observation("security",format!("{} cleartext request{} carried credential-like fields",cleartext.len(),if cleartext.len()==1 {""} else {"s"}),"Authorization, cookie or credential-like query field names were observed over HTTP. Values are omitted from this finding. Local fixtures can legitimately use HTTP; review the linked evidence.",&cleartext));
        }
        let synthetic: Vec<_> = completed
            .iter()
            .copied()
            .filter(|e| e.provenance != TrafficProvenance::Upstream)
            .collect();
        if !synthetic.is_empty() {
            insights.push(observation("rules",format!("{} response{} generated locally",synthetic.len(),if synthetic.len()==1 {""} else {"s"}),"Mock and block rules produced these responses without contacting the upstream server. These are not observations of upstream behavior.",&synthetic));
        }
        Ok(
            json!({"totalRequests":exchanges.len(),"completedRequests":completed.len(),"inFlightRequests":exchanges.len()-completed.len(),"mockRequests":exchanges.iter().filter(|e|e.provenance==TrafficProvenance::Mock).count(),"blockedRequests":exchanges.iter().filter(|e|e.provenance==TrafficProvenance::Blocked).count(),"upstreamRequests":exchanges.iter().filter(|e|e.provenance==TrafficProvenance::Upstream).count(),"totalErrors":errors.len(),"totalBytes":exchanges.iter().map(|e|e.request_bytes+e.response_bytes).sum::<i64>(),"averageDurationMs":average(&durations),"p95DurationMs":percentile(&durations,0.95),"hosts":hosts.into_iter().map(|(host,count)|json!({"host":host,"count":count})).collect::<Vec<_>>(),"endpoints":endpoints,"statusCodes":codes.into_iter().map(|(status,count)|json!({"status":status,"count":count})).collect::<Vec<_>>(),"insights":insights,"method":"Counts include saved in-flight exchanges. Error counts, timings, status codes and observations use completed exchanges only. Evidence lists are limited to 50 IDs per observation; synthetic responses are identified explicitly."}),
        )
    }
    pub fn compare_sessions(&self, left: &str, right: &str) -> Result<Value> {
        let left_s = self.session(left)?;
        let right_s = self.session(right)?;
        let left_e = self.all_exchanges(left)?;
        let right_e = self.all_exchanges(right)?;
        let mut a: BTreeMap<String, Vec<&Exchange>> = BTreeMap::new();
        let mut b: BTreeMap<String, Vec<&Exchange>> = BTreeMap::new();
        for e in &left_e {
            a.entry(key(e)).or_default().push(e);
        }
        for e in &right_e {
            b.entry(key(e)).or_default().push(e);
        }
        let added: Vec<_> = b.keys().filter(|k| !a.contains_key(*k)).cloned().collect();
        let removed: Vec<_> = a.keys().filter(|k| !b.contains_key(*k)).cloned().collect();
        let mut changed = Vec::new();
        for (key, av) in &a {
            if let Some(bv) = b.get(key) {
                let ac: BTreeSet<_> = av.iter().map(|e| e.status.unwrap_or(0)).collect();
                let bc: BTreeSet<_> = bv.iter().map(|e| e.status.unwrap_or(0)).collect();
                let ash = response_shape(av);
                let bsh = response_shape(bv);
                let added_fields: Vec<_> = bsh.difference(&ash).cloned().collect();
                let removed_fields: Vec<_> = ash.difference(&bsh).cloned().collect();
                let avg = |v: &Vec<&Exchange>| {
                    v.iter().map(|e| e.duration_ms).sum::<i64>() as f64 / v.len() as f64
                };
                if av.len() != bv.len()
                    || ac != bc
                    || ash != bsh
                    || ((avg(bv) - avg(av)).abs() > 100.0)
                {
                    changed.push(json!({"endpoint":key,"leftCount":av.len(),"rightCount":bv.len(),"leftStatuses":ac,"rightStatuses":bc,"leftAverageDurationMs":avg(av),"rightAverageDurationMs":avg(bv),"addedResponseFields":added_fields,"removedResponseFields":removed_fields,"leftExchangeIds":av.iter().map(|e|&e.id).collect::<Vec<_>>(),"rightExchangeIds":bv.iter().map(|e|&e.id).collect::<Vec<_>>()}));
                }
            }
        }
        Ok(
            json!({"leftSession":left_s,"rightSession":right_s,"addedEndpoints":added,"removedEndpoints":removed,"changedEndpoints":changed,"summary":{"leftRequests":left_e.len(),"rightRequests":right_e.len(),"addedEndpoints":added.len(),"removedEndpoints":removed.len(),"changedEndpoints":changed.len()},"method":"Exact normalized endpoint comparison; numeric and UUID path segments become :id. JSON shapes reflect observed payloads, not an inferred API contract."}),
        )
    }
    pub fn export_session(&self, session_id: &str) -> Result<Value> {
        Ok(
            json!({"format":"scope-session","version":1,"session":self.session(session_id)?,"exchanges":self.all_exchanges(session_id)?}),
        )
    }
    pub fn import_session(&self, mut value: Value) -> Result<Session> {
        if value["format"] != "scope-session" || value["version"] != 1 {
            bail!("Unsupported Snurpo session format/version");
        }
        let original: Session = serde_json::from_value(value["session"].clone())?;
        let exchanges: Vec<Exchange> = serde_json::from_value(value["exchanges"].take())?;
        self.import_validated(
            original.name,
            "import".into(),
            exchanges,
            Some(SessionUpdate {
                notes: Some(original.notes),
                tags: Some(original.tags),
                pinned: Some(original.pinned),
                ..SessionUpdate::default()
            }),
        )
    }
    fn import_validated(
        &self,
        name: String,
        source: String,
        mut exchanges: Vec<Exchange>,
        metadata: Option<SessionUpdate>,
    ) -> Result<Session> {
        if name.trim().is_empty() || name.len() > 256 {
            bail!("Session name must contain 1–256 characters");
        }
        if exchanges.len() > 100_000 {
            bail!("Import exceeds 100,000 exchanges");
        }
        for e in &exchanges {
            validate_url(&e.url)?;
            e.method.parse::<hudsucker::hyper::Method>()?;
            proxy::header_map(&e.request_headers)?;
            proxy::header_map(&e.response_headers)?;
            if let Some(status) = e.status {
                hudsucker::hyper::StatusCode::from_u16(status)?;
            }
            for (body, base64) in [
                (&e.request_body, e.request_body_base64),
                (&e.response_body, e.response_body_base64),
            ] {
                if decode_body(body, base64)?.len() > CAPTURE_LIMIT_BYTES {
                    bail!("Imported body exceeds 16 MiB capture limit");
                }
            }
            if e.duration_ms < 0 || e.request_bytes < 0 || e.response_bytes < 0 {
                bail!("Imported timing and sizes cannot be negative");
            }
        }
        let mut s = Session {
            id: uid(),
            name,
            source,
            created_at: now(),
            exchange_count: exchanges.len() as i64,
            revision: exchanges.len() as i64,
            notes: String::new(),
            tags: vec![],
            pinned: false,
        };
        sessions::apply_update(&mut s, metadata.unwrap_or_default())?;
        let name = s.name.clone();
        sessions::apply_update(
            &mut s,
            SessionUpdate {
                name: Some(name),
                ..SessionUpdate::default()
            },
        )?;
        let mut db = self.inner.db.lock().unwrap();
        let tx = db.transaction()?;
        tx.execute(
            "INSERT INTO sessions(id,name,source,created_at,notes,tags,pinned) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![s.id, s.name, s.source, s.created_at,s.notes,serde_json::to_string(&s.tags)?,s.pinned],
        )?;
        for e in &mut exchanges {
            e.id = uid();
            e.session_id = s.id.clone();
            if !e.completed {
                e.completed = true;
                if e.error.is_none() {
                    e.error =
                        Some("Imported exchange was not completed in the source capture".into());
                }
            }
            tx.execute(
                "INSERT INTO exchanges VALUES(?1,?2,?3,?4)",
                params![e.id, e.session_id, e.started_at, serde_json::to_string(e)?],
            )?;
        }
        sessions::audit(&tx, "session.import", Some(&s.id))?;
        tx.commit()?;
        Ok(s)
    }
    pub fn export_har(&self, session_id: &str) -> Result<Value> {
        let session = self.session(session_id)?;
        let entries:Vec<_>=self.all_exchanges(session_id)?.into_iter().map(|e| {
            let mime=|headers:&[Header]|headers.iter().find(|h|h.name.eq_ignore_ascii_case("content-type")).map(|h|h.value.clone()).unwrap_or_else(||"application/octet-stream".into());
            let url=url::Url::parse(&e.url).ok();let query=url.as_ref().map(|u|u.query_pairs().map(|(name,value)|json!({"name":name,"value":value})).collect::<Vec<_>>()).unwrap_or_default();
            let date=chrono::DateTime::from_timestamp_millis(e.started_at).map(|v|v.to_rfc3339_opts(chrono::SecondsFormat::Millis,true)).unwrap_or_else(||chrono::Utc::now().to_rfc3339());
            json!({"startedDateTime":date,"time":e.duration_ms,"pageref":session_id,"request":{"method":e.method,"url":e.url,"httpVersion":"HTTP/1.1","cookies":[],"headers":e.request_headers,"queryString":query,"headersSize":-1,"bodySize":e.request_bytes,"postData":{"mimeType":mime(&e.request_headers),"text":e.request_body,"_encoding":if e.request_body_base64{Some("base64")}else{None}}},"response":{"status":e.status.unwrap_or(0),"statusText":e.status.and_then(|s|hudsucker::hyper::StatusCode::from_u16(s).ok()).and_then(|s|s.canonical_reason().map(str::to_owned)).unwrap_or_default(),"httpVersion":"HTTP/1.1","cookies":[],"headers":e.response_headers,"content":{"size":e.response_bytes,"mimeType":mime(&e.response_headers),"text":e.response_body,"encoding":if e.response_body_base64{Some("base64")}else{None}},"redirectURL":e.response_headers.iter().find(|h|h.name.eq_ignore_ascii_case("location")).map(|h|&h.value).cloned().unwrap_or_default(),"headersSize":-1,"bodySize":e.response_bytes},"cache":{},"timings":{"send":0,"wait":e.duration_ms,"receive":0},"_scope":{"completed":e.completed,"provenance":e.provenance,"appliedRules":e.applied_rules,"error":e.error,"edited":e.edited,"originalRequest":e.original_request,"originalResponse":e.original_response,"protocolNote":"HTTP version is normalized; original version and trailers are not recorded"}})
        }).collect();
        Ok(
            json!({"log":{"version":"1.2","creator":{"name":"Snurpo","version":env!("CARGO_PKG_VERSION")},"pages":[{"id":session_id,"title":session.name,"startedDateTime":chrono::DateTime::from_timestamp_millis(session.created_at).unwrap().to_rfc3339(),"pageTimings":{}}],"entries":entries}}),
        )
    }
    pub fn import_har(&self, har: Value, name: String) -> Result<Session> {
        let entries = har["log"]["entries"]
            .as_array()
            .context("HAR must contain log.entries array")?;
        if entries.len() > 100_000 {
            bail!("Import exceeds 100,000 exchanges");
        }
        let mut exchanges = Vec::new();
        for item in entries {
            let req = &item["request"];
            let res = &item["response"];
            let scope = &item["_scope"];
            let mut e = self.fresh_exchange(
                "",
                req["method"]
                    .as_str()
                    .context("HAR request missing method")?
                    .into(),
                req["url"]
                    .as_str()
                    .context("HAR request missing URL")?
                    .into(),
            );
            let parse_headers = |v: &Value| -> Result<Vec<Header>> {
                if v.is_null() {
                    Ok(vec![])
                } else {
                    Ok(serde_json::from_value(v.clone())?)
                }
            };
            e.request_headers = parse_headers(&req["headers"])?;
            e.response_headers = parse_headers(&res["headers"])?;
            e.request_body = req["postData"]["text"].as_str().unwrap_or("").into();
            e.request_body_base64 = req["postData"]["_encoding"] == "base64";
            e.response_body = res["content"]["text"].as_str().unwrap_or("").into();
            e.response_body_base64 = res["content"]["encoding"] == "base64";
            e.request_bytes = decode_body(&e.request_body, e.request_body_base64)?.len() as i64;
            e.response_bytes = decode_body(&e.response_body, e.response_body_base64)?.len() as i64;
            e.started_at = item["startedDateTime"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.timestamp_millis())
                .unwrap_or_else(now);
            e.duration_ms = item["time"].as_f64().unwrap_or(0.0).max(0.0) as i64;
            e.status = match res["status"].as_u64() {
                Some(0) | None => None,
                Some(s) if s <= u16::MAX as u64 => Some(s as u16),
                _ => bail!("Invalid HAR response status"),
            };
            e.error = scope["error"].as_str().map(str::to_owned).or_else(|| {
                if e.status.is_none() {
                    Some("HAR entry has no completed response".into())
                } else {
                    None
                }
            });
            e.completed = scope["completed"].as_bool().unwrap_or(true);
            if let Some(value) = scope.get("provenance") {
                e.provenance = serde_json::from_value(value.clone())
                    .context("Invalid HAR Snurpo provenance")?;
            }
            if let Some(value) = scope.get("appliedRules") {
                e.applied_rules = serde_json::from_value(value.clone())
                    .context("Invalid HAR Snurpo rule evidence")?;
            }
            e.edited = scope["edited"].as_bool().unwrap_or(false);
            e.original_request = scope
                .get("originalRequest")
                .filter(|v| !v.is_null())
                .cloned();
            e.original_response = scope
                .get("originalResponse")
                .filter(|v| !v.is_null())
                .cloned();
            exchanges.push(e);
        }
        self.import_validated(name, "har".into(), exchanges, None)
    }
}

struct SchemaBudget {
    remaining: usize,
    truncated: bool,
}
fn bounded_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
fn infer_schema(value: &Value, depth: usize, budget: &mut SchemaBudget) -> Value {
    if depth >= 6 || budget.remaining == 0 {
        budget.truncated = true;
        return json!({});
    }
    budget.remaining -= 1;
    match value {
        Value::Null => json!({"type":"null"}),
        Value::Bool(_) => json!({"type":"boolean"}),
        Value::Number(number) => {
            json!({"type":if number.is_i64() || number.is_u64() { "integer" } else { "number" }})
        }
        Value::String(_) => json!({"type":"string"}),
        Value::Array(values) => {
            if values.len() > 20 {
                budget.truncated = true;
            }
            let mut items = json!({});
            for value in values.iter().take(20) {
                let schema = infer_schema(value, depth + 1, budget);
                merge_schema(&mut items, schema, budget);
            }
            json!({"type":"array","items":items})
        }
        Value::Object(values) => {
            if values.len() > 40 {
                budget.truncated = true;
            }
            let mut properties = serde_json::Map::new();
            for (key, value) in values.iter().take(40) {
                if bounded_name(key) {
                    properties.insert(key.clone(), infer_schema(value, depth + 1, budget));
                } else {
                    budget.truncated = true;
                }
            }
            // Observed presence is deliberately not used to infer required fields.
            json!({"type":"object","properties":properties,"additionalProperties":true})
        }
    }
}
fn merge_schema(target: &mut Value, next: Value, budget: &mut SchemaBudget) {
    if next == json!({}) {
        return;
    }
    if *target == json!({}) {
        *target = next;
        return;
    }
    if *target == next {
        return;
    }
    if target["type"] == "object" && next["type"] == "object" {
        let properties = target["properties"].as_object_mut().unwrap();
        for (key, schema) in next["properties"].as_object().unwrap() {
            if let Some(existing) = properties.get_mut(key) {
                merge_schema(existing, schema.clone(), budget);
            } else if properties.len() < 40 {
                properties.insert(key.clone(), schema.clone());
            } else {
                budget.truncated = true;
            }
        }
    } else if target["type"] == "array" && next["type"] == "array" {
        merge_schema(&mut target["items"], next["items"].clone(), budget);
    } else {
        let mut alternatives = target
            .get("anyOf")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_else(|| vec![target.clone()]);
        let incoming = next
            .get("anyOf")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_else(|| vec![next]);
        for schema in incoming {
            if !alternatives.contains(&schema) {
                if alternatives.len() < 8 {
                    alternatives.push(schema);
                } else {
                    budget.truncated = true;
                }
            }
        }
        *target = json!({"anyOf":alternatives});
    }
}
fn observed_media(headers: &[Header]) -> String {
    let media = headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("content-type"))
        .map(|header| {
            header
                .value
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
        .unwrap_or_default();
    let token = |value: &str| {
        !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&byte))
    };
    let valid = media.len() <= 128
        && media
            .split_once('/')
            .is_some_and(|(a, b)| token(a) && token(b));
    if valid {
        media
    } else {
        "application/octet-stream".into()
    }
}
fn observed_body_schema(body: &str, base64: bool, media: &str, budget: &mut SchemaBudget) -> Value {
    if base64 {
        return json!({"type":"string","description":"Binary payload; structure was not inferred."});
    }
    if media == "application/json" || media.ends_with("+json") {
        if body.len() <= 256 * 1024 {
            if let Ok(value) = serde_json::from_str::<Value>(body) {
                return infer_schema(&value, 0, budget);
            }
        } else {
            budget.truncated = true;
        }
        json!({"description":"JSON structure was unavailable or exceeded the inference limit."})
    } else {
        json!({"type":"string","description":"Non-JSON payload; structure was not inferred."})
    }
}
fn openapi_path(url: &url::Url) -> (String, Vec<String>) {
    let mut parameters = Vec::new();
    let path = url
        .path()
        .split('/')
        .map(|part| {
            if !part.is_empty()
                && (part.chars().all(|ch| ch.is_ascii_digit()) || Uuid::parse_str(part).is_ok())
            {
                let name = format!("id{}", parameters.len() + 1);
                parameters.push(name.clone());
                format!("{{{name}}}")
            } else {
                part.replace('{', "%7B").replace('}', "%7D")
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    (if path.is_empty() { "/".into() } else { path }, parameters)
}
impl Engine {
    /// An observed-only description, never an authoritative contract. Payload values,
    /// query values, credential headers and session names are intentionally omitted.
    pub fn infer_openapi(&self, session_id: &str) -> Result<Value> {
        let exchanges = self.all_exchanges(session_id)?;
        let eligible: Vec<_> = exchanges
            .iter()
            .filter(|e| {
                e.completed
                    && e.error.is_none()
                    && e.status.is_some_and(|status| (200..=599).contains(&status))
            })
            .collect();
        let mut budget = SchemaBudget {
            remaining: 10000,
            truncated: eligible.len() > 5000,
        };
        let mut groups: BTreeMap<(String, String), Vec<&Exchange>> = BTreeMap::new();
        let mut omitted = 0usize;
        for exchange in eligible.iter().take(5000) {
            let method = exchange.method.to_ascii_lowercase();
            if ![
                "get", "put", "post", "delete", "options", "head", "patch", "trace",
            ]
            .contains(&method.as_str())
            {
                omitted += 1;
                continue;
            }
            let Ok(url) = url::Url::parse(&exchange.url) else {
                omitted += 1;
                continue;
            };
            let (path, _) = openapi_path(&url);
            let key = (path, method);
            if !groups.contains_key(&key) && groups.len() >= 200 {
                budget.truncated = true;
                omitted += 1;
                continue;
            }
            groups.entry(key).or_default().push(exchange);
        }
        let mut paths = serde_json::Map::new();
        for ((path, method), items) in &groups {
            let mut servers = BTreeSet::new();
            let mut queries = BTreeSet::new();
            let mut path_parameters = Vec::new();
            let mut request_content = serde_json::Map::new();
            let mut responses = serde_json::Map::new();
            for exchange in items {
                let url = url::Url::parse(&exchange.url)?;
                if servers.len() < 20 {
                    servers.insert(url.origin().ascii_serialization());
                } else {
                    budget.truncated = true;
                }
                path_parameters = openapi_path(&url).1;
                for (name, _) in url.query_pairs() {
                    if bounded_name(&name) && queries.len() < 50 {
                        queries.insert(name.into_owned());
                    } else {
                        budget.truncated = true;
                    }
                }
                if !exchange.request_body.is_empty() {
                    let media = observed_media(&exchange.request_headers);
                    if request_content.contains_key(&media) || request_content.len() < 4 {
                        let schema = observed_body_schema(
                            &exchange.request_body,
                            exchange.request_body_base64,
                            &media,
                            &mut budget,
                        );
                        let existing = request_content
                            .entry(media)
                            .or_insert_with(|| json!({"schema":{}}));
                        merge_schema(&mut existing["schema"], schema, &mut budget);
                    } else {
                        budget.truncated = true;
                    }
                }
                let status = exchange.status.unwrap().to_string();
                if !responses.contains_key(&status) && responses.len() >= 16 {
                    budget.truncated = true;
                    continue;
                }
                let response=responses.entry(status).or_insert_with(||json!({"description":"Observed response; completeness and required fields are unknown.","content":{}}));
                if !exchange.response_body.is_empty() {
                    let media = observed_media(&exchange.response_headers);
                    let content = response["content"].as_object_mut().unwrap();
                    if content.contains_key(&media) || content.len() < 4 {
                        let schema = observed_body_schema(
                            &exchange.response_body,
                            exchange.response_body_base64,
                            &media,
                            &mut budget,
                        );
                        let existing = content.entry(media).or_insert_with(|| json!({"schema":{}}));
                        merge_schema(&mut existing["schema"], schema, &mut budget);
                    } else {
                        budget.truncated = true;
                    }
                }
            }
            let mut parameters:Vec<Value>=path_parameters.iter().map(|name|json!({"name":name,"in":"path","required":true,"schema":{"type":"string"},"description":"Numeric or UUID segment normalized from observed requests; values omitted."})).collect();
            parameters.extend(queries.iter().map(|name|json!({"name":name,"in":"query","required":false,"schema":{"type":"string"},"description":"Observed query name only. Values and requiredness were not inferred."})));
            let mut operation = json!({"description":"Inferred only from captured observations. Synthetic responses are included and counted separately; review before using as an API contract.","servers":servers.iter().map(|url|json!({"url":url})).collect::<Vec<_>>(),"parameters":parameters,"responses":responses,"x-scope-observations":{"count":items.len(),"upstream":items.iter().filter(|e|e.provenance==TrafficProvenance::Upstream).count(),"mock":items.iter().filter(|e|e.provenance==TrafficProvenance::Mock).count(),"blocked":items.iter().filter(|e|e.provenance==TrafficProvenance::Blocked).count(),"exchangeIds":items.iter().take(50).map(|e|&e.id).collect::<Vec<_>>(),"evidenceTruncated":items.len()>50}});
            if !request_content.is_empty() {
                operation["requestBody"] = json!({"required":false,"content":request_content});
            }
            paths.entry(path.clone()).or_insert_with(|| json!({}))[method] = operation;
        }
        Ok(
            json!({"openapi":"3.1.1","info":{"title":"Snurpo observed HTTP API","version":"observed","description":"Generated from completed captured HTTP responses. This describes observed traffic, not a verified API contract. No example values, query values, header credentials or session names are emitted."},"paths":paths,"x-scope":{"sessionId":session_id,"observedOnly":true,"eligibleExchanges":eligible.len(),"sampledExchanges":eligible.len().min(5000),"operationCount":groups.len(),"omittedExchanges":omitted,"truncated":budget.truncated,"limits":{"exchanges":5000,"operations":200,"schemaNodes":10000,"schemaDepth":6,"propertiesPerObject":40,"arrayItems":20,"jsonBodyBytes":262144,"evidenceIdsPerOperation":50},"limitations":["Only completed exchanges with a final response and no capture error are used.","Numeric and UUID segments are normalized; other static path segments and server names remain visible and may be sensitive.","Origins with the same normalized path and method are grouped into one operation and listed as observed servers.","JSON properties and types are observations only; requiredness, authentication, enums, constraints and unobserved responses are not inferred.","Mocks and blocks are synthetic observations and may differ from upstream behavior.","Bodies and schema traversal are bounded; unsupported HTTP methods and excess observations are omitted."]}}),
        )
    }
}
