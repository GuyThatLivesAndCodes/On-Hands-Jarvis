// Tool catalog and executor for the Grok agent loop.
//
// Tools are advertised to the model via `ToolDef`s (OpenAI-compatible
// JSON-schema-shaped function definitions). The set of advertised tools
// depends on the user's autonomy safeguards — a tool that requires
// `allow_input_control` for example only appears when that flag is on.
//
// Read-only tools (`read_system_info`, `read_qr_codes`) are always
// available so Grok can answer "what's on my screen / how busy is my
// machine" questions without any extra permissions.

use serde_json::{json, Value};

use crate::ai::grok::{FunctionDef, ToolDef};
use crate::automation::system::SystemSnapshot;
use crate::config::Autonomy;
use crate::qr::ScannedCode;

/// Snapshot of the local state and permissions the agent loop sees. Held
/// by the spawned tokio task so tool dispatch doesn't need to reach back
/// into the UI thread.
#[derive(Clone)]
pub struct AgentContext {
    pub system: SystemSnapshot,
    pub qr_codes: Vec<ScannedCode>,
    pub autonomy: Autonomy,
    pub wake_word: String,
}

/// Returns the OpenAI-compatible tool definitions advertised to Grok,
/// filtered to only the actions the user has authorized.
pub fn available_tools(autonomy: &Autonomy) -> Vec<ToolDef> {
    let mut tools = vec![read_system_info(), read_qr_codes()];
    if autonomy.allow_web_browsing {
        tools.push(open_url());
    }
    if autonomy.allow_app_launch {
        tools.push(launch_app());
    }
    if autonomy.allow_input_control {
        tools.push(move_mouse());
        tools.push(click_mouse());
        tools.push(type_text());
    }
    tools
}

/// A short human-readable summary of the tool catalog, suitable for
/// embedding in the system prompt so Grok knows *what* it can do
/// without re-reading the schema for each call.
pub fn catalog_summary(autonomy: &Autonomy) -> String {
    let mut lines = vec![
        "- read_system_info(): live CPU, memory, top processes.".to_string(),
        "- read_qr_codes(): QR codes currently visible on the user's screen.".to_string(),
    ];
    if autonomy.allow_web_browsing {
        lines.push("- open_url(url): open a URL in the user's default browser.".into());
    }
    if autonomy.allow_app_launch {
        lines.push("- launch_app(spec): start an application by name or path.".into());
    }
    if autonomy.allow_input_control {
        lines.push("- move_mouse(x, y): move the mouse cursor to absolute screen coordinates.".into());
        lines.push("- click_mouse(button=\"left\"|\"right\"): click the mouse.".into());
        lines.push("- type_text(text): type text into the focused window.".into());
    }
    if lines.len() == 2 {
        lines.push(
            "(All write/control tools are disabled in Settings; if the user asks you \
             to do one, suggest enabling the relevant safeguard.)"
                .into(),
        );
    }
    lines.join("\n")
}

/// Run a single tool call. Always returns a JSON value — successful
/// payloads on success, an `{error: "..."}` object on failure or
/// permission denial. The agent loop stringifies whatever we return and
/// feeds it back to Grok as a `Role::Tool` message.
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
                "properties": {
                    "text": {"type": "string"}
                },
                "required": ["text"],
                "additionalProperties": false
            }),
        },
    }
}

