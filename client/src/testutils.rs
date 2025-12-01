use anyhow::Result;
use nix::libc;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tracing::info;

/// Starts the Daml sandbox in the background.
/// Returns Ok(SandboxGuard) if the process starts successfully.
pub async fn daml_start(package_root: PathBuf, sandbox_port: u16) -> Result<SandboxGuard> {
    let sandbox_admin_port = sandbox_port + 1;
    let sequencer_port = sandbox_port + 2;
    let sequencer_admin_port = sandbox_port + 3;
    let mediator_port = sandbox_port + 4;
    let mut child;
    unsafe {
        child = Command::new("daml")
            .args(&[
                "start",
                "--sandbox-port",
                &sandbox_port.to_string(),
                "--sandbox-admin-api-port",
                &sandbox_admin_port.to_string(),
                "--sandbox-sequencer-public-port",
                &sequencer_port.to_string(),
                "--sandbox-sequencer-admin-port",
                &sequencer_admin_port.to_string(),
                "--sandbox-mediator-admin-port",
                &mediator_port.to_string(),
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

    wait_for_sandbox_ready(&mut child)?;
    let guard = SandboxGuard {
        child: Some(child),
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
        if line.contains("The Canton sandbox and JSON API are ready to use.") {
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
        tracing_subscriber::fmt::init();
        let package_root = PathBuf::from("/Users/gyorgybalazsi/rust-client-toolbox/_daml/daml-asset");
        let sandbox_port = 6865;
        let _guard = daml_start(package_root, sandbox_port)
            .await
            .expect("Failed to start sandbox");
        
    }
}
