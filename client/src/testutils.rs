use anyhow::Result;
use nix::libc;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex, OwnedMutexGuard};
use tracing::info;

/// Every sandbox binds the same fixed ports (ledger API plus Canton's admin APIs),
/// and `cargo test` runs the tests of one binary in parallel. Without serialization,
/// concurrent sandboxes fail with `Failed to bind to address /127.0.0.1:6868`.
fn sandbox_lock() -> Arc<Mutex<()>> {
    static LOCK: OnceLock<Arc<Mutex<()>>> = OnceLock::new();
    LOCK.get_or_init(|| Arc::new(Mutex::new(()))).clone()
}

/// Starts the Daml sandbox in the background.
/// Returns Ok(SandboxGuard) if the process starts successfully.
///
/// Only one sandbox runs at a time per test process: this waits until the previous
/// `SandboxGuard` has been dropped (which kills its sandbox) before starting.
pub async fn start_sandbox(package_root: PathBuf, dar_path: PathBuf, sandbox_port: u16) -> Result<SandboxGuard> {
    let lock = sandbox_lock().lock_owned().await;
    let mut child;
    unsafe {
        child = Command::new("dpm")
            .args(&[
                "sandbox",
                "--dar",
                dar_path.to_str().unwrap(),
                "--ledger-api-port",
                &sandbox_port.to_string(),
            ])
            .current_dir(&package_root)
            .stdout(Stdio::piped())
            .pre_exec(|| {
                // SAFETY: setpgid is required to create a new process group for the child.
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            })
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start sandbox: {}", e))?;
    }

    if let Err(e) = wait_for_sandbox_ready(&mut child) {
        // Do not leak a half-started sandbox: it would keep the ports bound and
        // break every test that runs after this one.
        let _ = close_sandbox(&mut child);
        return Err(e);
    }
    let guard = SandboxGuard {
        child: Some(child),
        _lock: lock,
    };
    Ok(guard)
}

fn wait_for_sandbox_ready(child: &mut Child) -> anyhow::Result<()> {
    let stdout = child
        .stdout
        .as_mut()
        .expect("Failed to capture sandbox stdout");
    info!("Captured sandbox stdout");
    let reader = BufReader::new(stdout);

    for line in reader.lines().take(120) {
        // up to 2 minutes if 1 line/sec
        let line = line?;
        info!("Sandbox stdout line: {}", line); // Optionally log each line
        if line.contains("Canton sandbox is ready.") {
            info!("Sandbox is ready!");
            return Ok(());
        }
    }
    Err(anyhow::anyhow!(
        "Sandbox did not print readiness message in time"
    ))
}

/// Closes the Daml sandbox process.
pub fn close_sandbox(child: &mut Child) -> anyhow::Result<()> {
    let pgid = child.id(); // Process group ID is the PID of the leader
    killpg(Pid::from_raw(pgid as i32), Signal::SIGKILL)
        .map_err(|e| anyhow::anyhow!("Failed to send SIGKILL to sandbox process group: {}", e))?;
    child
        .wait()
        .map_err(|e| anyhow::anyhow!("Failed to wait for sandbox to exit: {}", e))?;
    Ok(())
}

pub struct SandboxGuard {
    pub child: Option<std::process::Child>,
    // Declared after `child`: fields drop in order after `Drop::drop`, so the sandbox
    // is killed before the next test is allowed to start one.
    _lock: OwnedMutexGuard<()>,
}

impl Drop for SandboxGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = close_sandbox(child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_start_and_close_sandbox() {
        let _ = tracing_subscriber::fmt().try_init();
        let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let package_root = PathBuf::from(&crate_root).join("..").join("_daml").join("daml-asset");
        let dar_path = package_root.join("main").join(".daml").join("dist").join("daml-asset-0.0.1.dar");
        let sandbox_port = 6865;
        let _guard = start_sandbox(package_root, dar_path, sandbox_port)
            .await
            .expect("Failed to start sandbox");

    }
}
