//! In-app Assist — **built-in Local AI** by default (stratum/hashrate watch + tools).
//! Optional backends: Ollama on this PC, or an OpenAI-compatible cloud API.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_MODEL: &str = "gpt-4o-mini";
pub const OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434/v1";
pub const OLLAMA_DEFAULT_MODEL: &str = "llama3.2";
const MAX_TOOL_ROUNDS: u8 = 5;

/// Where Assist runs inference. Default = built into Companion (no network LLM).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AssistBackend {
    /// On-device playbook + NLU — ships inside Companion, works offline.
    #[default]
    BuiltIn,
    /// Optional: local Ollama on this PC (`127.0.0.1:11434`).
    Ollama,
    /// Optional: OpenAI-compatible cloud / remote endpoint.
    Cloud,
}

impl AssistBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BuiltIn => "built_in",
            Self::Ollama => "ollama",
            Self::Cloud => "cloud",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "ollama" | "local_ollama" => Self::Ollama,
            "cloud" | "openai" | "api" => Self::Cloud,
            _ => Self::BuiltIn,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::BuiltIn => "Built-in (local)",
            Self::Ollama => "Ollama (this PC)",
            Self::Cloud => "Cloud API",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AssistRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssistToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssistMessage {
    pub role: AssistRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<AssistToolCall>,
}

impl AssistMessage {
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: AssistRole::System,
            content: text.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: AssistRole::User,
            content: text.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: AssistRole::Assistant,
            content: text.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }
    pub fn tool(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            role: AssistRole::Tool,
            content: text.into(),
            tool_call_id: Some(id.into()),
            tool_calls: Vec::new(),
        }
    }
}

/// Live snapshot passed into the model / local helper (read-only).
#[derive(Clone, Debug, Serialize)]
pub struct AssistSnapshot {
    pub companion_version: String,
    pub usb_open: bool,
    pub mining: bool,
    pub com_port: String,
    pub stratum: String,
    pub worker: String,
    pub target_mhz: u8,
    pub fw: String,
    pub board_mac: String,
    pub hashrate_khs: f64,
    pub hashrate_hs: f64,
    /// Pool-inferred H/s from accepted shares (0 if unknown).
    pub pool_estimated_hs: f64,
    pub accepted: u32,
    pub rejected: u32,
    pub session_accepted: u32,
    pub session_rejected: u32,
    pub accept_rate_pct: f64,
    pub expected_shares_per_hour: f64,
    pub pool_phase: String,
    pub stratum_phase: String,
    pub stratum_connected: bool,
    pub stratum_authorized: bool,
    pub stratum_difficulty: f64,
    pub stratum_jobs: u32,
    pub stratum_submits: u64,
    pub stratum_last_job: String,
    pub stratum_last_error: String,
    pub linked_boards: u32,
    pub boards_below_target_khs: u32,
    /// Rough healthy floor per board (kH/s) used by the watch loop.
    pub target_khs_per_board: f64,
    pub bench_busy: bool,
    /// Rolling median kH/s (0 if not enough samples yet).
    pub baseline_khs: f64,
    /// True when current rate fell sharply vs baseline.
    pub rate_cliff: bool,
    pub reject_streak: u32,
    /// Authorized but jobs not advancing while rate is soft.
    pub jobs_stalled: bool,
    pub ports: Vec<String>,
    pub linked: Vec<String>,
    pub discovered: Vec<String>,
    pub flash_busy: bool,
    pub last_ok: String,
    pub last_error: String,
    pub recent_logs: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum AssistAction {
    GetFleetStatus,
    /// Focused stratum + hashrate health report (same data, mining-first summary).
    WatchStratum,
    /// Apply safe max-hashrate steps: clock 240 → start mine if idle → bench if slow.
    OptimizeHashrate,
    ListPorts,
    ScanWorkers,
    ConnectBoard { endpoint: Option<String> },
    DisconnectBoard { endpoint: Option<String> },
    SetPoolConfig {
        stratum: Option<String>,
        worker: Option<String>,
        password: Option<String>,
    },
    StartMining,
    StopMining,
    SetClock { mhz: u8 },
    BenchBoards,
    ConfigureBoardWifi {
        endpoint: Option<String>,
        ssid: String,
        password: String,
    },
    GetEventLog { limit: usize },
    /// High-risk — UI confirms before running.
    UpdateBoardFirmware { live_push: bool, wifi: bool },
    SwitchTab { tab: String },
}

/// One automatic watch-loop recommendation.
#[derive(Clone, Debug)]
pub struct WatchStep {
    pub action: AssistAction,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct WatchReport {
    pub headline: String,
    pub detail: String,
    pub steps: Vec<WatchStep>,
    /// Stable signature so the UI can avoid spamming identical notes.
    pub signature: String,
    /// Human-readable anomaly tags for optional LLM escalation.
    pub anomalies: Vec<String>,
}

/// One remesure outcome — persisted so Built-in Local AI can bias the next watch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssistFixRecord {
    pub label: String,
    pub action_kind: String,
    pub before_khs: f64,
    pub after_khs: f64,
    pub delta_khs: f64,
    pub accept_pct_before: f64,
    pub accept_pct_after: f64,
    pub board_mac: String,
    pub ts_unix: u64,
    /// User vote: 1 helped, -1 worse, 0 unknown.
    #[serde(default)]
    pub user_vote: i8,
}

impl AssistFixRecord {
    pub fn helped(&self) -> bool {
        self.user_vote > 0
            || (self.user_vote == 0
                && self.after_khs > self.before_khs * 1.05
                && self.after_khs + 2.0 >= self.before_khs)
    }

