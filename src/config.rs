use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Parser)]
#[command(
    name = "grok-agent",
    about = "Local AI coding agent powered by xAI Grok 4.5",
    version
)]
pub struct Config {
    /// xAI API key (or set XAI_API_KEY)
    #[arg(long, env = "XAI_API_KEY")]
    pub api_key: Option<String>,

    /// xAI API base URL
    #[arg(long, env = "XAI_BASE_URL", default_value = "https://api.x.ai/v1")]
    pub base_url: String,

    /// Model id
    #[arg(long, env = "GROK_MODEL", default_value = "grok-4.5")]
    pub model: String,

    /// Workspace root the agent may read/write/execute within
    #[arg(long, env = "GROK_WORKSPACE", default_value = ".")]
    pub workspace: PathBuf,

    /// One-shot prompt (skips GUI / REPL)
    #[arg(short = 'p', long)]
    pub prompt: Option<String>,

    /// Force terminal CLI instead of GUI
    #[arg(long, default_value_t = false)]
    pub cli: bool,

    /// Max agent tool turns per user message
    #[arg(long, default_value_t = 24)]
    pub max_turns: usize,

    /// Enable xAI server-side web_search tool
    #[arg(long, default_value_t = false)]
    pub web_search: bool,

    /// Enable xAI server-side code_interpreter tool
    #[arg(long, default_value_t = false)]
    pub code_interpreter: bool,

    /// Shell command timeout in seconds
    #[arg(long, default_value_t = 60)]
    pub shell_timeout_secs: u64,

    /// Max characters returned from a single tool result
    #[arg(long, default_value_t = 40_000)]
    pub tool_output_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSettings {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub workspace: String,
    pub web_search: bool,
    pub code_interpreter: bool,
    pub max_turns: usize,
}

impl Config {
    pub fn load() -> Result<Self> {
        let _ = dotenvy::dotenv();
        let mut cfg = Self::parse();

        if let Some(saved) = load_saved_settings() {
            if cfg.api_key.as_deref().unwrap_or("").trim().is_empty()
                && !saved.api_key.trim().is_empty()
            {
                cfg.api_key = Some(saved.api_key);
            }
            if cfg.base_url == "https://api.x.ai/v1" && !saved.base_url.trim().is_empty() {
                cfg.base_url = saved.base_url;
            }
            if cfg.model == "grok-4.5" && !saved.model.trim().is_empty() {
                cfg.model = saved.model;
            }
            if cfg.workspace == PathBuf::from(".") && !saved.workspace.trim().is_empty() {
                cfg.workspace = PathBuf::from(saved.workspace);
            }
            if !cfg.web_search {
                cfg.web_search = saved.web_search;
            }
            if !cfg.code_interpreter {
                cfg.code_interpreter = saved.code_interpreter;
            }
            if cfg.max_turns == 24 && saved.max_turns > 0 {
                cfg.max_turns = saved.max_turns;
            }
        }

        cfg.workspace = canonicalize_workspace(&cfg.workspace)?;
        Ok(cfg)
    }

    pub fn require_api_key(&self) -> Result<()> {
        if self.api_key.as_deref().unwrap_or("").trim().is_empty() {
            bail!(
                "Missing XAI_API_KEY. Set it in Settings, the environment, or pass --api-key.\n\
                 Get a key at https://console.x.ai/"
            );
        }
        Ok(())
    }

    pub fn api_key(&self) -> &str {
        self.api_key.as_deref().unwrap_or_default()
    }

    pub fn to_saved(&self) -> SavedSettings {
        SavedSettings {
            api_key: self.api_key().to_string(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            workspace: self.workspace.display().to_string(),
            web_search: self.web_search,
            code_interpreter: self.code_interpreter,
            max_turns: self.max_turns,
        }
    }

    pub fn apply_saved_fields(
        &mut self,
        api_key: String,
        base_url: String,
        model: String,
        workspace: PathBuf,
        web_search: bool,
        code_interpreter: bool,
        max_turns: usize,
    ) -> Result<()> {
        self.api_key = Some(api_key);
        self.base_url = base_url;
        self.model = model;
        self.workspace = canonicalize_workspace(&workspace)?;
        self.web_search = web_search;
        self.code_interpreter = code_interpreter;
        self.max_turns = max_turns.max(1);
        Ok(())
    }
}

pub fn config_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let dir = PathBuf::from(home).join(".grok-agent");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

pub fn settings_path() -> Option<PathBuf> {
    Some(config_dir()?.join("settings.json"))
}

pub fn load_saved_settings() -> Option<SavedSettings> {
    let path = settings_path()?;
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn save_settings(settings: &SavedSettings) -> Result<()> {
    let path = settings_path().context("could not resolve settings path")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, data)?;
    Ok(())
}

fn canonicalize_workspace(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return std::fs::canonicalize(path)
            .with_context(|| format!("workspace path does not exist: {}", path.display()));
    }
    std::fs::create_dir_all(path)
        .with_context(|| format!("could not create workspace: {}", path.display()))?;
    std::fs::canonicalize(path)
        .with_context(|| format!("workspace path does not exist: {}", path.display()))
}
