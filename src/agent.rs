use anyhow::Result;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::client::{InputItem, XaiClient};
use crate::config::Config;
use crate::tools::ToolRuntime;

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Status(String),
    AssistantDelta(String),
    ToolStart { name: String, args: String },
    ToolResult { name: String, preview: String },
    Finished,
    Error(String),
}

pub struct Agent {
    client: XaiClient,
    tools: Arc<ToolRuntime>,
    tool_defs: Vec<serde_json::Value>,
    instructions: String,
    max_turns: usize,
    previous_response_id: Option<String>,
    events: Option<Sender<AgentEvent>>,
    echo_console: bool,
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
            events: None,
            echo_console: true,
        }
    }

    pub fn with_events(mut self, tx: Sender<AgentEvent>) -> Self {
        self.events = Some(tx);
        self.echo_console = false;
        self
    }

    pub fn reset(&mut self) {
        self.previous_response_id = None;
    }

    fn emit(&self, event: AgentEvent) {
        if let Some(tx) = &self.events {
            let _ = tx.send(event);
        }
    }

    pub async fn run_turn(&mut self, user_message: &str) -> Result<String> {
        let mut input = vec![InputItem::user(user_message)];
        let mut final_text = String::new();

        for turn in 1..=self.max_turns {
            self.emit(AgentEvent::Status(format!("Thinking (turn {turn})…")));

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
                if self.echo_console {
                    println!("{text}");
                }
                self.emit(AgentEvent::AssistantDelta(text.clone()));
                final_text = text;
            }

            let calls = response.function_calls();
            if calls.is_empty() {
                self.emit(AgentEvent::Finished);
                return Ok(final_text);
            }

            if self.echo_console {
                println!("tool turn {turn}: {} call(s)", calls.len());
            }
            self.emit(AgentEvent::Status(format!(
                "Running {} tool(s)…",
                calls.len()
            )));

            let mut outputs = Vec::with_capacity(calls.len());
            for (call_id, name, arguments) in calls {
                let args_preview = compact_args(&arguments);
                if self.echo_console {
                    println!("→ {name}({args_preview})");
                }
                self.emit(AgentEvent::ToolStart {
                    name: name.clone(),
                    args: args_preview,
                });

                let result = self.tools.execute(&name, &arguments).await;
                let preview = preview_result(&result);
                if self.echo_console {
                    println!("← {preview}");
                }
                self.emit(AgentEvent::ToolResult {
                    name: name.clone(),
                    preview,
                });
                outputs.push(InputItem::function_result(call_id, result));
            }

            input = outputs;
        }

        let err = format!(
            "Reached max tool turns ({}). Increase with --max-turns.",
            self.max_turns
        );
        self.emit(AgentEvent::Error(err.clone()));
        anyhow::bail!(err)
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