    pub fn worsened(&self) -> bool {
        self.user_vote < 0
            || (self.user_vote == 0 && self.after_khs + 2.0 < self.before_khs * 0.9)
    }
}

/// Rolling memory of Assist fixes (persisted in mine_prefs).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AssistMemory {
    #[serde(default)]
    pub fixes: Vec<AssistFixRecord>,
    /// Last healthy per-board floor learned from baselines (kH/s).
    #[serde(default)]
    pub learned_floor_khs: f64,
    /// Last stratum URL that cleared hard-diff / auth after a fix (optional).
    #[serde(default)]
    pub preferred_stratum: String,
}

impl AssistMemory {
    pub fn push_fix(&mut self, rec: AssistFixRecord) {
        self.fixes.push(rec);
        while self.fixes.len() > 40 {
            self.fixes.remove(0);
        }
        // Refresh learned floor from successful remesures.
        let good: Vec<f64> = self
            .fixes
            .iter()
            .filter(|f| f.helped() && f.after_khs > 20.0)
            .map(|f| f.after_khs)
            .collect();
        if good.len() >= 2 {
            let med = median_f64(&good);
            // Soft floor = 75% of healthy median, clamped.
            self.learned_floor_khs = (med * 0.75).clamp(40.0, 400.0);
        }
    }

    /// True when this action kind recently worsened rate for this board (or any).
    pub fn recently_failed(&self, action_kind: &str, board_mac: &str) -> bool {
        let mac = board_mac.trim().to_ascii_lowercase();
        let mut fails = 0u32;
        for f in self.fixes.iter().rev().take(12) {
            if !f.action_kind.eq_ignore_ascii_case(action_kind) {
                continue;
            }
            if !mac.is_empty()
                && !f.board_mac.is_empty()
                && !f.board_mac.eq_ignore_ascii_case(&mac)
            {
                continue;
            }
            if f.worsened() {
                fails += 1;
            }
            if fails >= 2 {
                return true;
            }
        }
        false
    }

