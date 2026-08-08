use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::Config;

#[derive(Debug, Clone, Serialize)]
pub struct FunctionTool {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl FunctionTool {
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            kind: "function",
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum InputItem {
    Message {
        role: String,
        content: String,
    },
    FunctionCallOutput {
        #[serde(rename = "type")]
        kind: &'static str,
        call_id: String,
        output: String,
    },
}

impl InputItem {
    pub fn user(content: impl Into<String>) -> Self {
        Self::Message {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn function_result(call_id: impl Into<String>, output: impl Into<String>) -> Self {
        Self::FunctionCallOutput {
            kind: "function_call_output",
            call_id: call_id.into(),
            output: output.into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseBody {
    pub id: String,
    #[serde(default)]
    pub output: Vec<OutputItem>,
    pub error: Option<ApiError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputItem {
    Message {
        #[serde(default)]
        content: Vec<ContentPart>,
    },
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    OutputText {
        text: String,
    },
    Text {
        text: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Clone)]
pub struct XaiClient {
    http: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl XaiClient {
    pub fn new(cfg: &Config) -> Result<Self> {
        let http = Client::builder()
            .user_agent(concat!("grok-agent/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .context("failed to build HTTP client")?;

        Ok(Self {
            http,
            api_key: cfg.api_key().to_string(),
            base_url: cfg.base_url.trim_end_matches('/').to_string(),
            model: cfg.model.clone(),
        })
    }

    pub async fn create_response(
        &self,
        input: Vec<InputItem>,
        tools: &[Value],
        instructions: Option<&str>,
        previous_response_id: Option<&str>,
    ) -> Result<ResponseBody> {
        let mut body = json!({
            "model": self.model,
            "input": input,
            "tools": tools,
            "parallel_tool_calls": true,
        });

        if let Some(instructions) = instructions {
            body["instructions"] = Value::String(instructions.to_string());
        }
        if let Some(prev) = previous_response_id {
            body["previous_response_id"] = Value::String(prev.to_string());
        }

        let url = format!("{}/responses", self.base_url);
        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("request failed: POST {url}"))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read response body")?;

        if !status.is_success() {
            bail!("xAI API error ({status}): {text}");
        }

        let parsed: ResponseBody =
            serde_json::from_str(&text).with_context(|| format!("invalid JSON response: {text}"))?;

        if let Some(err) = &parsed.error {
            bail!(
                "xAI returned error: {}",
                err.message.as_deref().unwrap_or("unknown error")
            );
        }

        Ok(parsed)
    }
}

impl ResponseBody {
    pub fn text(&self) -> String {
        let mut parts = Vec::new();
        for item in &self.output {
            if let OutputItem::Message { content, .. } = item {
                for part in content {
                    match part {
                        ContentPart::OutputText { text } | ContentPart::Text { text } => {
                            parts.push(text.clone());
                        }
                        ContentPart::Other => {}
                    }
                }
            }
        }
        parts.join("\n")
    }

    pub fn function_calls(&self) -> Vec<(String, String, String)> {
        self.output
            .iter()
            .filter_map(|item| match item {
                OutputItem::FunctionCall {
                    call_id,
                    name,
                    arguments,
                } => Some((call_id.clone(), name.clone(), arguments.clone())),
                _ => None,
            })
            .collect()
    }
}
