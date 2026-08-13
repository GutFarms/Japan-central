//! User-configured HTTP API feeds — pull JSON / text / CSV / files from one or more sources.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How to interpret a pulled response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApiContentKind {
    /// Sniff Content-Type + URL extension, then body.
    #[default]
    Auto,
    Json,
    Text,
    Csv,
    /// Save response body as a file under ApiDownloads/.
    File,
}

impl ApiContentKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Json => "JSON",
            Self::Text => "Text",
            Self::Csv => "CSV",
            Self::File => "File",
        }
    }

    pub fn all() -> &'static [ApiContentKind] {
        &[
            Self::Auto,
            Self::Json,
            Self::Text,
            Self::Csv,
            Self::File,
        ]
    }
}

/// One HTTP endpoint inside a feed (feeds may pull several file types from different sources).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSource {
    pub url: String,
    #[serde(default)]
    pub kind: ApiContentKind,
    /// JSON dotted path, CSV column name/index, or save-as filename for File.
    #[serde(default)]
    pub path: String,
}

impl ApiSource {
    pub fn new(url: String, kind: ApiContentKind, path: String) -> Self {
        Self { url, kind, path }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiFeed {
    pub id: u64,
    pub name: String,
    /// Primary source URL (kept for older persisted feeds).
    pub url: String,
    /// Optional `Authorization` value, or `Header-Name: value`.
    #[serde(default)]
    pub auth: String,
    /// Optional path for the primary source (JSON path / CSV column / save-as).
    #[serde(default)]
    pub json_path: String,
    /// Content kind for the primary source.
    #[serde(default)]
    pub kind: ApiContentKind,
    /// Extra sources (other URLs / file types) pulled with this feed.
    #[serde(default)]
    pub sources: Vec<ApiSource>,
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
    /// Last saved file path(s), if any File sources succeeded.
    #[serde(default)]
    pub last_saved: String,
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
            kind: ApiContentKind::Auto,
            sources: Vec::new(),
            enabled: true,
            last_status: String::new(),
            last_summary: String::new(),
            last_preview: String::new(),
            last_pulled_ms: 0,
            last_saved: String::new(),
        }
    }

    /// Primary + extra sources in pull order.
    pub fn all_sources(&self) -> Vec<ApiSource> {
        let mut out = vec![ApiSource {
            url: self.url.clone(),
            kind: self.kind,
            path: self.json_path.clone(),
        }];
        out.extend(self.sources.iter().cloned());
        out
    }
}

#[derive(Debug, Clone)]
pub struct ApiPullOutcome {
    pub id: u64,
    pub ok: bool,
    pub status: String,
    pub summary: String,
    pub preview: String,
    pub saved: String,
    pub pulled_ms: u64,
}

const UA: &str = "Njordr-seas-CYD-miner/0.8.76";
const MAX_PREVIEW: usize = 3500;
const MAX_FILE_BYTES: usize = 32 * 1024 * 1024;

/// Directory next to the Companion exe (or CWD) for File-kind downloads.
pub fn api_downloads_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return parent.join("ApiDownloads");
        }
    }
    PathBuf::from("ApiDownloads")
}

/// GET every source on the feed; aggregate status / summary / saved files.
pub fn pull_feed(feed: &ApiFeed) -> ApiPullOutcome {
    let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let sources = feed.all_sources();
    if sources.is_empty() {
        return ApiPullOutcome {
            id: feed.id,
            ok: false,
            status: "No sources configured".into(),
            summary: String::new(),
            preview: String::new(),
            saved: String::new(),
            pulled_ms: now_ms,
        };
    }

    let mut ok_n = 0usize;
    let mut fail_n = 0usize;
    let mut summaries: Vec<String> = Vec::new();
    let mut previews: Vec<String> = Vec::new();
    let mut saved: Vec<String> = Vec::new();
    let mut statuses: Vec<String> = Vec::new();

    for (i, src) in sources.iter().enumerate() {
        let label = if i == 0 {
            "primary".to_string()
        } else {
            format!("src{}", i)
        };
        let one = pull_source(feed, src, &label);
        if one.ok {
            ok_n += 1;
        } else {
            fail_n += 1;
        }
        statuses.push(format!("{label}: {}", one.status));
        if !one.summary.is_empty() {
            summaries.push(if sources.len() > 1 {
                format!("{label} {}", one.summary)
            } else {
                one.summary
            });
        }
        if !one.preview.is_empty() && previews.len() < 2 {
            previews.push(one.preview);
        }
        if !one.saved.is_empty() {
            saved.push(one.saved);
        }
    }

    let ok = fail_n == 0 && ok_n > 0;
    let status = if sources.len() == 1 {
        statuses.into_iter().next().unwrap_or_default()
    } else {
        format!("{ok_n} ok · {fail_n} fail · {}", statuses.join(" · "))
    };

    ApiPullOutcome {
        id: feed.id,
        ok,
        status,
        summary: summaries.join(" · "),
        preview: previews.join("\n---\n"),
        saved: saved.join("; "),
        pulled_ms: now_ms,
    }
}

