//! User-configured HTTP API feeds — pull external site info into Companion.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiFeed {
    pub id: u64,
    pub name: String,
    pub url: String,
    /// Optional `Authorization` value, or `Header-Name: value`.
    #[serde(default)]
    pub auth: String,
    /// Optional dotted JSON path (e.g. `data.price` or `0.temp`) for a short summary.
    #[serde(default)]
    pub json_path: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub last_status: String,
    #[serde(default)]
    pub last_summary: String,
    #[serde(default)]
    pub last_preview: String,
    #[serde(default)]
    pub last_pulled_ms: u64,
}

fn default_true() -> bool {
    true
}

impl ApiFeed {
    pub fn new(id: u64, name: String, url: String) -> Self {
        Self {
            id,
            name,
            url,
            auth: String::new(),
            json_path: String::new(),
            enabled: true,
            last_status: String::new(),
            last_summary: String::new(),
            last_preview: String::new(),
            last_pulled_ms: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiPullOutcome {
    pub id: u64,
    pub ok: bool,
    pub status: String,
    pub summary: String,
    pub preview: String,
    pub pulled_ms: u64,
}

const UA: &str = "Njordr-seas-CYD-miner/0.8.39";
const MAX_PREVIEW: usize = 3500;

/// GET `url` and return status + preview (+ optional JSON-path summary).
pub fn pull_feed(feed: &ApiFeed) -> ApiPullOutcome {
    let url = feed.url.trim();
    let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return ApiPullOutcome {
            id: feed.id,
            ok: false,
            status: "URL must start with http:// or https://".into(),
            summary: String::new(),
            preview: String::new(),
            pulled_ms: now_ms,
        };
    }

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(25))
        .user_agent(UA)
        .build();

    let mut req = agent.get(url);
    let auth = feed.auth.trim();
    if !auth.is_empty() {
        if let Some((k, v)) = auth.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            if !key.is_empty() {
                req = req.set(key, val);
            } else {
                req = req.set("Authorization", auth);
            }
        } else {
            req = req.set("Authorization", auth);
        }
    }

    match req.call() {
        Ok(resp) => {
            let code = resp.status();
            let body = resp.into_string().unwrap_or_default();
            let preview = truncate(&body, MAX_PREVIEW);
            let summary = extract_summary(&body, &feed.json_path);
            let ok = (200..300).contains(&code);
            ApiPullOutcome {
                id: feed.id,
                ok,
                status: if ok {
                    format!("HTTP {code} OK")
                } else {
                    format!("HTTP {code}")
                },
                summary,
                preview,
                pulled_ms: now_ms,
            }
        }
        Err(e) => ApiPullOutcome {
            id: feed.id,
            ok: false,
            status: format!("Error: {e}"),
            summary: String::new(),
            preview: String::new(),
            pulled_ms: now_ms,
        },
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

fn extract_summary(body: &str, path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        // Prefer a one-line headline from JSON if possible.
        if let Ok(v) = serde_json::from_str::<Value>(body) {
            return json_headline(&v);
        }
        return body.lines().next().unwrap_or("").trim().chars().take(120).collect();
    }
    match serde_json::from_str::<Value>(body) {
        Ok(root) => match walk_json(&root, path) {
            Some(v) => value_brief(&v),
            None => format!("path `{path}` not found"),
        },
        Err(_) => "response is not JSON".into(),
    }
}

fn walk_json<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for part in path.split('.').filter(|p| !p.is_empty()) {
        cur = if let Ok(idx) = part.parse::<usize>() {
            cur.get(idx)?
        } else {
            cur.get(part)?
        };
    }
    Some(cur)
}

fn value_brief(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.chars().take(160).collect(),
        Value::Array(a) => format!("[{} items]", a.len()),
        Value::Object(o) => format!("{{{} keys}}", o.len()),
    }
}

fn json_headline(v: &Value) -> String {
    match v {
        Value::Object(map) => {
            for key in ["message", "status", "title", "name", "price", "result", "data"] {
                if let Some(child) = map.get(key) {
                    let brief = value_brief(child);
                    if !brief.is_empty() {
                        return format!("{key}: {brief}");
                    }
                }
            }
            format!("{{{} keys}}", map.len())
        }
        Value::Array(a) => format!("[{} items]", a.len()),
        other => value_brief(other),
    }
}

/// Load feeds from eframe storage JSON.
pub fn load_feeds(raw: &str) -> Vec<ApiFeed> {
    serde_json::from_str(raw).unwrap_or_default()
}

pub fn save_feeds(feeds: &[ApiFeed]) -> String {
    serde_json::to_string(feeds).unwrap_or_else(|_| "[]".into())
}

pub fn next_feed_id(feeds: &[ApiFeed]) -> u64 {
    feeds.iter().map(|f| f.id).max().unwrap_or(0).saturating_add(1)
}