    pub fn memory_prompt_line(&self) -> String {
        if self.fixes.is_empty() && self.learned_floor_khs <= 0.0 {
            return String::new();
        }
        let last = self
            .fixes
            .iter()
            .rev()
            .take(5)
            .map(|f| {
                format!(
                    "{} {:+.0}kH vote={}",
                    f.action_kind, f.delta_khs, f.user_vote
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        format!(
            "MEMORY: floor≈{:.0} kH/s/board · recent fixes: {}",
            self.learned_floor_khs, last
        )
    }
}

/// Effective soft floor: learned when available, else default 80.
pub fn assist_target_floor_khs(memory: &AssistMemory, baseline_khs: f64, linked: u32) -> f64 {
    let mut floor = if memory.learned_floor_khs >= 40.0 {
        memory.learned_floor_khs
    } else {
        80.0
    };
    // When baseline is known, don't demand more than ~85% of recent healthy fleet rate / boards.
    if baseline_khs > 30.0 && linked > 0 {
        let per = baseline_khs / linked as f64;
        floor = floor.min((per * 0.85).max(40.0));
    }
    floor.clamp(40.0, 400.0)
}

/// Median of a small f64 slice (empty → 0).
pub fn median_f64(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = v.len() / 2;
    if v.len() % 2 == 0 {
        (v[mid - 1] + v[mid]) / 2.0
    } else {
        v[mid]
    }
}

/// Detect mining anomalies from live snapshot (used for event-driven LLM escalate).
pub fn collect_anomalies(snap: &AssistSnapshot) -> Vec<String> {
    let mut out = Vec::new();
    if snap.mining
        && (snap.stratum_phase.eq_ignore_ascii_case("auth-fail")
            || !snap.stratum_last_error.is_empty())
        && !snap.stratum_authorized
    {
        out.push(format!(
            "stratum_auth_fail: {}",
            if snap.stratum_last_error.is_empty() {
                snap.stratum_phase.clone()
            } else {
                trunc(&snap.stratum_last_error, 80)
            }
        ));
    }
    if snap.mining && !snap.stratum_connected && !snap.stratum_authorized {
        out.push("stratum_disconnected".into());
    }
    if snap.rate_cliff && snap.baseline_khs > 20.0 {
        out.push(format!(
            "rate_cliff: {:.0}→{:.0} kH/s (baseline)",
            snap.baseline_khs, snap.hashrate_khs
        ));
    }
    if snap.jobs_stalled {
        out.push("jobs_stalled_authorized_soft_hashrate".into());
    }
    if snap.reject_streak >= 3 {
        out.push(format!("reject_streak:{}", snap.reject_streak));
    }
    if snap.stratum_authorized
        && snap.stratum_difficulty >= 0.05
        && snap.hashrate_hs > 50_000.0
        && snap.expected_shares_per_hour < 5.0
    {
        out.push(format!(
            "hard_share_diff:{:.3}_for_{:.0}_khs",
            snap.stratum_difficulty, snap.hashrate_khs
        ));
    }
    out
}

pub fn anomaly_system_addon(anomalies: &[String]) -> String {
    format!(
        "ANOMALY ESCALATION — continuous watch already runs local safe fixes. \
You see: [{}]. Call watch_stratum once, then at most one of: optimize_hashrate, \
set_pool_config (ESP port like stratum+tcp://btc.hmpool.io:3337), set_clock 240, \
or bench_boards. Do not flash. Reply in ≤4 short lines with what you did and why.",
        anomalies.join(", ")
    )
}

#[derive(Clone, Debug)]
pub struct PendingTool {
    pub id: String,
    pub action: AssistAction,
}

pub fn tool_definitions() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "watch_stratum",
                "description": "PRIMARY: read stratum connection health + hashrate flow (jobs, auth, accepts/rejects, difficulty, board rates) and return a mining-first report.",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "optimize_hashrate",
                "description": "PRIMARY: push hashrate as high as safe — set 240 MHz, start mining if linked+idle, bench boards when rate is soft. Uses live monitoring signals.",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_fleet_status",
                "description": "Full fleet JSON snapshot (USB/Wi-Fi link, mining, pool, hashrate, accepts/rejects, firmware).",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_ports",
                "description": "Refresh and list USB serial COM ports on this PC.",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "scan_workers",
                "description": "Scan USB and LAN for CYD miner boards.",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "connect_board",
                "description": "Link a board over USB COM or Wi-Fi host:19284.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "endpoint": {
                            "type": "string",
                            "description": "COM port (e.g. COM5) or Wi-Fi endpoint (e.g. 192.168.4.1:19284). Empty = selected COM."
                        }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "disconnect_board",
                "description": "Disconnect a linked board (or close primary USB if endpoint omitted).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "endpoint": { "type": "string" }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "set_pool_config",
                "description": "Update Companion pool URL, worker/BTC address, and/or password (saved in prefs).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "stratum": { "type": "string" },
                        "worker": { "type": "string" },
                        "password": { "type": "string" }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "start_mining",
                "description": "Start PC-side stratum and push work to linked boards.",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "stop_mining",
                "description": "Stop mining / disconnect from the pool.",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "set_clock",
                "description": "Set board CPU clock to 80, 160, or 240 MHz. Prefer 240 for max hashrate.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "mhz": { "type": "integer", "enum": [80, 160, 240] }
                    },
                    "required": ["mhz"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "bench_boards",
                "description": "Run D0 HW/HW+/HW-SW retune to lock the fastest SHA path on linked boards.",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "configure_board_wifi",
                "description": "Push home Wi-Fi SSID/password to a linked board (SoftAP→STA).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "endpoint": { "type": "string" },
                        "ssid": { "type": "string" },
                        "password": { "type": "string" }
                    },
                    "required": ["ssid", "password"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_event_log",
                "description": "Return recent Companion event-log lines.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "minimum": 1, "maximum": 40 }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "update_board_firmware",
                "description": "Flash/push board firmware. Requires user confirmation in the UI.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "live_push": {
                            "type": "boolean",
                            "description": "USB auto-reset push without BOOT (default true)."
                        },
                        "wifi": {
                            "type": "boolean",
                            "description": "Wi-Fi OTA push (default false)."
                        }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "switch_tab",
                "description": "Switch the Companion UI tab: mine, setup, settings, or assist.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "tab": { "type": "string", "enum": ["mine", "setup", "settings", "assist"] }
                    },
                    "required": ["tab"]
                }
            }
        }
    ])
}

pub fn system_prompt(snap: &AssistSnapshot) -> String {
    format!(
        "You are the mining watch assistant for Njörðr Seas' CYD miner Companion v{}.\n\
PRIMARY MISSION (always):\n\
1) Continuously reason about stratum connection health (connected → authorized → jobs flowing → accepts).\n\
2) Push board hashrate as high as it will safely go using live monitoring: clock 240 MHz, start mining when linked, bench/retune when rates are soft, restart mining only when the pool link is clearly dead.\n\
3) Prefer tools watch_stratum and optimize_hashrate before other actions.\n\
Rules:\n\
- Never invent hashrates, jobs, or accept counts — read watch_stratum / get_fleet_status.\n\
- If auth-fail: fix worker/BTC address (set_pool_config) — do not spam start/stop.\n\
- If share difficulty is hard for CYD hashrates, recommend an ESP/IoT pool port (e.g. HM Pool :3337).\n\
- Be concise; report stratum phase, fleet kH/s, A/R, and the next action you took.\n\
- Firmware flash needs UI confirmation.\n\
Current snapshot:\n{}",
        snap.companion_version,
        serde_json::to_string_pretty(snap).unwrap_or_else(|_| "{}".into())
    )
}

