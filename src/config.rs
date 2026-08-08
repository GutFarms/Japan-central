use anyhow::{bail, Context, Result};
use clap::Parser;
use std::path::PathBuf;

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

    /// One-shot prompt (skips interactive REPL)
    #[arg(short = 'p', long)]
    pub prompt: Option<String>,

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

impl Config {
    pub fn load() -> Result<Self> {
        let _ = dotenvy::dotenv();
        let mut cfg = Self::parse();
        cfg.workspace = std::fs::canonicalize(&cfg.workspace).with_context(|| {
            format!(
                "workspace path does not exist: {}",
                cfg.workspace.display()
            )
        })?;

        if cfg.api_key.as_deref().unwrap_or("").trim().is_empty() {
            bail!(
                "Missing XAI_API_KEY. Set it in the environment or pass --api-key.\n\
                 Get a key at https://console.x.ai/"
            );
        }

        Ok(cfg)
    }

    pub fn api_key(&self) -> &str {
        self.api_key.as_deref().unwrap_or_default()
    }
}
