// Tool catalog and executor for the Grok agent loop.
//
// Read-only tools (`read_system_info`, `read_qr_codes`, `list_processes`,
// `read_clipboard`) are always available so Grok can answer "what's on
// my screen / what's running" without extra permissions. Mutating tools
// only appear in the catalog when the matching `Autonomy` flag is on.

use serde_json::{json, Value};
use std::sync::mpsc::Sender;

use crate::ai::grok::{FunctionDef, ToolDef};
use crate::automation::system::SystemSnapshot;
use crate::config::Autonomy;
use crate::qr::ScannedCode;

/// Commands a tool can dispatch back to the UI thread (which holds the
/// `egui::Context` and can call clipboard / repaint APIs).
#[derive(Debug, Clone)]
pub enum UiCommand {
    SetClipboard(String),
}

/// Snapshot of the local state and permissions the agent loop sees.
#[derive(Clone)]
pub struct AgentContext {
    pub system: SystemSnapshot,
    pub qr_codes: Vec<ScannedCode>,
    pub autonomy: Autonomy,
    pub wake_word: String,
    /// Most recently read clipboard text, captured at submit time.
    pub clipboard: Option<String>,
    /// Channel back to the UI thread for things only it can do.
    pub ui_tx: Sender<UiCommand>,
}

pub fn available_tools(autonomy: &Autonomy) -> Vec<ToolDef> {
    let mut tools = vec![
        read_system_info(),
        read_qr_codes(),
        list_processes(),
        read_clipboard(),
    ];
    if autonomy.allow_screen_capture {
        tools.push(capture_screen());
    }
    if autonomy.allow_web_browsing {
        tools.push(open_url());
    }
    if autonomy.allow_app_launch {
        tools.push(launch_app());
        tools.push(close_app());
    }
    if autonomy.allow_input_control {
        tools.push(move_mouse());
        tools.push(click_mouse());
        tools.push(type_text());
        tools.push(set_clipboard());
    }
    if autonomy.allow_shell_commands {
        tools.push(run_shell_command());
    }
    tools
}

pub fn catalog_summary(autonomy: &Autonomy) -> String {
    let mut lines = vec![
        "- read_system_info(): live CPU/memory/uptime + top processes.".to_string(),
        "- read_qr_codes(): QR codes currently visible on the user's screen.".to_string(),
        "- list_processes(limit?): list running processes by CPU usage.".to_string(),
        "- read_clipboard(): read the current system clipboard text.".to_string(),
    ];
    if autonomy.allow_screen_capture {
        lines.push("- capture_screen(monitor?): save a screenshot to disk and return the path.".into());
    }
    if autonomy.allow_web_browsing {
        lines.push("- open_url(url): open a URL in the user's default browser.".into());
    }
    if autonomy.allow_app_launch {
        lines.push("- launch_app(spec): start an application by name or path.".into());
        lines.push("- close_app(name|pid): terminate a running application.".into());
    }
    if autonomy.allow_input_control {
        lines.push("- move_mouse(x, y): move the cursor to screen coordinates.".into());
        lines.push("- click_mouse(button=\"left\"|\"right\"): click the mouse.".into());
        lines.push("- type_text(text): type text into the focused window.".into());
        lines.push("- set_clipboard(text): write text to the system clipboard.".into());
    }
    if autonomy.allow_shell_commands {
        lines.push("- run_shell_command(command, timeout_secs?): run a shell command and return stdout/stderr.".into());
    }
    if lines.len() == 4 {
        lines.push(
            "(All write/control tools are disabled in Settings; if the user asks you \
             to do one, suggest enabling the relevant safeguard.)"
                .into(),
        );
    }
    lines.join("\n")
}