struct SourceOutcome {
    ok: bool,
    status: String,
    summary: String,
    preview: String,
    saved: String,
}

fn pull_source(feed: &ApiFeed, src: &ApiSource, label: &str) -> SourceOutcome {
    let url = src.url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return SourceOutcome {
            ok: false,
            status: "URL must start with http:// or https://".into(),
            summary: String::new(),
            preview: String::new(),
            saved: String::new(),
        };
    }

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(60))
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
            let content_type = resp
                .header("Content-Type")
                .unwrap_or("")
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            let ok_http = (200..300).contains(&code);
            let kind = resolve_kind(src.kind, &content_type, url);

            let mut bytes = Vec::new();
            let mut reader = resp.into_reader();
            let mut buf = [0u8; 64 * 1024];
            let mut truncated = false;
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if bytes.len() + n > MAX_FILE_BYTES {
                            bytes.extend_from_slice(&buf[..MAX_FILE_BYTES.saturating_sub(bytes.len())]);
                            truncated = true;
                            break;
                        }
                        bytes.extend_from_slice(&buf[..n]);
                    }
                    Err(_) => break,
                }
            }

            if !ok_http {
                let body = String::from_utf8_lossy(&bytes);
                return SourceOutcome {
                    ok: false,
                    status: format!("HTTP {code}"),
                    summary: String::new(),
                    preview: truncate(body.trim(), 400),
                    saved: String::new(),
                };
            }

            match kind {
                ApiContentKind::File => {
                    match save_file_bytes(feed, src, label, url, &content_type, &bytes) {
                        Ok(path) => SourceOutcome {
                            ok: true,
                            status: format!(
                                "HTTP {code} · saved {}{}B",
                                if truncated { "partial " } else { "" },
                                bytes.len()
                            ),
                            summary: format!("file {}", path.display()),
                            preview: format!("Saved {}", path.display()),
                            saved: path.display().to_string(),
                        },
                        Err(e) => SourceOutcome {
                            ok: false,
                            status: format!("HTTP {code} · save failed: {e}"),
                            summary: String::new(),
                            preview: String::new(),
                            saved: String::new(),
                        },
                    }
                }
                ApiContentKind::Json => {
                    let body = String::from_utf8_lossy(&bytes);
                    let summary = extract_json_summary(&body, &src.path);
                    SourceOutcome {
                        ok: true,
                        status: format!("HTTP {code} · JSON"),
                        summary,
                        preview: truncate(&body, MAX_PREVIEW),
                        saved: String::new(),
                    }
                }
                ApiContentKind::Csv => {
                    let body = String::from_utf8_lossy(&bytes);
                    let summary = extract_csv_summary(&body, &src.path);
                    SourceOutcome {
                        ok: true,
                        status: format!("HTTP {code} · CSV"),
                        summary,
                        preview: truncate(&body, MAX_PREVIEW),
                        saved: String::new(),
                    }
                }
                ApiContentKind::Text | ApiContentKind::Auto => {
                    // Auto that didn't resolve to json/csv/file lands here as text.
                    let body = String::from_utf8_lossy(&bytes);
                    if kind == ApiContentKind::Auto
                        || (src.kind == ApiContentKind::Auto && looks_like_json(&body))
                    {
                        if looks_like_json(&body) {
                            let summary = extract_json_summary(&body, &src.path);
                            return SourceOutcome {
                                ok: true,
                                status: format!("HTTP {code} · JSON"),
                                summary,
                                preview: truncate(&body, MAX_PREVIEW),
                                saved: String::new(),
                            };
                        }
                        if looks_like_csv(&body) {
                            let summary = extract_csv_summary(&body, &src.path);
                            return SourceOutcome {
                                ok: true,
                                status: format!("HTTP {code} · CSV"),
                                summary,
                                preview: truncate(&body, MAX_PREVIEW),
                                saved: String::new(),
                            };
                        }
                    }
                    let summary = body
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .chars()
                        .take(120)
                        .collect();
                    SourceOutcome {
                        ok: true,
                        status: format!("HTTP {code} · text"),
                        summary,
                        preview: truncate(&body, MAX_PREVIEW),
                        saved: String::new(),
                    }
                }
            }
        }
        Err(e) => SourceOutcome {
            ok: false,
            status: format!("Error: {e}"),
            summary: String::new(),
            preview: String::new(),
            saved: String::new(),
        },
    }
}

