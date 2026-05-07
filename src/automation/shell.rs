// Run a shell command synchronously with a timeout. We pipe through
// the platform's default shell so things like pipes, redirects, and
// shell builtins work; the agent loop already gates this behind
// `Autonomy::allow_shell_commands`.

use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub status_success: bool,
}

pub fn run(command: &str, timeout: Duration) -> Result<ShellOutput> {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().with_context(|| format!("spawn `{command}`"))?;

    // Poll until the child exits or we hit the timeout.
    let deadline = Instant::now() + timeout;
    let exit = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut s) = child.stdout.take() {
        use std::io::Read;
        let _ = s.read_to_string(&mut stdout);
    }
    if let Some(mut s) = child.stderr.take() {
        use std::io::Read;
        let _ = s.read_to_string(&mut stderr);
    }

    match exit {
        Some(status) => Ok(ShellOutput {
            stdout,
            stderr,
            exit_code: status.code(),
            status_success: status.success(),
        }),
        None => Ok(ShellOutput {
            stdout,
            stderr: format!("(timed out after {}s)\n{stderr}", timeout.as_secs()),
            exit_code: None,
            status_success: false,
        }),
    }
}