/// Run a single tool call and return a JSON value to feed back to Grok.
pub fn execute(ctx: &AgentContext, name: &str, args: Value) -> Value {
    match name {
        "read_system_info" => json!(ctx.system),

        "read_qr_codes" => json!(ctx
            .qr_codes
            .iter()
            .map(|c| json!({
                "monitor": c.monitor_index,
                "content": c.content,
                "corners": c.corners,
            }))
            .collect::<Vec<_>>()),

        "list_processes" => {
            let limit = args
                .get("limit")
                .and_then(|v| v.as_i64())
                .map(|v| v as usize)
                .unwrap_or(20);
            json!(ctx.system.top_processes.iter().take(limit).collect::<Vec<_>>())
        }

        "read_clipboard" => match &ctx.clipboard {
            Some(s) => json!({"text": s}),
            None => json!({"text": ""}),
        },

        "capture_screen" => {
            if !ctx.autonomy.allow_screen_capture {
                return permission_denied("allow_screen_capture");
            }
            let monitor = args.get("monitor").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
            match capture_screen_to_tmp(monitor) {
                Ok((path, w, h)) => json!({
                    "ok": true,
                    "path": path,
                    "width": w,
                    "height": h,
                    "monitor": monitor,
                }),
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "open_url" => {
            if !ctx.autonomy.allow_web_browsing {
                return permission_denied("allow_web_browsing");
            }
            let url = match args.get("url").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u,
                _ => return missing_arg("url"),
            };
            match crate::automation::apps::open_url(url) {
                Ok(_) => json!({"ok": true, "opened": url}),
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "launch_app" => {
            if !ctx.autonomy.allow_app_launch {
                return permission_denied("allow_app_launch");
            }
            let spec = match args.get("spec").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s,
                _ => return missing_arg("spec"),
            };
            match crate::automation::apps::launch_app(spec) {
                Ok(_) => json!({"ok": true, "launched": spec}),
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "close_app" => {
            if !ctx.autonomy.allow_app_launch {
                return permission_denied("allow_app_launch");
            }
            let target = args.get("target").and_then(|v| v.as_str()).map(str::to_string);
            let pid = args.get("pid").and_then(|v| v.as_i64()).map(|v| v as u32);
            match crate::automation::system::close_app(target.as_deref(), pid) {
                Ok(killed) => json!({"ok": true, "killed_pids": killed}),
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "move_mouse" => {
            if !ctx.autonomy.allow_input_control {
                return permission_denied("allow_input_control");
            }
            let x = match args.get("x").and_then(|v| v.as_i64()) {
                Some(v) => v as i32,
                None => return missing_arg("x"),
            };
            let y = match args.get("y").and_then(|v| v.as_i64()) {
                Some(v) => v as i32,
                None => return missing_arg("y"),
            };
            match crate::automation::input::Input::new().and_then(|mut i| i.move_mouse(x, y)) {
                Ok(_) => json!({"ok": true, "x": x, "y": y}),
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "click_mouse" => {
            if !ctx.autonomy.allow_input_control {
                return permission_denied("allow_input_control");
            }
            let button = args.get("button").and_then(|v| v.as_str()).unwrap_or("left");
            let res = crate::automation::input::Input::new().and_then(|mut i| match button {
                "right" => i.click_right(),
                _ => i.click_left(),
            });
            match res {
                Ok(_) => json!({"ok": true, "button": button}),
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "type_text" => {
            if !ctx.autonomy.allow_input_control {
                return permission_denied("allow_input_control");
            }
            let text = match args.get("text").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return missing_arg("text"),
            };
            match crate::automation::input::Input::new().and_then(|mut i| i.type_text(text)) {
                Ok(_) => json!({"ok": true, "typed_chars": text.chars().count()}),
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        "set_clipboard" => {
            if !ctx.autonomy.allow_input_control {
                return permission_denied("allow_input_control");
            }
            let text = match args.get("text").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return missing_arg("text"),
            };
            let _ = ctx.ui_tx.send(UiCommand::SetClipboard(text.to_string()));
            json!({"ok": true})
        }

        "run_shell_command" => {
            if !ctx.autonomy.allow_shell_commands {
                return permission_denied("allow_shell_commands");
            }
            let cmd = match args.get("command").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => return missing_arg("command"),
            };
            let timeout = args
                .get("timeout_secs")
                .and_then(|v| v.as_i64())
                .unwrap_or(20)
                .clamp(1, 120) as u64;
            match crate::automation::shell::run(&cmd, std::time::Duration::from_secs(timeout)) {
                Ok(out) => json!({
                    "ok": out.status_success,
                    "exit_code": out.exit_code,
                    "stdout": out.stdout,
                    "stderr": out.stderr,
                }),
                Err(e) => json!({"error": e.to_string()}),
            }
        }

        _ => json!({"error": format!("unknown tool: {name}")}),
    }
}

fn permission_denied(flag: &str) -> Value {
    json!({
        "error": format!(
            "permission denied — the user has not enabled `{flag}` in Settings → Autonomy"
        )
    })
}

fn missing_arg(name: &str) -> Value {
    json!({"error": format!("missing required argument `{name}`")})
}

fn capture_screen_to_tmp(monitor_index: usize) -> anyhow::Result<(String, u32, u32)> {
    use anyhow::Context;
    let monitors = xcap::Monitor::all().context("enumerate monitors")?;
    let monitor = monitors
        .get(monitor_index)
        .ok_or_else(|| anyhow::anyhow!("monitor index {monitor_index} out of range"))?;
    let img = monitor.capture_image().context("capture monitor")?;
    let w = img.width();
    let h = img.height();
    let path = std::env::temp_dir().join(format!(
        "jarvis-screenshot-{}-mon{}.png",
        chrono::Utc::now().format("%Y%m%dT%H%M%S"),
        monitor_index
    ));
    img.save(&path).context("write screenshot")?;
    Ok((path.to_string_lossy().to_string(), w, h))
}

// ---------- tool definitions ---------------------------------------------

fn read_system_info() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "read_system_info".into(),
            description: "Get a live snapshot of the user's machine: CPU usage, memory \
                          usage, uptime, and the top 10 processes by CPU."
                .into(),
            parameters: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
    }
}

fn read_qr_codes() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "read_qr_codes".into(),
            description:
                "List QR codes currently visible on the user's screen. Each entry has a \
                 monitor index, the decoded content (often a URL), and screen-space corners."
                    .into(),
            parameters: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
    }
}

fn list_processes() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "list_processes".into(),
            description: "List running processes ordered by CPU usage.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 20}
                },
                "additionalProperties": false
            }),
        },
    }
}

