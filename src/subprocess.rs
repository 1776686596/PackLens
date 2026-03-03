use crate::error::AdapterError;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
}

pub async fn run_command(
    cmd: &str,
    args: &[&str],
    timeout_secs: u64,
) -> Result<CommandOutput, AdapterError> {
    let result = timeout(
        Duration::from_secs(timeout_secs),
        Command::new(cmd)
            .args(args)
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .kill_on_drop(true)
            .output(),
    )
    .await;

    let output = match result {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return Err(AdapterError::Io(e)),
        Err(_) => {
            return Err(AdapterError::Timeout {
                cmd: cmd.to_string(),
                timeout_secs,
            });
        }
    };

    if !output.status.success() {
        return Err(AdapterError::CommandFailed {
            cmd: cmd.to_string(),
            code: output.status.code().unwrap_or(-1),
        });
    }

    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
