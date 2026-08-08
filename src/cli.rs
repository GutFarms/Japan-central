use anyhow::Result;
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::sync::Arc;

use crate::agent::Agent;
use crate::client::XaiClient;
use crate::config::{config_dir, Config};
use crate::tools::ToolRuntime;

pub async fn run_cli(cfg: Config) -> Result<()> {
    cfg.require_api_key()?;

    let client = XaiClient::new(&cfg)?;
    let tools = Arc::new(ToolRuntime::new(&cfg));
    let mut agent = Agent::new(&cfg, client, tools.clone());

    println!("{}", "grok-agent".bold());
    println!(
        "{}  model={}  workspace={}",
        "local runtime → xAI Grok".dimmed(),
        cfg.model.cyan(),
        cfg.workspace.display()
    );
    println!(
        "{}",
        "Commands: /quit  /reset  /help   |   Tip: set XAI_API_KEY in Settings or .env".dimmed()
    );
    println!();

    if let Some(prompt) = cfg.prompt.clone() {
        agent.run_turn(&prompt).await?;
        return Ok(());
    }

    let mut rl = DefaultEditor::new()?;
    let history_path = config_dir().map(|d| d.join("history"));
    if let Some(path) = &history_path {
        let _ = rl.load_history(path);
    }

    loop {
        match rl.readline(&format!("{} ", "you>".bold().blue())) {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(line.as_str());

                match line.as_str() {
                    "/quit" | "/exit" | ":q" => break,
                    "/reset" => {
                        agent.reset();
                        println!("{}", "Conversation reset.".yellow());
                    }
                    "/help" => print_help(),
                    "/workspace" => {
                        println!("{}", tools.workspace().display());
                    }
                    _ => {
                        if let Err(err) = agent.run_turn(&line).await {
                            eprintln!("{} {:#}", "error:".red().bold(), err);
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("{}", "(ctrl-c) use /quit to exit".dimmed());
            }
            Err(ReadlineError::Eof) => break,
            Err(err) => {
                eprintln!("readline error: {err}");
                break;
            }
        }
    }

    if let Some(path) = history_path {
        let _ = rl.save_history(&path);
    }

    Ok(())
}

fn print_help() {
    println!(
        "{}",
        r#"
grok-agent — local agent loop around Grok 4.5

  Type a task. The agent can list/read/write/edit files, glob, and run shell
  commands inside the workspace.

  /help       Show this help
  /workspace  Print workspace root
  /reset      Clear conversation state
  /quit       Exit

  GUI (default):
    grok-agent

  CLI:
    grok-agent --cli
    grok-agent -p "Summarize this repo"
"#
        .trim()
    );
}