fn read_clipboard() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "read_clipboard".into(),
            description: "Read the user's current system-clipboard text.".into(),
            parameters: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
    }
}

fn capture_screen() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "capture_screen".into(),
            description: "Save a PNG screenshot of one of the user's monitors to a temp \
                          file and return its path + dimensions.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "monitor": {"type": "integer", "minimum": 0, "default": 0}
                },
                "additionalProperties": false
            }),
        },
    }
}

fn open_url() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "open_url".into(),
            description: "Open a URL in the user's default web browser.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "The URL to open."}
                },
                "required": ["url"],
                "additionalProperties": false
            }),
        },
    }
}

fn launch_app() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "launch_app".into(),
            description: "Launch an application by executable name or absolute path. \
                          Quoted arguments are supported."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "spec": {
                        "type": "string",
                        "description": "Executable to launch (e.g. \"firefox\", \"/usr/bin/code .\")."
                    }
                },
                "required": ["spec"],
                "additionalProperties": false
            }),
        },
    }
}

fn close_app() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "close_app".into(),
            description: "Terminate one or more running processes. Provide either a `target` \
                          name substring or a specific `pid`."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "target": {"type": "string", "description": "Substring of the process name."},
                    "pid":    {"type": "integer", "description": "Specific process id."}
                },
                "additionalProperties": false
            }),
        },
    }
}

fn move_mouse() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "move_mouse".into(),
            description: "Move the mouse cursor to absolute screen coordinates (in pixels).".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer"},
                    "y": {"type": "integer"}
                },
                "required": ["x", "y"],
                "additionalProperties": false
            }),
        },
    }
}

fn click_mouse() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "click_mouse".into(),
            description: "Click the mouse at the current cursor position.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "button": {"type": "string", "enum": ["left", "right"], "default": "left"}
                },
                "additionalProperties": false
            }),
        },
    }
}

fn type_text() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "type_text".into(),
            description: "Type a string of text into the currently focused window.".into(),
            parameters: json!({
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"],
                "additionalProperties": false
            }),
        },
    }
}

fn set_clipboard() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "set_clipboard".into(),
            description: "Write text to the system clipboard.".into(),
            parameters: json!({
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"],
                "additionalProperties": false
            }),
        },
    }
}

fn run_shell_command() -> ToolDef {
    ToolDef {
        kind: "function",
        function: FunctionDef {
            name: "run_shell_command".into(),
            description: "Run a shell command on the user's machine and return stdout, stderr, \
                          and the exit code. Use sparingly — destructive commands can be \
                          undone only by the user."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command":      {"type": "string"},
                    "timeout_secs": {"type": "integer", "minimum": 1, "maximum": 120, "default": 20}
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        },
    }
}
