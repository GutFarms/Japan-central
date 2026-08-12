mod filesystem;
mod shell;

use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use crate::client::FunctionTool;
use crate::config::Config;

#[derive(Clone)]
pub struct ToolRuntime {
    workspace: PathBuf,
    shell_timeout_secs: u64,
    output_limit: usize,
}

impl ToolRuntime {
    pub fn new(cfg: &Config) -> Self {
        Self {
            workspace: cfg.workspace.clone(),
            shell_timeout_secs: cfg.shell_timeout_secs,
            output_limit: cfg.tool_output_limit,
        }
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub fn definitions(&self, web_search: bool, code_interpreter: bool) -> Vec<Value> {
        let mut tools = vec![
            FunctionTool::new(
                "list_dir",
                "List files and directories under a path relative to the workspace.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative directory path. Use \".\" for workspace root."
                        }
                    },
                    "required": ["path"]
                }),
            ),
            FunctionTool::new(
                "read_file",
                "Read a UTF-8 text file from the workspace.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative file path within the workspace."
                        },
                        "offset": {
                            "type": "integer",
                            "description": "Optional 1-based starting line."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Optional max number of lines to return."
                        }
                    },
                    "required": ["path"]
                }),
            ),
            FunctionTool::new(
                "write_file",
                "Create or overwrite a UTF-8 text file inside the workspace. Creates parent directories.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative file path within the workspace."
                        },
                        "content": {
                            "type": "string",
                            "description": "Full file contents to write."
                        }
                    },
                    "required": ["path", "content"]
                }),
            ),
            FunctionTool::new(
                "edit_file",
                "Replace the first occurrence of old_string with new_string in a workspace file.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative file path within the workspace."
                        },
                        "old_string": {
                            "type": "string",
                            "description": "Exact text to find."
                        },
                        "new_string": {
                            "type": "string",
                            "description": "Replacement text."
                        }
                    },
                    "required": ["path", "old_string", "new_string"]
                }),
            ),
            FunctionTool::new(
                "glob_files",
                "Find files under the workspace matching a glob pattern.",
                json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Glob pattern, e.g. \"**/*.rs\" or \"src/**/*.toml\"."
                        }
                    },
                    "required": ["pattern"]
                }),
            ),
            FunctionTool::new(
                "run_command",
                "Run a shell command inside the workspace. Prefer non-interactive commands.",
                json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Shell command to execute."
                        }
                    },
                    "required": ["command"]
                }),
            ),
        ]
        .into_iter()
        .map(|t| serde_json::to_value(t).expect("tool serializes"))
        .collect::<Vec<_>>();

        if web_search {
            tools.push(json!({ "type": "web_search" }));
        }
        if code_interpreter {
            tools.push(json!({ "type": "code_interpreter" }));
        }

        tools
    }

    pub async fn execute(&self, name: &str, arguments: &str) -> String {
        let result = self.execute_inner(name, arguments).await;
        match result {
            Ok(output) => truncate(&output, self.output_limit),
            Err(err) => format!("ERROR: {err:#}"),
        }
    }

    async fn execute_inner(&self, name: &str, arguments: &str) -> Result<String> {
        let args: Value = serde_json::from_str(arguments)
            .map_err(|e| anyhow::anyhow!("invalid tool arguments JSON: {e}"))?;

        match name {
            "list_dir" => {
                let path = args_string(&args, "path")?;
                filesystem::list_dir(self, &path)
            }
            "read_file" => {
                let path = args_string(&args, "path")?;
                let offset = args_usize(&args, "offset");
                let limit = args_usize(&args, "limit");
                filesystem::read_file(self, &path, offset, limit)
            }
            "write_file" => {
                let path = args_string(&args, "path")?;
                let content = args_string(&args, "content")?;
                filesystem::write_file(self, &path, &content).await
            }
            "edit_file" => {
                let path = args_string(&args, "path")?;
                let old = args_string(&args, "old_string")?;
                let new = args_string(&args, "new_string")?;
                filesystem::edit_file(self, &path, &old, &new).await
            }
            "glob_files" => {
                let pattern = args_string(&args, "pattern")?;
                filesystem::glob_files(self, &pattern)
            }
            "run_command" => {
                let command = args_string(&args, "command")?;
                shell::run_command(self, &command).await
            }
            other => Err(anyhow::anyhow!("unknown tool: {other}")),
        }
    }

    pub fn resolve_path(&self, relative: &str) -> Result<PathBuf> {
        let rel = Path::new(relative);
        if rel.is_absolute() {
            anyhow::bail!("path must be relative to the workspace: {relative}");
        }
        if rel
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            // Allow .. only after canonicalize verification below.
        }

        let joined = self.workspace.join(rel);
        let canonical = if joined.exists() {
            std::fs::canonicalize(&joined)?
        } else {
            let parent = joined
                .parent()
                .ok_or_else(|| anyhow::anyhow!("invalid path: {relative}"))?;
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
            let parent_canon = std::fs::canonicalize(parent)?;
            let file_name = joined
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("invalid path: {relative}"))?;
            parent_canon.join(file_name)
        };

        if !canonical.starts_with(&self.workspace) {
            anyhow::bail!("path escapes workspace: {relative}");
        }
        Ok(canonical)
    }
}

fn args_string<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing string argument: {key}"))
}

fn args_usize(args: &Value, key: &str) -> Option<usize> {
    args.get(key).and_then(|v| v.as_u64()).map(|n| n as usize)
}

fn truncate(s: &str, limit: usize) -> String {
    if s.chars().count() <= limit {
        return s.to_string();
    }
    let truncated: String = s.chars().take(limit).collect();
    format!("{truncated}\n\n...[truncated, showing first {limit} chars]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_workspace_escape() {
        let tmp = std::env::temp_dir().join(format!("grok-agent-test-{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();
        let runtime = ToolRuntime {
            workspace: fs::canonicalize(&tmp).unwrap(),
            shell_timeout_secs: 5,
            output_limit: 1000,
        };
        let err = runtime.resolve_path("../outside.txt").unwrap_err();
        assert!(err.to_string().contains("escapes workspace"));
        let _ = fs::remove_dir_all(&tmp);
    }
}