/// Continuous-monitoring playbook from live telemetry (no LLM required).
pub fn evaluate_mining_watch(snap: &AssistSnapshot, memory: &AssistMemory) -> WatchReport {
    let mut steps = Vec::new();
    let mut notes = Vec::new();
    let boards = snap.linked_boards.max(1);
    let khs = snap.hashrate_khs;
    let per = khs / boards as f64;
    let mem_line = memory.memory_prompt_line();
    if !mem_line.is_empty() {
        notes.push(mem_line);
    }

    if snap.flash_busy {
        return WatchReport {
            headline: "Watch paused — firmware update in progress".into(),
            detail: "Flash owns the COM; mining watch resumes when Update board finishes.".into(),
            steps,
            signature: "flash_busy".into(),
            anomalies: Vec::new(),
        };
    }

    if !snap.usb_open && snap.linked_boards == 0 {
        if !snap.ports.is_empty() {
            steps.push(WatchStep {
                action: AssistAction::ConnectBoard { endpoint: None },
                reason: "No board linked — connect selected COM".into(),
            });
        } else {
            steps.push(WatchStep {
                action: AssistAction::ScanWorkers,
                reason: "No ports/boards — scan for CYDs".into(),
            });
        }
        notes.push("Fleet idle: need a linked board before stratum/hashrate watch can run.".into());
    } else if !snap.mining {
        steps.push(WatchStep {
            action: AssistAction::StartMining,
            reason: "Board linked but mining off — start stratum + job push".into(),
        });
        notes.push("Mining was off; starting pool session.".into());
    } else {
        // Stratum health
        if !snap.stratum_authorized {
            if snap.stratum_phase.eq_ignore_ascii_case("auth-fail")
                || !snap.stratum_last_error.is_empty()
            {
                notes.push(format!(
                    "Stratum AUTH FAIL — fix worker/BTC address (now {}). {}",
                    if snap.worker.is_empty() {
                        "empty"
                    } else {
                        snap.worker.as_str()
                    },
                    if snap.stratum_last_error.is_empty() {
                        String::new()
                    } else {
                        trunc(&snap.stratum_last_error, 100)
                    }
                ));
            } else if !snap.stratum_connected {
                steps.push(WatchStep {
                    action: AssistAction::StartMining,
                    reason: "Stratum not connected while mining — restart pool session".into(),
                });
                notes.push(format!(
                    "Stratum phase={} — reconnecting pool.",
                    if snap.stratum_phase.is_empty() {
                        "…"
                    } else {
                        snap.stratum_phase.as_str()
                    }
                ));
            } else {
                notes.push(format!(
                    "Stratum connected, waiting authorize (phase {}). Jobs={} submits={}.",
                    snap.stratum_phase, snap.stratum_jobs, snap.stratum_submits
                ));
            }
        } else {
            notes.push(format!(
                "Stratum AUTHORIZED · diff={:.4} · jobs={} · submits={} · A={} R={} ({:.0}% ok) · expect {:.2}/h",
                snap.stratum_difficulty,
                snap.stratum_jobs,
                snap.stratum_submits,
                snap.session_accepted,
                snap.session_rejected,
                snap.accept_rate_pct,
                snap.expected_shares_per_hour
            ));
            if snap.stratum_jobs == 0 && snap.stratum_submits == 0 {
                notes.push("Authorized but no jobs yet — waiting for pool notify.".into());
            }
            if snap.stratum_difficulty >= 0.05
                && snap.hashrate_hs > 50_000.0
                && snap.expected_shares_per_hour < 5.0
            {
                notes.push(format!(
                    "Share diff {:.3} is hard for ~{:.0} kH/s — use an ESP/IoT pool port (HM :3337) or wait for vardiff.",
                    snap.stratum_difficulty, snap.hashrate_khs
                ));
            }
            if snap.session_accepted + snap.session_rejected >= 8 && snap.accept_rate_pct < 70.0 {
                notes.push(format!(
                    "Reject pressure high ({:.0}% accepts) — check difficulty / worker / stale jobs.",
                    snap.accept_rate_pct
                ));
            }
        }

        // Hashrate push — prefer measured cliff vs rolling baseline over a fixed floor alone.
        if snap.target_mhz < 240 {
            steps.push(WatchStep {
                action: AssistAction::SetClock { mhz: 240 },
                reason: format!("Clock {} MHz → 240 for max SHA throughput", snap.target_mhz),
            });
        }
        let soft = per < snap.target_khs_per_board || snap.boards_below_target_khs > 0;
        let cliff = snap.rate_cliff && snap.baseline_khs > 20.0;
        let bench_failed = memory.recently_failed("bench", &snap.board_mac)
            || memory.recently_failed("optimize-bench", &snap.board_mac)
            || memory.recently_failed("bench-done", &snap.board_mac);
        if snap.stratum_authorized && !snap.bench_busy && (soft || cliff || snap.jobs_stalled) {
            if bench_failed {
                notes.push(
                    "Bench recently hurt rate — skipping auto-bench; try clock/restart first."
                        .into(),
                );
                if snap.target_mhz < 240 {
                    steps.push(WatchStep {
                        action: AssistAction::SetClock { mhz: 240 },
                        reason: "Bench failed recently — ensure 240 MHz instead".into(),
                    });
                } else if !snap.jobs_stalled {
                    steps.push(WatchStep {
                        action: AssistAction::StartMining,
                        reason: "Bench failed recently — soft restart mining session".into(),
                    });
                }
            } else {
            let why = if cliff {
                format!(
                    "Rate cliff {:.0}→{:.0} kH/s vs baseline — bench climb HW+/HW/HW-SW",
                    snap.baseline_khs, khs
                )
            } else if snap.jobs_stalled {
                "Jobs stalled / soft hashrate while authorized — bench retune".into()
            } else {
                format!(
                    "Rate soft (~{:.0} kH/s/board, floor {:.0}) — bench climb paths",
                    per, snap.target_khs_per_board
                )
            };
            steps.push(WatchStep {
                action: AssistAction::BenchBoards,
                reason: why,
            });
            }
        } else if snap.stratum_authorized && khs < 1.0 && snap.stratum_jobs > 0 && !snap.bench_busy
        {
            if !bench_failed {
            steps.push(WatchStep {
                action: AssistAction::BenchBoards,
                reason: "Jobs flowing but hashrate ~0 — retune boards".into(),
            });
            }
        }
        if snap.baseline_khs > 0.0 {
            notes.push(format!(
                "Baseline {:.0} kH/s · now {:.0}{}",
                snap.baseline_khs,
                khs,
                if snap.rate_cliff { " · CLIFF" } else { "" }
            ));
        }
    }

    let headline = if snap.stratum_authorized {
        format!(
            "Watch · {:.0} kH/s · stratum OK · {} board(s)",
            khs, snap.linked_boards.max(u32::from(snap.usb_open))
        )
    } else if snap.mining {
        format!(
            "Watch · mining · stratum {} · {:.0} kH/s",
            if snap.stratum_phase.is_empty() {
                "…"
            } else {
                snap.stratum_phase.as_str()
            },
            khs
        )
    } else {
        "Watch · fleet idle".into()
    };

    let anomalies = collect_anomalies(snap);
    if !anomalies.is_empty() {
        notes.push(format!("Anomalies: {}", anomalies.join(", ")));
    }
    let detail = notes.join("\n");
    let signature = format!(
        "{}|{}|{}|{}|{}|{:.0}|{}|{}",
        snap.mining,
        snap.stratum_authorized,
        snap.stratum_phase,
        steps
            .iter()
            .map(|s| format!("{:?}", std::mem::discriminant(&s.action)))
            .collect::<Vec<_>>()
            .join(","),
        snap.target_mhz,
        khs,
        snap.session_accepted + snap.session_rejected,
        anomalies.join(";")
    );

    WatchReport {
        headline,
        detail,
        steps,
        signature,
        anomalies,
    }
}

