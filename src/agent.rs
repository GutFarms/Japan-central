use anyhow::Result;
use colored::Colorize;
use std::io::{self, Write};
use std::sync::Arc;

use crate::client::{InputItem, XaiClient};
use crate::config::Config;
use crate::tools::ToolRuntime;

pub struct Agent {
    client: XaiClient,
    tools: Arc<ToolRuntime>,
    tool_defs: Vec<serde_json::Value>,
    instructions: String,
    max_turns: usize,
    previous_response_id: Option<String>,
}

impl Agent {
    pub fn new(cfg: &Config, client: XaiClient, tools: Arc<ToolRuntime>) -> Self {
        let tool_defs = tools.definitions(cfg.web_search, cfg.code_interpreter);
        let instructions = system_prompt(cfg);
        Self {
            client,
            tools,
            tool_defs,
            instructions,
            max_turns: cfg.max_turns,
            previous_response_id: None,
        }
    }

    pub fn reset(&mut self) {
        self.previous_response_id = None;
    }

    pub async fn run_turn(&mut self, user_message: &str) -> Result<String> {
        let mut input = vec![InputItem::user(user_message)];
        let mut final_text = String::new();

        for turn in 1..=self.max_turns {
            let instructions = if self.previous_response_id.is_none() {
                Some(self.instructions.as_str())
            } else {
                None
            };

            let response = self
                .client
                .create_response(
                    input,
                    &self.tool_defs,
                    instructions,
                    self.previous_response_id.as_deref(),
                )
                .await?;

            self.previous_response_id = Some(response.id.clone());

            let text = response.text();
            if !text.trim().is_empty() {
                println!("{}", text.green());
                final_text = text;
            }

            let calls = response.function_calls();
            if calls.is_empty() {
                return Ok(final_text);
            }

            println!(
                "{}",
                format!("⚙ tool turn {turn}: {} call(s)", calls.len()).dimmed()
            );

            let mut outputs = Vec::with_capacity(calls.len());
            for (call_id, name, arguments) in calls {
                println!(
                    "{}",
                    format!("→ {name}({})", compact_args(&arguments)).cyan()
                );
                let _ = io::stdout().flush();
                let result = self.tools.execute(&name, &arguments).await;
                let preview = preview_result(&result);
                println!("{}", format!("← {preview}").dimmed());
                outputs.push(InputItem::function_result(call_id, result));
            }

            input = outputs;
        }

        anyhow::bail!(
            "Reached max tool turns ({}). Increase with --max-turns.",
            self.max_turns
        );
    }
}

fn system_prompt(cfg: &Config) -> String {
    format!(
        "You are grok-agent, a local coding agent on the user's PC.\n\
         Backend model: {model} via the xAI API.\n\
         Workspace root: {workspace}\n\
         \n\
         Use tools to inspect and modify files and run commands when needed.\n\
         Prefer small, correct edits. Do not escape the workspace.\n\
         Be concise. When a task is done, summarize what changed.\n\
         Today (UTC): {today}.",
        model = cfg.model,
        workspace = cfg.workspace.display(),
        today = chrono::Utc::now().format("%Y-%m-%d"),
    )
}

fn compact_args(arguments: &str) -> String {
    let trimmed = arguments.trim();
    if trimmed.len() <= 120 {
        trimmed.to_string()
    } else {
        format!("{}…", &trimmed[..120])
    }
}

fn preview_result(result: &str) -> String {
    let one_line = result.lines().next().unwrap_or("").trim();
    if one_line.len() <= 160 {
        one_line.to_string()
    } else {
        format!("{}…", &one_line[..160])
    }
}