fn resolve_kind(requested: ApiContentKind, content_type: &str, url: &str) -> ApiContentKind {
    if requested != ApiContentKind::Auto {
        return requested;
    }
    if content_type.contains("json") {
        return ApiContentKind::Json;
    }
    if content_type.contains("csv") || content_type == "text/comma-separated-values" {
        return ApiContentKind::Csv;
    }
    if content_type.starts_with("text/") {
        return ApiContentKind::Text;
    }
    if content_type.starts_with("image/")
        || content_type.starts_with("application/octet-stream")
        || content_type.starts_with("application/zip")
        || content_type.starts_with("application/pdf")
        || content_type.starts_with("application/gzip")
        || content_type.contains("binary")
    {
        return ApiContentKind::File;
    }
    let path = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    if path.ends_with(".json") {
        return ApiContentKind::Json;
    }
    if path.ends_with(".csv") {
        return ApiContentKind::Csv;
    }
    if path.ends_with(".txt")
        || path.ends_with(".md")
        || path.ends_with(".log")
        || path.ends_with(".xml")
        || path.ends_with(".html")
        || path.ends_with(".htm")
    {
        return ApiContentKind::Text;
    }
    if path.ends_with(".bin")
        || path.ends_with(".zip")
        || path.ends_with(".exe")
        || path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".gif")
        || path.ends_with(".webp")
        || path.ends_with(".pdf")
        || path.ends_with(".gz")
    {
        return ApiContentKind::File;
    }
    ApiContentKind::Text
}

fn save_file_bytes(
    feed: &ApiFeed,
    src: &ApiSource,
    label: &str,
    url: &str,
    content_type: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    let dir = api_downloads_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    let name = if !src.path.trim().is_empty() {
        sanitize_filename(src.path.trim())
    } else {
        filename_from_url(url, content_type, &feed.name, label)
    };
    let dest = unique_path(&dir, &name);
    std::fs::write(&dest, bytes).map_err(|e| format!("write {}: {e}", dest.display()))?;
    Ok(dest)
}

fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    for n in 2..1000 {
        let p = dir.join(format!("{stem}-{n}{ext}"));
        if !p.exists() {
            return p;
        }
    }
    dir.join(format!("{stem}-dup{ext}"))
}

fn filename_from_url(url: &str, content_type: &str, feed_name: &str, label: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);
    if let Some(file) = path.rsplit('/').next() {
        let file = file.trim();
        if !file.is_empty() && file.contains('.') && file.len() < 120 {
            return sanitize_filename(file);
        }
    }
    let ext = extension_for_content_type(content_type);
    sanitize_filename(&format!(
        "{}-{}{}",
        feed_name.replace(' ', "_"),
        label,
        ext
    ))
}

fn extension_for_content_type(ct: &str) -> &'static str {
    if ct.contains("json") {
        ".json"
    } else if ct.contains("csv") {
        ".csv"
    } else if ct.contains("png") {
        ".png"
    } else if ct.contains("jpeg") || ct.contains("jpg") {
        ".jpg"
    } else if ct.contains("gif") {
        ".gif"
    } else if ct.contains("webp") {
        ".webp"
    } else if ct.contains("pdf") {
        ".pdf"
    } else if ct.contains("zip") {
        ".zip"
    } else if ct.starts_with("text/") {
        ".txt"
    } else {
        ".bin"
    }
}