pub fn format_watch_report(report: &WatchReport) -> String {
    if report.detail.is_empty() {
        report.headline.clone()
    } else {
        format!("{}\n{}", report.headline, report.detail)
    }
}

pub fn parse_action(name: &str, arguments: &str) -> Result<AssistAction, String> {
    let args: Value = if arguments.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(arguments).map_err(|e| format!("bad tool args: {e}"))?
    };
    let opt_str = |k: &str| {
        args.get(k)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    match name {
        "get_fleet_status" => Ok(AssistAction::GetFleetStatus),
        "watch_stratum" => Ok(AssistAction::WatchStratum),
        "optimize_hashrate" => Ok(AssistAction::OptimizeHashrate),
        "list_ports" => Ok(AssistAction::ListPorts),
        "scan_workers" => Ok(AssistAction::ScanWorkers),
        "connect_board" => Ok(AssistAction::ConnectBoard {
            endpoint: opt_str("endpoint"),
        }),
        "disconnect_board" => Ok(AssistAction::DisconnectBoard {
            endpoint: opt_str("endpoint"),
        }),
        "set_pool_config" => Ok(AssistAction::SetPoolConfig {
            stratum: opt_str("stratum"),
            worker: opt_str("worker"),
            password: opt_str("password"),
        }),
        "start_mining" => Ok(AssistAction::StartMining),
        "stop_mining" => Ok(AssistAction::StopMining),
        "set_clock" => {
            let mhz = args
                .get("mhz")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| "set_clock needs mhz".to_string())? as u8;
            if !matches!(mhz, 80 | 160 | 240) {
                return Err("mhz must be 80, 160, or 240".into());
            }
            Ok(AssistAction::SetClock { mhz })
        }
        "bench_boards" => Ok(AssistAction::BenchBoards),
        "configure_board_wifi" => Ok(AssistAction::ConfigureBoardWifi {
            endpoint: opt_str("endpoint"),
            ssid: opt_str("ssid").ok_or_else(|| "ssid required".to_string())?,
            password: args
                .get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        }),
        "get_event_log" => {
            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(12)
                .clamp(1, 40) as usize;
            Ok(AssistAction::GetEventLog { limit })
        }
        "update_board_firmware" => Ok(AssistAction::UpdateBoardFirmware {
            live_push: args
                .get("live_push")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            wifi: args.get("wifi").and_then(|v| v.as_bool()).unwrap_or(false),
        }),
        "switch_tab" => Ok(AssistAction::SwitchTab {
            tab: opt_str("tab").unwrap_or_else(|| "assist".into()),
        }),
        other => Err(format!("unknown tool: {other}")),
    }
}

