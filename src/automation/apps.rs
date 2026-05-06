// Application launching. Wraps `open` so the OS associates URLs/files
// with the right handler, and falls back to `Command` for arbitrary
// executables.

use anyhow::{anyhow, Context, Result};
use std::process::{Command, Stdio};

pub fn launch_app(spec: &str) -> Result<()> {
    if spec.trim().is_empty() {
        return Err(anyhow!("empty application spec"));
    }
    // If it parses as a URL or an existing path, hand it off to the OS.
    if spec.contains("://") || std::path::Path::new(spec).exists() {
        open::that_detached(spec).with_context(|| format!("open {spec}"))?;
        return Ok(());
    }
    // Otherwise treat as an executable name with optional arguments.
    let mut parts = shell_split(spec);
    let program = parts.remove(0);
    Command::new(&program)
        .args(parts)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("spawn {program}"))?;
    Ok(())
}

pub fn open_url(url: &str) -> Result<()> {
    open::that_detached(url).with_context(|| format!("open url {url}"))?;
    Ok(())
}

/// Tiny shell-style splitter that preserves double-quoted segments. We
/// keep this in-house rather than depending on the `shlex` crate.
fn shell_split(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for c in s.chars() {
        match c {
            '"' => in_quote = !in_quote,
            ' ' | '\t' if !in_quote => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
