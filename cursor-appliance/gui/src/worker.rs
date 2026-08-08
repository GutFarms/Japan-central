use crate::settings::GuiSettings;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerState {
    Stopped,
    Starting,
    Running,
    Offline,
    Error(String),
}

pub struct WorkerController {
    appliance_root: PathBuf,
    child: Option<Child>,
    pub state: WorkerState,
    pub last_health: String,
    pub last_log: String,
    log_tail: Arc<Mutex<Vec<String>>>,
    last_poll: Instant,
}

impl WorkerController {
    pub fn new(appliance_root: PathBuf) -> Self {
        Self {
            appliance_root,
            child: None,
            state: WorkerState::Stopped,
            last_health: "unchecked".into(),
            last_log: String::new(),
            log_tail: Arc::new(Mutex::new(Vec::new())),
            last_poll: Instant::now() - Duration::from_secs(10),
        }
    }

    pub fn start_script(&self) -> PathBuf {
        self.appliance_root.join("scripts").join("start-worker.sh")
    }

    pub fn start(&mut self, settings: &GuiSettings) -> Result<(), String> {
        if settings.offline_mode {
            self.state = WorkerState::Offline;
            return Err("offline mode is on — cloud worker disabled".into());
        }
        if self.is_alive() {
            self.state = WorkerState::Running;
            return Ok(());
        }

        let script = self.start_script();
        if !script.is_file() {
            return Err(format!("missing start script: {}", script.display()));
        }

        // Ensure .env reflects current GUI settings before spawn.
        settings.save(&self.appliance_root)?;

        let mut cmd = Command::new(&script);
        cmd.current_dir(&self.appliance_root)
            .env("CURSOR_APPLIANCE_OFFLINE", "0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| format!("spawn worker: {e}"))?;
        self.attach_log_readers(&mut child);
        self.child = Some(child);
        self.state = WorkerState::Starting;
        self.last_log = "worker process starting…".into();
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // Also stop any orphaned start-worker / agent worker for this appliance.
        let _ = Command::new("pkill")
            .args(["-f", "scripts/start-worker.sh"])
            .status();
        let name = "cursor-appliance";
        let _ = Command::new("pkill")
            .args(["-f", &format!("agent worker.*--name {name}")])
            .status();
        self.state = WorkerState::Stopped;
        self.last_health = "stopped".into();
        self.last_log = "worker stopped".into();
    }

    pub fn set_offline(&mut self, offline: bool, settings: &mut GuiSettings) {
        settings.offline_mode = offline;
        let _ = settings.save(&self.appliance_root);
        if offline {
            self.stop();
            self.state = WorkerState::Offline;
            self.last_log = "offline mode — local UI only, cloud worker off".into();
        } else if matches!(self.state, WorkerState::Offline) {
            self.state = WorkerState::Stopped;
            self.last_log = "online mode — ready to start cloud worker".into();
        }
    }

    pub fn tick(&mut self, settings: &GuiSettings) {
        if settings.offline_mode {
            self.state = WorkerState::Offline;
            return;
        }

        let alive = self.is_alive();
        if !alive && matches!(self.state, WorkerState::Starting | WorkerState::Running) {
            self.state = WorkerState::Error("worker process exited".into());
            self.child = None;
        }

        if self.last_poll.elapsed() >= Duration::from_secs(2) {
            self.last_poll = Instant::now();
            self.poll_health(&settings.management_addr);
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

    fn is_alive(&mut self) -> bool {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(None) => true,
                Ok(Some(_)) => false,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    fn poll_health(&mut self, management_addr: &str) {
        let url = format!("http://{management_addr}/healthz");
        match ureq::get(&url).timeout(Duration::from_secs(1)).call() {
            Ok(resp) if resp.status() >= 200 && resp.status() < 300 => {
                self.last_health = "healthy".into();
                if !matches!(self.state, WorkerState::Offline) {
                    self.state = WorkerState::Running;
                }
            }
            Ok(resp) => {
                self.last_health = format!("http {}", resp.status());
            }
            Err(err) => {
                self.last_health = format!("unreachable ({err})");
                if matches!(self.state, WorkerState::Running) {
                    // Process may still be starting bridge before health binds.
                    self.state = WorkerState::Starting;
                }
            }
        }
    }

    fn attach_log_readers(&self, child: &mut Child) {
        let tail = Arc::clone(&self.log_tail);
        if let Some(stdout) = child.stdout.take() {
            let tail_out = Arc::clone(&tail);
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().flatten() {
                    push_log(&tail_out, line);
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let tail_err = Arc::clone(&tail);
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    push_log(&tail_err, line);
                }
            });
        }
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
    // Prefer CARGO / source layout: <repo>/cursor-appliance/gui → appliance root is parent.
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let gui = PathBuf::from(manifest);
        if let Some(parent) = gui.parent() {
            return parent.to_path_buf();
        }
    }

    // Installed next to scripts: cursor-appliance/gui/target/... or cursor-appliance/bin
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