/// Built-in Local AI: map plain English to tools + a natural reply (no cloud).
pub fn local_assist(user: &str, snap: &AssistSnapshot) -> (String, Vec<PendingTool>) {
    let low = user.to_ascii_lowercase();
    let mut tools = Vec::new();

    let push = |tools: &mut Vec<PendingTool>, action: AssistAction| {
        tools.push(PendingTool {
            id: format!("local-{}", tools.len() + 1),
            action,
        });
    };

    if low.contains("watch")
        || low.contains("stratum")
        || low.contains("monitor")
        || low.contains("status")
        || low.contains("hashrate")
        || low.contains("how's")
        || low.contains("how is")
        || low == "fleet"
        || (low.contains("what") && (low.contains("doing") || low.contains("happening")))
    {
        push(&mut tools, AssistAction::WatchStratum);
        if low.contains("max")
            || low.contains("optim")
            || low.contains("push")
            || low.contains("faster")
            || low.contains("boost")
        {
            push(&mut tools, AssistAction::OptimizeHashrate);
        }
        return (
            format!(
                "Built-in AI · watching stratum + hashrate…\n{}",
                builtin_brief(snap)
            ),
            tools,
        );
    }
    if low.contains("optim")
        || low.contains("max hash")
        || low.contains("maximise")
        || low.contains("maximize")
        || low.contains("boost")
        || low.contains("tune")
        || low == "max hashrate"
        || low.contains("faster")
        || low.contains("speed up")
    {
        push(&mut tools, AssistAction::OptimizeHashrate);
        return (
            "Built-in AI · pushing hashrate (clock / mine / bench from live signals)…".into(),
            tools,
        );
    }
    if low.contains("scan") || low.contains("find board") || low.contains("find worker") {
        push(&mut tools, AssistAction::ScanWorkers);
        return ("Built-in AI · scanning for CYD boards…".into(), tools);
    }
    if low.contains("list port") || low.contains("com port") || low.contains("serial") {
        push(&mut tools, AssistAction::ListPorts);
        return ("Built-in AI · refreshing serial ports…".into(), tools);
    }
    if low.contains("stop mine") || low.contains("stop mining") || low == "stop" {
        push(&mut tools, AssistAction::StopMining);
        return ("Built-in AI · stopping mining…".into(), tools);
    }
    if low.contains("start mine") || low.contains("start mining") || low == "mine" {
        push(&mut tools, AssistAction::StartMining);
        return ("Built-in AI · starting mining…".into(), tools);
    }
    if low.contains("connect") || low.contains("link") {
        push(&mut tools, AssistAction::ConnectBoard { endpoint: None });
        return ("Built-in AI · linking the selected board…".into(), tools);
    }
    if low.contains("disconnect") || low.contains("close usb") {
        push(&mut tools, AssistAction::DisconnectBoard { endpoint: None });
        return ("Built-in AI · disconnecting…".into(), tools);
    }
    if low.contains("bench") || low.contains("retune") {
        push(&mut tools, AssistAction::BenchBoards);
        return ("Built-in AI · running board bench…".into(), tools);
    }
    if low.contains("log") || low.contains("event") {
        push(&mut tools, AssistAction::GetEventLog { limit: 16 });
        return ("Built-in AI · pulling recent event log…".into(), tools);
    }
    if low.contains("setup") && low.contains("wifi") {
        push(
            &mut tools,
            AssistAction::SwitchTab {
                tab: "setup".into(),
            },
        );
        return ("Built-in AI · opening Setup for Wi‑Fi…".into(), tools);
    }
    if low.contains("flash") || low.contains("firmware") || low.contains("update board") {
        push(
            &mut tools,
            AssistAction::UpdateBoardFirmware {
                live_push: true,
                wifi: false,
            },
        );
        return (
            "Built-in AI · firmware update needs your confirmation…".into(),
            tools,
        );
    }
    if low.contains("240") && (low.contains("mhz") || low.contains("clock")) {
        push(&mut tools, AssistAction::SetClock { mhz: 240 });
        return ("Built-in AI · setting clock to 240 MHz…".into(), tools);
    }
    if low.contains("160") && (low.contains("mhz") || low.contains("clock")) {
        push(&mut tools, AssistAction::SetClock { mhz: 160 });
        return ("Built-in AI · setting clock to 160 MHz…".into(), tools);
    }
    if low.contains("80") && (low.contains("mhz") || low.contains("clock")) {
        push(&mut tools, AssistAction::SetClock { mhz: 80 });
        return ("Built-in AI · setting clock to 80 MHz…".into(), tools);
    }
    if low.contains("help") || low.contains("what can") || low == "?" {
        return (
            "Built-in Local AI (runs inside Companion — no cloud required).\n\
I watch stratum + hashrate and can start/stop mining, bench, set 240 MHz, scan/connect boards.\n\
Try: Watch stratum · Max hashrate · Start mining · Bench.".into(),
            tools,
        );
    }
    if low.contains("why")
        && (low.contains("reject") || low.contains("accept") || low.contains("share"))
    {
        let report = evaluate_mining_watch(snap, &AssistMemory::default());
        return (
            format!(
                "Built-in AI · share health\n{}\n\nTip: CYDs need low share difficulty (ESP/IoT pool, e.g. HM :3337).",
                format_watch_report(&report)
            ),
            tools,
        );
    }

    let report = evaluate_mining_watch(snap, &AssistMemory::default());
    (
        format!(
            "Built-in Local AI\n{}\n\n{}",
            builtin_brief(snap),
            format_watch_report(&report)
        ),
        tools,
    )
}

