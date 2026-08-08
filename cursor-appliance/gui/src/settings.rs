use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GuiSettings {
    pub dark_mode: bool,
    pub offline_mode: bool,
    pub appliance_name: String,
    pub worker_dir: String,
    pub management_addr: String,
    pub api_key: String,
    pub idle_release_timeout: u64,
    pub debug_worker: bool,
    pub auto_start_worker: bool,
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            dark_mode: true,
            offline_mode: false,
            appliance_name: "cursor-appliance".into(),
            worker_dir: String::new(),
            management_addr: "127.0.0.1:8733".into(),
            api_key: String::new(),
            idle_release_timeout: 0,
            debug_worker: false,
            auto_start_worker: false,
        }
    }
}

impl GuiSettings {
    pub fn settings_path(appliance_root: &Path) -> PathBuf {
        appliance_root.join("data").join("gui-settings.json")
    }

    pub fn load(appliance_root: &Path) -> Self {
        let path = Self::settings_path(appliance_root);
        let mut settings = if path.is_file() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|raw| serde_json::from_str(&raw).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        };

        // Seed from .env when GUI settings are empty.
        let env_path = appliance_root.join(".env");
        if env_path.is_file() {
            if let Ok(raw) = fs::read_to_string(&env_path) {
                for line in raw.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    let Some((key, value)) = line.split_once('=') else {
                        continue;
                    };
                    let value = value.trim().trim_matches('"').to_string();
                    match key.trim() {
                        "CURSOR_API_KEY" if settings.api_key.is_empty() => {
                            settings.api_key = value;
                        }
                        "CURSOR_APPLIANCE_NAME" if settings.appliance_name.is_empty() => {
                            settings.appliance_name = value;
                        }
                        "CURSOR_APPLIANCE_WORKER_DIR" if settings.worker_dir.is_empty() => {
                            settings.worker_dir = value;
                        }
                        "CURSOR_APPLIANCE_MANAGEMENT_ADDR" => {
                            if settings.management_addr == "127.0.0.1:8733" && !value.is_empty() {
                                settings.management_addr = value;
                            }
                        }
                        "CURSOR_APPLIANCE_IDLE_RELEASE_TIMEOUT" => {
                            if let Ok(v) = value.parse() {
                                settings.idle_release_timeout = v;
                            }
                        }
                        "CURSOR_APPLIANCE_OFFLINE" => {
                            settings.offline_mode = matches!(
                                value.to_ascii_lowercase().as_str(),
                                "1" | "true" | "yes" | "on"
                            );
                        }
                        "CURSOR_APPLIANCE_DEBUG" => {
                            settings.debug_worker = matches!(
                                value.to_ascii_lowercase().as_str(),
                                "1" | "true" | "yes" | "on"
                            );
                        }
                        _ => {}
                    }
                }
            }
        }

        if settings.worker_dir.is_empty() {
            if let Some(repo) = appliance_root.parent() {
                settings.worker_dir = repo.display().to_string();
            }
        }

        settings
    }

    pub fn save(&self, appliance_root: &Path) -> Result<(), String> {
        let data_dir = appliance_root.join("data");
        fs::create_dir_all(&data_dir).map_err(|e| format!("create data dir: {e}"))?;

        let path = Self::settings_path(appliance_root);
        let raw = serde_json::to_string_pretty(self).map_err(|e| format!("serialize: {e}"))?;
        fs::write(&path, raw).map_err(|e| format!("write settings: {e}"))?;

        self.sync_env(appliance_root)?;
        Ok(())
    }

    /// Keep shell scripts / systemd in sync with GUI settings.
    fn sync_env(&self, appliance_root: &Path) -> Result<(), String> {
        let env_path = appliance_root.join(".env");
        let mut lines = Vec::new();
        if env_path.is_file() {
            let raw = fs::read_to_string(&env_path).map_err(|e| format!("read .env: {e}"))?;
            for line in raw.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("CURSOR_API_KEY=")
                    || trimmed.starts_with("CURSOR_APPLIANCE_NAME=")
                    || trimmed.starts_with("CURSOR_APPLIANCE_WORKER_DIR=")
                    || trimmed.starts_with("CURSOR_APPLIANCE_MANAGEMENT_ADDR=")
                    || trimmed.starts_with("CURSOR_APPLIANCE_IDLE_RELEASE_TIMEOUT=")
                    || trimmed.starts_with("CURSOR_APPLIANCE_OFFLINE=")
                    || trimmed.starts_with("CURSOR_APPLIANCE_DEBUG=")
                {
                    continue;
                }
                lines.push(line.to_string());
            }
        } else {
            lines.push("# Managed by cursor-appliance-gui".into());
        }

        let push = |lines: &mut Vec<String>, key: &str, value: &str| {
            lines.push(format!("{key}={value}"));
        };

        push(&mut lines, "CURSOR_API_KEY", &self.api_key);
        push(&mut lines, "CURSOR_APPLIANCE_NAME", &self.appliance_name);
        if !self.worker_dir.is_empty() {
            push(
                &mut lines,
                "CURSOR_APPLIANCE_WORKER_DIR",
                &self.worker_dir,
            );
        }
        push(
            &mut lines,
            "CURSOR_APPLIANCE_MANAGEMENT_ADDR",
            &self.management_addr,
        );
        push(
            &mut lines,
            "CURSOR_APPLIANCE_IDLE_RELEASE_TIMEOUT",
            &self.idle_release_timeout.to_string(),
        );
        push(
            &mut lines,
            "CURSOR_APPLIANCE_OFFLINE",
            if self.offline_mode { "1" } else { "0" },
        );
        push(
            &mut lines,
            "CURSOR_APPLIANCE_DEBUG",
            if self.debug_worker { "1" } else { "0" },
        );

        fs::write(&env_path, lines.join("\n") + "\n").map_err(|e| format!("write .env: {e}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&env_path, fs::Permissions::from_mode(0o600));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cursor-appliance-gui-test-{nanos}"));
        fs::create_dir_all(root.join("data")).unwrap();
        root
    }

    #[test]
    fn save_roundtrip_and_env_sync() {
        let root = temp_root();
        let mut settings = GuiSettings::default();
        settings.dark_mode = false;
        settings.offline_mode = true;
        settings.appliance_name = "test-box".into();
        settings.api_key = "secret".into();
        settings.worker_dir = "/tmp/repo".into();
        settings.save(&root).unwrap();

        let loaded = GuiSettings::load(&root);
        assert!(!loaded.dark_mode);
        assert!(loaded.offline_mode);
        assert_eq!(loaded.appliance_name, "test-box");
        assert_eq!(loaded.api_key, "secret");

        let env = fs::read_to_string(root.join(".env")).unwrap();
        assert!(env.contains("CURSOR_APPLIANCE_OFFLINE=1"));
        assert!(env.contains("CURSOR_APPLIANCE_NAME=test-box"));
        assert!(env.contains("CURSOR_API_KEY=secret"));

        let _ = fs::remove_dir_all(root);
    }
}