fn sanitize_filename(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(120)
        .collect();
    if s.is_empty() || s == "." || s == ".." {
        "download.bin".into()
    } else {
        s
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

fn looks_like_json(body: &str) -> bool {
    let t = body.trim_start();
    (t.starts_with('{') || t.starts_with('[')) && serde_json::from_str::<Value>(t).is_ok()
}

fn looks_like_csv(body: &str) -> bool {
    let mut lines = body.lines().filter(|l| !l.trim().is_empty());
    let Some(h) = lines.next() else {
        return false;
    };
    h.contains(',') && lines.next().is_some_and(|l| l.contains(','))
}

fn extract_json_summary(body: &str, path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        if let Ok(v) = serde_json::from_str::<Value>(body) {
            return json_headline(&v);
        }
        return body
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .chars()
            .take(120)
            .collect();
    }
    match serde_json::from_str::<Value>(body) {
        Ok(root) => match walk_json(&root, path) {
            Some(v) => value_brief(&v),
            None => format!("path `{path}` not found"),
        },
        Err(_) => "response is not JSON".into(),
    }
}

fn extract_csv_summary(body: &str, path: &str) -> String {
    let mut lines = body.lines().filter(|l| !l.trim().is_empty());
    let Some(header) = lines.next() else {
        return "empty CSV".into();
    };
    let headers: Vec<&str> = split_csv_line(header);
    let Some(row) = lines.next() else {
        return format!("{} columns", headers.len());
    };
    let cols: Vec<&str> = split_csv_line(row);
    let path = path.trim();
    if path.is_empty() {
        let preview: Vec<String> = headers
            .iter()
            .zip(cols.iter())
            .take(3)
            .map(|(h, c)| format!("{h}={c}"))
            .collect();
        return if preview.is_empty() {
            format!("{} cols", headers.len())
        } else {
            preview.join(", ")
        };
    }
    let idx = if let Ok(i) = path.parse::<usize>() {
        i
    } else {
        headers
            .iter()
            .position(|h| h.eq_ignore_ascii_case(path))
            .unwrap_or(usize::MAX)
    };
    cols.get(idx)
        .map(|c| (*c).chars().take(160).collect())
        .unwrap_or_else(|| format!("column `{path}` not found"))
}

fn split_csv_line(line: &str) -> Vec<&str> {
    // Lightweight split — good enough for simple feeds (no escaped-comma parser).
    line.split(',').map(|s| s.trim().trim_matches('"')).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_kinds_from_url_and_type() {
        assert_eq!(
            resolve_kind(ApiContentKind::Auto, "application/json", "https://x/a"),
            ApiContentKind::Json
        );
        assert_eq!(
            resolve_kind(ApiContentKind::Auto, "text/csv", "https://x/a"),
            ApiContentKind::Csv
        );
        assert_eq!(
            resolve_kind(ApiContentKind::Auto, "application/octet-stream", "https://x/a.bin"),
            ApiContentKind::File
        );
        assert_eq!(
            resolve_kind(ApiContentKind::Auto, "", "https://cdn/x/firmware.bin"),
            ApiContentKind::File
        );
        assert_eq!(
            resolve_kind(ApiContentKind::Json, "text/plain", "https://x/a.bin"),
            ApiContentKind::Json
        );
    }

    #[test]
    fn csv_column_summary() {
        let body = "temp,humidity\n21.5,40\n22,41\n";
        assert_eq!(extract_csv_summary(body, "temp"), "21.5");
        assert_eq!(extract_csv_summary(body, "1"), "40");
        assert!(extract_csv_summary(body, "").contains("temp="));
    }

    #[test]
    fn feed_all_sources_includes_extras() {
        let mut f = ApiFeed::new(1, "kit".into(), "https://a/info.json".into());
        f.kind = ApiContentKind::Json;
        f.sources.push(ApiSource::new(
            "https://a/data.csv".into(),
            ApiContentKind::Csv,
            "price".into(),
        ));
        f.sources.push(ApiSource::new(
            "https://a/blob.bin".into(),
            ApiContentKind::File,
            "blob.bin".into(),
        ));
        assert_eq!(f.all_sources().len(), 3);
    }
}