fn builtin_brief(snap: &AssistSnapshot) -> String {
    format!(
        "USB={} · mining={} · {:.0} kH/s (baseline {:.0}) · stratum {}{} · A={} R={} · {} board(s) · {} MHz",
        if snap.usb_open { "linked" } else { "idle" },
        if snap.mining { "on" } else { "off" },
        snap.hashrate_khs,
        snap.baseline_khs,
        if snap.stratum_authorized {
            "AUTHORIZED"
        } else if snap.stratum_connected {
            "connected"
        } else {
            "down"
        },
        if snap.stratum_phase.is_empty() {
            String::new()
        } else {
            format!("/{}", snap.stratum_phase)
        },
        snap.session_accepted,
        snap.session_rejected,
        snap.linked_boards.max(u32::from(snap.usb_open)),
        snap.target_mhz
    )
}

/// Anomaly escalation using built-in tools only (no network LLM).
pub fn builtin_anomaly_plan(
    anomalies: &[String],
    snap: &AssistSnapshot,
) -> (String, Vec<PendingTool>) {
    let report = evaluate_mining_watch(snap, &AssistMemory::default());
    let mut tools: Vec<PendingTool> = report
        .steps
        .into_iter()
        .enumerate()
        .map(|(i, step)| PendingTool {
            id: format!("anom-{}", i + 1),
            action: step.action,
        })
        .collect();
    if tools.is_empty() && snap.stratum_authorized && !snap.bench_busy {
        tools.push(PendingTool {
            id: "anom-opt".into(),
            action: AssistAction::OptimizeHashrate,
        });
    }
    (
        format!(
            "Built-in AI · anomaly [{}]\n{}",
            anomalies.join(", "),
            format_watch_report(&evaluate_mining_watch(snap, &AssistMemory::default()))
        ),
        tools,
    )
}

pub fn max_tool_rounds() -> u8 {
    MAX_TOOL_ROUNDS
}

pub fn suggest_chips() -> &'static [&'static str] {
    &[
        "Watch stratum",
        "Max hashrate",
        "Start mining",
        "Stop mining",
        "Bench",
        "Clock 240",
        "Scan boards",
        "Help",
    ]
}

fn messages_to_openai(msgs: &[AssistMessage]) -> Value {
    let mut out = Vec::new();
    for m in msgs {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "role".into(),
            json!(match m.role {
                AssistRole::System => "system",
                AssistRole::User => "user",
                AssistRole::Assistant => "assistant",
                AssistRole::Tool => "tool",
            }),
        );
        if m.role == AssistRole::Tool {
            if let Some(id) = &m.tool_call_id {
                obj.insert("tool_call_id".into(), json!(id));
            }
            obj.insert("content".into(), json!(m.content));
        } else if !m.tool_calls.is_empty() {
            obj.insert(
                "content".into(),
                if m.content.is_empty() {
                    Value::Null
                } else {
                    json!(m.content)
                },
            );
            let calls: Vec<Value> = m
                .tool_calls
                .iter()
                .map(|c| {
                    json!({
                        "id": c.id,
                        "type": "function",
                        "function": { "name": c.name, "arguments": c.arguments }
                    })
                })
                .collect();
            obj.insert("tool_calls".into(), Value::Array(calls));
        } else {
            obj.insert("content".into(), json!(m.content));
        }
        out.push(Value::Object(obj));
    }
    Value::Array(out)
}

#[derive(Debug)]
pub enum LlmRound {
    /// Model wants tools — append assistant message + execute these.
    Tools {
        assistant: AssistMessage,
        pending: Vec<PendingTool>,
    },
    /// Final natural-language reply.
    Done { assistant: AssistMessage },
}

