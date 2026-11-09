use super::*;

pub const MAX_RULES: usize = 256;
pub const MAX_RULE_BODY_BYTES: usize = 1024 * 1024;
pub const MAX_RULE_CONFIG_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PauseStage {
    Request,
    Response,
    Both,
}
impl<'de> Deserialize<'de> for PauseStage {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        // Unit enums normally also accept {"request":null}. The wire contract
        // deliberately accepts only the three documented string values.
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "request" => Ok(Self::Request),
            "response" => Ok(Self::Response),
            "both" => Ok(Self::Both),
            _ => Err(serde::de::Error::unknown_variant(
                &value,
                &["request", "response", "both"],
            )),
        }
    }
}
impl PauseStage {
    pub(crate) fn includes(self, stage: Self) -> bool {
        self == Self::Both || self == stage
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleMatcher {
    pub method: Option<String>,
    pub host: Option<String>,
    pub path_prefix: Option<String>,
    pub url_contains: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum RuleAction {
    #[serde(rename_all = "camelCase")]
    RewriteRequest {
        method: Option<String>,
        url: Option<String>,
        headers: Option<Vec<Header>>,
        body: Option<String>,
        body_base64: Option<bool>,
    },
    #[serde(rename_all = "camelCase")]
    RewriteResponse {
        status: Option<u16>,
        headers: Option<Vec<Header>>,
        body: Option<String>,
        body_base64: Option<bool>,
    },
    #[serde(rename_all = "camelCase")]
    MockResponse {
        status: u16,
        #[serde(default)]
        headers: Vec<Header>,
        #[serde(default)]
        body: String,
        #[serde(default)]
        body_base64: bool,
    },
    Block {
        reason: Option<String>,
    },
    Pause {
        stage: PauseStage,
    },
}
impl RuleAction {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::RewriteRequest { .. } => "rewriteRequest",
            Self::RewriteResponse { .. } => "rewriteResponse",
            Self::MockResponse { .. } => "mockResponse",
            Self::Block { .. } => "block",
            Self::Pause { .. } => "pause",
        }
    }
    pub(crate) fn edit(&self) -> EditMessage {
        match self {
            Self::RewriteRequest {
                method,
                url,
                headers,
                body,
                body_base64,
            } => EditMessage {
                action: "continue".into(),
                method: method.clone(),
                url: url.clone(),
                headers: headers.clone(),
                body: body.clone(),
                body_base64: *body_base64,
                ..EditMessage::default()
            },
            Self::RewriteResponse {
                status,
                headers,
                body,
                body_base64,
            } => EditMessage {
                action: "continue".into(),
                status: *status,
                headers: headers.clone(),
                body: body.clone(),
                body_base64: *body_base64,
                ..EditMessage::default()
            },
            Self::MockResponse {
                status,
                headers,
                body,
                body_base64,
            } => EditMessage {
                action: "continue".into(),
                status: Some(*status),
                headers: Some(headers.clone()),
                body: Some(body.clone()),
                body_base64: Some(*body_base64),
                ..EditMessage::default()
            },
            Self::Block { reason } => EditMessage {
                action: "continue".into(),
                status: Some(403),
                headers: Some(vec![Header {
                    name: "content-type".into(),
                    value: "text/plain; charset=utf-8".into(),
                }]),
                body: Some(format!(
                    "Scope blocked this request{}",
                    reason
                        .as_ref()
                        .map(|s| format!(": {s}"))
                        .unwrap_or_default()
                )),
                ..EditMessage::default()
            },
            // A pause has no automatic edit. Its continuation uses the same
            // validated PendingExchange/EditMessage path as global breakpoints.
            Self::Pause { .. } => EditMessage::continue_unchanged(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrafficRule {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub matcher: RuleMatcher,
    pub action: RuleAction,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TrafficProvenance {
    #[default]
    Upstream,
    Mock,
    Blocked,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedRule {
    pub rule_id: String,
    pub name: String,
    pub action: String,
    pub stage: String,
}
impl TrafficRule {
    fn validate(&mut self) -> Result<()> {
        if self.name.trim().is_empty()
            || self.name.len() > 160
            || self.name.chars().any(char::is_control)
        {
            bail!("Rule name must contain 1–160 bytes without control characters");
        }
        self.name = self.name.trim().to_owned();
        if !(-1_000_000..=1_000_000).contains(&self.priority) {
            bail!("Rule priority must be between -1000000 and 1000000");
        }
        if let Some(method) = &mut self.matcher.method {
            *method = method.to_ascii_uppercase();
            method.parse::<hudsucker::hyper::Method>()?;
            if method == "CONNECT" {
                bail!("CONNECT tunnels cannot be matched by HTTP rules");
            }
        }
        if let Some(host) = &mut self.matcher.host {
            if host.is_empty() || host.len() > 253 || host.contains(['/', ':', '@', '*', '?', '#'])
            {
                bail!("Rule host must be an exact hostname without scheme, port or wildcards");
            }
            *host = url::Host::parse(host)
                .context("Invalid rule hostname")?
                .to_string()
                .to_ascii_lowercase();
        }
        if let Some(path) = &self.matcher.path_prefix {
            if !path.starts_with('/') || path.len() > 2048 || path.chars().any(char::is_control) {
                bail!("Path prefix must start with / and contain at most 2048 bytes");
            }
        }
        if let Some(part) = &self.matcher.url_contains {
            if part.is_empty() || part.len() > 2048 || part.chars().any(char::is_control) {
                bail!("URL substring must contain 1–2048 bytes without control characters");
            }
        }
        if let RuleAction::RewriteRequest {
            method: Some(method),
            ..
        } = &mut self.action
        {
            *method = method.to_ascii_uppercase();
        }
        let edit = self.action.edit();
        edit.validate()?;
        if edit.method.as_deref() == Some("CONNECT") {
            bail!("Rules cannot rewrite to CONNECT");
        }
        if edit.body_base64.is_some() && edit.body.is_none() {
            bail!("bodyBase64 requires body");
        }
        if let Some(headers) = &edit.headers {
            if headers.len() > 128
                || headers
                    .iter()
                    .any(|h| h.name.len() > 256 || h.value.len() > 16 * 1024)
            {
                bail!("Rules support at most 128 headers, 256 bytes per name and 16 KiB per value");
            }
        }
        if let Some(body) = &edit.body {
            if body.len() > MAX_RULE_BODY_BYTES * 2
                || decode_body(body, edit.body_base64.unwrap_or(false))?.len() > MAX_RULE_BODY_BYTES
            {
                bail!("Rule body exceeds 1 MiB");
            }
        }
        if let Some(status) = edit.status {
            if !(200..=599).contains(&status) {
                bail!("Rule response status must be a final HTTP status between 200 and 599");
            }
        }
        match &self.action {
            RuleAction::RewriteRequest {
                method,
                url,
                headers,
                body,
                ..
            } if method.is_none() && url.is_none() && headers.is_none() && body.is_none() => {
                bail!("Request rewrite must change at least one field")
            }
            RuleAction::RewriteResponse {
                status,
                headers,
                body,
                ..
            } if status.is_none() && headers.is_none() && body.is_none() => {
                bail!("Response rewrite must change at least one field")
            }
            RuleAction::Block {
                reason: Some(reason),
            } if reason.len() > 512 || reason.chars().any(char::is_control) => {
                bail!("Block reason must be at most 512 bytes without control characters")
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn matches(&self, method: &str, url: &str) -> bool {
        if !self.enabled {
            return false;
        }
        let Ok(parsed) = url::Url::parse(url) else {
            return false;
        };
        self.matcher.method.as_ref().is_none_or(|m| m == method)
            && self.matcher.host.as_ref().is_none_or(|h| {
                parsed
                    .host_str()
                    .is_some_and(|host| host.eq_ignore_ascii_case(h))
            })
            && self
                .matcher
                .path_prefix
                .as_ref()
                .is_none_or(|path| parsed.path().starts_with(path))
            && self
                .matcher
                .url_contains
                .as_ref()
                .is_none_or(|part| url.contains(part))
    }
    pub(crate) fn evidence(&self, stage: &str) -> AppliedRule {
        AppliedRule {
            rule_id: self.id.clone(),
            name: self.name.clone(),
            action: self.action.kind().into(),
            stage: stage.into(),
        }
    }
}
pub(crate) fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS traffic_rules(id TEXT PRIMARY KEY,priority INTEGER NOT NULL,data TEXT NOT NULL);")?;
    Ok(())
}
pub(crate) fn load(db: &Connection) -> Result<Vec<Arc<TrafficRule>>> {
    let mut statement = db.prepare("SELECT data FROM traffic_rules ORDER BY priority,id")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    let mut rules = Vec::new();
    let mut bytes = 0usize;
    for row in rows {
        let data = row?;
        bytes = bytes.saturating_add(data.len());
        if bytes > MAX_RULE_CONFIG_BYTES || rules.len() >= MAX_RULES {
            bail!("Persisted traffic rules exceed configuration limits");
        }
        let mut rule: TrafficRule =
            serde_json::from_str(&data).context("Invalid persisted traffic rule")?;
        rule.validate().context("Invalid persisted traffic rule")?;
        rules.push(Arc::new(rule));
    }
    Ok(rules)
}
impl Engine {
    pub fn list_rules(&self) -> Result<Vec<TrafficRule>> {
        Ok(self
            .inner
            .rules
            .read()
            .unwrap()
            .iter()
            .map(|rule| rule.as_ref().clone())
            .collect())
    }
    /// Whether the incoming request's rule snapshot would pause either stage.
    /// This is read-only and uses the same enabled/matcher logic as capture.
    pub fn has_pause_rule_for(&self, method: &str, url: &str) -> bool {
        self.matching_rules(method, url)
            .iter()
            .any(|rule| matches!(rule.action, RuleAction::Pause { .. }))
    }
    pub(crate) fn matching_rules(&self, method: &str, url: &str) -> Vec<Arc<TrafficRule>> {
        self.inner
            .rules
            .read()
            .unwrap()
            .iter()
            .filter(|rule| rule.matches(method, url))
            .cloned()
            .collect()
    }
    pub fn save_rule(&self, mut rule: TrafficRule) -> Result<TrafficRule> {
        rule.validate()?;
        if rule.id.is_empty() {
            rule.id = uid();
        }
        Uuid::parse_str(&rule.id).context("Rule ID must be a UUID or empty for a new rule")?;
        let mut db = self.inner.db.lock().unwrap();
        let tx = db.transaction()?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM traffic_rules WHERE id=?1)",
            [&rule.id],
            |row| row.get(0),
        )?;
        let count: i64 =
            tx.query_row("SELECT COUNT(*) FROM traffic_rules", [], |row| row.get(0))?;
        if !exists && count >= MAX_RULES as i64 {
            bail!("At most 256 traffic rules can be stored");
        }
        let data = serde_json::to_string(&rule)?;
        let stored_bytes: i64 = tx.query_row(
            "SELECT COALESCE(SUM(length(CAST(data AS BLOB))),0) FROM traffic_rules WHERE id != ?1",
            [&rule.id],
            |row| row.get(0),
        )?;
        if stored_bytes.saturating_add(data.len() as i64) > MAX_RULE_CONFIG_BYTES as i64 {
            bail!("Traffic rule configuration exceeds 8 MiB");
        }
        tx.execute("INSERT INTO traffic_rules(id,priority,data) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET priority=excluded.priority,data=excluded.data", params![rule.id,rule.priority,data])?;
        sessions::audit(
            &tx,
            if exists { "rule.update" } else { "rule.create" },
            Some(&rule.id),
        )?;
        let snapshot = load(&tx)?;
        tx.commit()?;
        *self.inner.rules.write().unwrap() = snapshot;
        Ok(rule)
    }
    pub fn delete_rule(&self, id: &str) -> Result<()> {
        let mut db = self.inner.db.lock().unwrap();
        let tx = db.transaction()?;
        if tx.execute("DELETE FROM traffic_rules WHERE id=?1", [id])? == 0 {
            bail!("Rule not found");
        }
        sessions::audit(&tx, "rule.delete", Some(id))?;
        let snapshot = load(&tx)?;
        tx.commit()?;
        *self.inner.rules.write().unwrap() = snapshot;
        Ok(())
    }
}
