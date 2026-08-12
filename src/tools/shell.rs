use anyhow::{Context, Result};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use super::ToolRuntime;

pub async fn run_command(runtime: &ToolRuntime, command: &str) -> Result<String> {
    let command = command.trim();
    if command.is_empty() {
        anyhow::bail!("command must not be empty");
    }

    let child = Command::new("bash")
        .arg("-lc")
        .arg(command)
        .current_dir(runtime.workspace())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("failed to spawn shell")?;

    let output = timeout(
        Duration::from_secs(runtime.shell_timeout_secs),
        child.wait_with_output(),
    )
    .await
    .context("command timed out")?
    .context("failed waiting for command")?;

    let mut text = String::new();
    text.push_str(&format!("exit_code: {}\n", output.status.code().unwrap_or(-1)));
    if !output.stdout.is_empty() {
        text.push_str("stdout:\n");
        text.push_str(&String::from_utf8_lossy(&output.stdout));
        if !text.ends_with('\n') {
            text.push('\n');
        }
    }
    if !output.stderr.is_empty() {
        text.push_str("stderr:\n");
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        if !text.ends_with('\n') {
            text.push('\n');
        }
    }
    if output.stdout.is_empty() && output.stderr.is_empty() {
        text.push_str("(no output)\n");
    }
    Ok(text)
}