pub struct AssistClient {
    pub backend: AssistBackend,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl AssistClient {
    pub fn from_backend(
        backend: AssistBackend,
        key: &str,
        base: &str,
        model: &str,
    ) -> Self {
        match backend {
            AssistBackend::BuiltIn => Self {
                backend,
                api_key: String::new(),
                base_url: String::new(),
                model: "built-in".into(),
            },
            AssistBackend::Ollama => {
                let base_url = if base.trim().is_empty() {
                    OLLAMA_BASE_URL.to_string()
                } else {
                    base.trim().trim_end_matches('/').to_string()
                };
                let model = if model.trim().is_empty() {
                    OLLAMA_DEFAULT_MODEL.to_string()
                } else {
                    model.trim().to_string()
                };
                // Ollama accepts any/empty key; use a placeholder for Authorization headers.
                let api_key = if key.trim().is_empty() {
                    "ollama".into()
                } else {
                    key.trim().to_string()
                };
                Self {
                    backend,
                    api_key,
                    base_url,
                    model,
                }
            }
            AssistBackend::Cloud => {
                let api_key = if key.trim().is_empty() {
                    std::env::var("OPENAI_API_KEY")
                        .or_else(|_| std::env::var("CYD_ASSIST_API_KEY"))
                        .unwrap_or_default()
                } else {
                    key.trim().to_string()
                };
                let base_url = if base.trim().is_empty() {
                    DEFAULT_BASE_URL.to_string()
                } else {
                    base.trim().trim_end_matches('/').to_string()
                };
                let model = if model.trim().is_empty() {
                    DEFAULT_MODEL.to_string()
                } else {
                    model.trim().to_string()
                };
                Self {
                    backend,
                    api_key,
                    base_url,
                    model,
                }
            }
        }
    }

    pub fn uses_http(&self) -> bool {
        !matches!(self.backend, AssistBackend::BuiltIn)
    }

    pub fn configured(&self) -> bool {
        match self.backend {
            AssistBackend::BuiltIn => true,
            AssistBackend::Ollama => !self.base_url.is_empty(),
            AssistBackend::Cloud => !self.api_key.is_empty(),
        }
    }

    pub fn chat_round(&self, messages: &[AssistMessage]) -> Result<LlmRound, String> {
        if matches!(self.backend, AssistBackend::BuiltIn) {
            return Err("Built-in AI does not use HTTP — use local_assist.".into());
        }
        if matches!(self.backend, AssistBackend::Cloud) && self.api_key.is_empty() {
            return Err("Cloud Assist needs an API key in Settings.".into());
        }
        let url = format!("{}/chat/completions", self.base_url);
        let body = json!({
            "model": self.model,
            "messages": messages_to_openai(messages),
            "tools": tool_definitions(),
            "tool_choice": "auto",
            "temperature": 0.2,
        });
        let resp = ureq::post(&url)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(90))
            .send_json(body)
            .map_err(|e| format!("Assist API: {e}"))?;
        let status = resp.status();
        let text = resp
            .into_string()
            .map_err(|e| format!("Assist API read: {e}"))?;
        if !(200..300).contains(&status) {
            return Err(format!("Assist API HTTP {status}: {}", trunc(&text, 400)));
        }
        let v: Value =
            serde_json::from_str(&text).map_err(|e| format!("Assist API JSON: {e}"))?;
        let choice = v
            .pointer("/choices/0/message")
            .ok_or_else(|| "Assist API: missing choices[0].message".to_string())?;
        let content = choice
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();
        let mut tool_calls = Vec::new();
        if let Some(arr) = choice.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in arr {
                let id = tc
                    .get("id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("call")
                    .to_string();
                let name = tc
                    .pointer("/function/name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let arguments = tc
                    .pointer("/function/arguments")
                    .and_then(|x| x.as_str())
                    .unwrap_or("{}")
                    .to_string();
                tool_calls.push(AssistToolCall {
                    id,
                    name,
                    arguments,
                });
            }
        }
        let mut clean_pending = Vec::new();
        let mut parse_errs = Vec::new();
        for tc in &tool_calls {
            match parse_action(&tc.name, &tc.arguments) {
                Ok(action) => clean_pending.push(PendingTool {
                    id: tc.id.clone(),
                    action,
                }),
                Err(e) => parse_errs.push(format!("{}: {e}", tc.name)),
            }
        }
        let assistant = AssistMessage {
            role: AssistRole::Assistant,
            content: content.clone(),
            tool_call_id: None,
            tool_calls: tool_calls.clone(),
        };
        if !clean_pending.is_empty() {
            return Ok(LlmRound::Tools {
                assistant,
                pending: clean_pending,
            });
        }
        if !parse_errs.is_empty() {
            return Ok(LlmRound::Done {
                assistant: AssistMessage::assistant(format!(
                    "{content}\n\n(Tool parse error: {})",
                    parse_errs.join("; ")
                )),
            });
        }
        Ok(LlmRound::Done {
            assistant: AssistMessage::assistant(if content.is_empty() {
                "(no reply)".into()
            } else {
                content
            }),
        })
    }
}

fn trunc(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}
