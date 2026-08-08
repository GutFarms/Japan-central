use crate::settings::GuiSettings;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    None,
    Cloud,
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerState {
    Stopped,
    Starting,
    Running,
    Local,
    FailingOver,
    Offline,
    Error(String),
}

pub struct WorkerController {
    appliance_root: PathBuf,
    cloud_child: Option<Child>,
    local_child: Option<Child>,
    pub backend: Backend,
    pub state: WorkerState,
    pub last_health: String,
    pub last_log: String,
    pub local_status: String,
    log_tail: Arc<Mutex<Vec<String>>>,
    last_poll: Instant,
    suppress_failover: bool,
}

impl WorkerController {
    pub fn new(appliance_root: PathBuf) -> Self {
        Self {
            appliance_root,
            cloud_child: None,
            local_child: None,
            backend: Backend::None,
            state: WorkerState::Stopped,
            last_health: "unchecked".into(),
            last_log: String::new(),
            local_status: String::new(),
            log_tail: Arc::new(Mutex::new(Vec::new())),
            last_poll: Instant::now() - Duration::from_secs(10),
            suppress_failover: false,
        }
    }

    pub fn start_cloud_script(&self) -> PathBuf {
        self.appliance_root.join("scripts").join("start-worker.sh")
    }

    pub fn start_local_script(&self) -> PathBuf {
        self.appliance_root
            .join("scripts")
            .join("start-local-worker.sh")
    }

