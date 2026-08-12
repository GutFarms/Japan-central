use anyhow::{Context, Result};
use std::fs;
use walkdir::WalkDir;

use super::ToolRuntime;

pub fn list_dir(runtime: &ToolRuntime, path: &str) -> Result<String> {
    let dir = runtime.resolve_path(path)?;
    if !dir.is_dir() {
        anyhow::bail!("{} is not a directory", path);
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let entry = entry?;
        let meta = entry.metadata()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let kind = if meta.is_dir() { "dir" } else { "file" };
        let size = if meta.is_file() {
            format!(" {}B", meta.len())
        } else {
            String::new()
        };
        entries.push(format!("{kind:>4}  {name}{size}"));
    }
    entries.sort();
    if entries.is_empty() {
        Ok("(empty)".into())
    } else {
        Ok(entries.join("\n"))
    }
}

pub fn read_file(
    runtime: &ToolRuntime,
    path: &str,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<String> {
    let file = runtime.resolve_path(path)?;
    let content = fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
    let lines: Vec<&str> = content.lines().collect();
    let start = offset.unwrap_or(1).saturating_sub(1);
    if start >= lines.len() {
        return Ok(format!(
            "(file has {} lines; offset {} is past end)",
            lines.len(),
            start + 1
        ));
    }
    let end = limit
        .map(|n| (start + n).min(lines.len()))
        .unwrap_or(lines.len());
    let mut out = String::new();
    for (idx, line) in lines[start..end].iter().enumerate() {
        out.push_str(&format!("{:>6}|{}\n", start + idx + 1, line));
    }
    Ok(out)
}

pub async fn write_file(runtime: &ToolRuntime, path: &str, content: &str) -> Result<String> {
    let file = runtime.resolve_path(path)?;
    if let Some(parent) = file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&file, content)
        .await
        .with_context(|| format!("write {}", file.display()))?;
    Ok(format!(
        "Wrote {} bytes to {}",
        content.len(),
        file.strip_prefix(runtime.workspace())
            .unwrap_or(&file)
            .display()
    ))
}

pub async fn edit_file(
    runtime: &ToolRuntime,
    path: &str,
    old: &str,
    new: &str,
) -> Result<String> {
    let file = runtime.resolve_path(path)?;
    let content = tokio::fs::read_to_string(&file)
        .await
        .with_context(|| format!("read {}", file.display()))?;
    let matches = content.matches(old).count();
    if matches == 0 {
        anyhow::bail!("old_string not found in {path}");
    }
    if matches > 1 {
        anyhow::bail!("old_string found {matches} times in {path}; make it unique");
    }
    let updated = content.replacen(old, new, 1);
    tokio::fs::write(&file, &updated)
        .await
        .with_context(|| format!("write {}", file.display()))?;
    Ok(format!("Updated {path}"))
}

pub fn glob_files(runtime: &ToolRuntime, pattern: &str) -> Result<String> {
    let walker = WalkDir::new(runtime.workspace())
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(name == "target" || name == ".git" || name == "node_modules")
        });

    let mut matches = Vec::new();
    for entry in walker {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(runtime.workspace())
            .unwrap_or(entry.path());
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if glob::Pattern::new(pattern)
            .with_context(|| format!("invalid glob: {pattern}"))?
            .matches(&rel_str)
        {
            matches.push(rel_str);
        }
        if matches.len() >= 200 {
            matches.push("...[truncated after 200 matches]".into());
            break;
        }
    }
    matches.sort();
    if matches.is_empty() {
        Ok("(no matches)".into())
    } else {
        Ok(matches.join("\n"))
    }
}
