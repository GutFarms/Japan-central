mod agent;
mod cli;
mod client;
mod config;
mod gui;
mod tools;

use anyhow::Result;
use config::Config;

fn main() -> Result<()> {
    let cfg = Config::load()?;

    if cfg.cli || cfg.prompt.is_some() {
        let rt = tokio::runtime::Runtime::new()?;
        return rt.block_on(cli::run_cli(cfg));
    }

    gui::run_gui(cfg).map_err(|err| anyhow::anyhow!("GUI error: {err}"))
}