    pub fn start_cloud(&mut self, settings: &GuiSettings) -> Result<(), String> {
        if settings.offline_mode {
            self.state = WorkerState::Offline;
            return Err("offline mode is on — starting local worker instead".into());
        }
        self.suppress_failover = true;
        self.stop_local();
        self.suppress_failover = false;

        if self.cloud_alive() {
            self.backend = Backend::Cloud;
            self.state = WorkerState::Running;
            return Ok(());
        }

        let script = self.start_cloud_script();
        if !script.is_file() {
            return Err(format!("missing start script: {}", script.display()));
        }

        settings.save(&self.appliance_root)?;

        let mut cmd = Command::new(&script);
        cmd.current_dir(&self.appliance_root)
            .env("CURSOR_APPLIANCE_OFFLINE", "0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| format!("spawn cloud worker: {e}"))?;
        self.attach_log_readers(&mut child, "cloud");
        self.cloud_child = Some(child);
        self.backend = Backend::Cloud;
        self.state = WorkerState::Starting;
        self.last_log = "cloud worker starting…".into();
        Ok(())
    }

    pub fn start_local(&mut self, settings: &GuiSettings) -> Result<(), String> {
        if self.local_alive() {
            self.backend = Backend::Local;
            self.state = if settings.offline_mode {
                WorkerState::Offline
            } else {
                WorkerState::Local
            };
            return Ok(());
        }

        let script = self.start_local_script();
        if !script.is_file() {
            return Err(format!("missing local worker script: {}", script.display()));
        }

        // Ensure local binary exists (script builds if needed).
        let _ = settings.save(&self.appliance_root);

        let mut cmd = Command::new(&script);
        cmd.current_dir(&self.appliance_root)
            .env(
                "CURSOR_APPLIANCE_LOCAL_ADDR",
                &settings.local_management_addr,
            )
            .env("CURSOR_APPLIANCE_NAME", &settings.appliance_name)
            .env("CURSOR_APPLIANCE_WORKER_DIR", &settings.worker_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("spawn local worker: {e}"))?;
        self.attach_log_readers(&mut child, "local");
        self.local_child = Some(child);
        self.backend = Backend::Local;
        self.state = if settings.offline_mode {
            WorkerState::Offline
        } else {
            WorkerState::Local
        };
        self.last_log = "local worker takeover started…".into();
        self.last_health = "local starting".into();
        Ok(())
    }

    /// Stop cloud worker only. When failover is enabled, local takes over.
    pub fn stop_cloud(&mut self, settings: &GuiSettings) {
        self.kill_cloud_process();
        if settings.local_failover || settings.offline_mode {
            self.state = WorkerState::FailingOver;
            self.last_log = "cloud stopped — starting local worker takeover".into();
            if let Err(err) = self.start_local(settings) {
                self.state = WorkerState::Error(err);
            }
        } else {
            self.backend = Backend::None;
            self.state = WorkerState::Stopped;
            self.last_health = "stopped".into();
            self.last_log = "cloud worker stopped".into();
        }
    }

    pub fn stop_local(&mut self) {
        if let Some(mut child) = self.local_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let addr = std::env::var("CURSOR_APPLIANCE_LOCAL_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8734".into());
        let _ = ureq::post(&format!("http://{addr}/v1/shutdown"))
            .timeout(Duration::from_millis(400))
            .call();
        let _ = Command::new("pkill")
            .args(["-f", "scripts/start-local-worker.sh"])
            .status();
        let _ = Command::new("pkill")
            .args(["-f", "cursor-local-worker"])
            .status();
        if self.backend == Backend::Local {
            self.backend = Backend::None;
        }
    }

    pub fn stop_all(&mut self) {
        self.suppress_failover = true;
        self.kill_cloud_process();
        self.stop_local();
        self.suppress_failover = false;
        self.backend = Backend::None;
        self.state = WorkerState::Stopped;
        self.last_health = "stopped".into();
        self.last_log = "all workers stopped".into();
    }

    pub fn set_offline(&mut self, offline: bool, settings: &mut GuiSettings) {
        settings.offline_mode = offline;
        let _ = settings.save(&self.appliance_root);
        if offline {
            self.suppress_failover = true;
            self.kill_cloud_process();
            self.suppress_failover = false;
            match self.start_local(settings) {
                Ok(()) => {
                    self.state = WorkerState::Offline;
                    self.last_log =
                        "offline mode — local worker took over (cloud disconnected)".into();
                }
                Err(err) => {
                    self.state = WorkerState::Error(err);
                }
            }
        } else if matches!(self.state, WorkerState::Offline | WorkerState::Local) {
            self.last_log = "online mode — local worker still running; start cloud when ready".into();
            self.state = WorkerState::Local;
        }
    }

    pub fn tick(&mut self, settings: &GuiSettings) {
        let cloud_alive = self.cloud_alive();
        let local_alive = self.local_alive();

        // Unexpected cloud death → failover.
        if !cloud_alive
            && matches!(
                self.state,
                WorkerState::Starting | WorkerState::Running
            )
            && self.backend == Backend::Cloud
            && !self.suppress_failover
        {
            self.cloud_child = None;
            if settings.local_failover || settings.offline_mode {
                self.state = WorkerState::FailingOver;
                self.last_log = "cloud worker exited — failing over to local worker".into();
                if let Err(err) = self.start_local(settings) {
                    self.state = WorkerState::Error(err);
                }
            } else {
                self.backend = Backend::None;
                self.state = WorkerState::Error("cloud worker process exited".into());
            }
        }

        if settings.offline_mode && !local_alive && settings.local_failover {
            let _ = self.start_local(settings);
        }

        if self.last_poll.elapsed() >= Duration::from_secs(2) {
            self.last_poll = Instant::now();
            match self.backend {
                Backend::Cloud => self.poll_health(&settings.management_addr, true),
                Backend::Local => {
                    self.poll_health(&settings.local_management_addr, false);
                    self.poll_local_status(&settings.local_management_addr);
                    if settings.offline_mode {
                        self.state = WorkerState::Offline;
                    } else if local_alive {
                        self.state = WorkerState::Local;
                    }
                }
                Backend::None => {}
            }
            if let Ok(tail) = self.log_tail.lock() {
                if let Some(last) = tail.last() {
                    self.last_log = last.clone();
                }
            }
        }
    }

    pub fn recent_logs(&self) -> String {
        self.log_tail
            .lock()
            .map(|v| v.join("\n"))
            .unwrap_or_default()
    }

    fn kill_cloud_process(&mut self) {
        if let Some(mut child) = self.cloud_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = Command::new("pkill")
            .args(["-f", "scripts/start-worker.sh"])
            .status();
        let _ = Command::new("pkill")
            .args(["-f", "agent worker.*--name"])
            .status();
    }

    fn cloud_alive(&mut self) -> bool {
        alive(&mut self.cloud_child)
    }

    fn local_alive(&mut self) -> bool {
        alive(&mut self.local_child)
    }

    fn poll_health(&mut self, management_addr: &str, is_cloud: bool) {
        let url = format!("http://{management_addr}/healthz");
        match ureq::get(&url).timeout(Duration::from_secs(1)).call() {
            Ok(resp) if resp.status() >= 200 && resp.status() < 300 => {
                self.last_health = if is_cloud {
                    "cloud healthy".into()
                } else {
                    "local healthy".into()
                };
                if is_cloud && !matches!(self.state, WorkerState::Offline) {
                    self.state = WorkerState::Running;
                    self.backend = Backend::Cloud;
                }
            }
            Ok(resp) => {
                self.last_health = format!("http {}", resp.status());
            }
            Err(err) => {
                self.last_health = format!("unreachable ({err})");
                if is_cloud && matches!(self.state, WorkerState::Running) {
                    self.state = WorkerState::Starting;
                }
            }
        }
    }

    fn poll_local_status(&mut self, addr: &str) {
        let url = format!("http://{addr}/status");
        if let Ok(resp) = ureq::get(&url).timeout(Duration::from_secs(1)).call() {
            if let Ok(text) = resp.into_string() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    let uptime = v
                        .get("uptime_secs")
                        .and_then(|x| x.as_u64())
                        .unwrap_or(0);
                    let done = v.get("jobs_done").and_then(|x| x.as_u64()).unwrap_or(0);
                    let queue = v
                        .get("queue_incoming")
                        .and_then(|x| x.as_u64())
                        .unwrap_or(0);
                    let last = v
                        .get("last_job")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    self.local_status = format!(
                        "uptime {uptime}s · done {done} · queue {queue}{}",
                        if last.is_empty() {
                            String::new()
                        } else {
                            format!(" · last {last}")
                        }
                    );
                }
            }
        }
    }

    fn attach_log_readers(&self, child: &mut Child, label: &str) {
        let tail = Arc::clone(&self.log_tail);
        let label_out = label.to_string();
        if let Some(stdout) = child.stdout.take() {
            let tail_out = Arc::clone(&tail);
            let label = label_out.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().flatten() {
                    push_log(&tail_out, format!("[{label}] {line}"));
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let tail_err = Arc::clone(&tail);
            let label = label_out;
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    push_log(&tail_err, format!("[{label}] {line}"));
                }
            });
        }
    }
}

fn alive(child: &mut Option<Child>) -> bool {
    if let Some(proc) = child.as_mut() {
        match proc.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => {
                *child = None;
                false
            }
            Err(_) => false,
        }
    } else {
        false
    }
}

fn push_log(tail: &Arc<Mutex<Vec<String>>>, line: String) {
    if let Ok(mut guard) = tail.lock() {
        guard.push(line);
        let overflow = guard.len().saturating_sub(200);
        if overflow > 0 {
            guard.drain(0..overflow);
        }
    }
}

pub fn appliance_root_from_exe() -> PathBuf {
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let gui = PathBuf::from(manifest);
        if let Some(parent) = gui.parent() {
            return parent.to_path_buf();
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().take(6) {
            if ancestor.join("scripts").join("start-worker.sh").is_file() {
                return ancestor.to_path_buf();
            }
        }
    }

    PathBuf::from(".").canonicalize().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn path_exists(path: &str) -> bool {
    Path::new(path).exists()
}

