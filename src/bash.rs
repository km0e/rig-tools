//! Bash/shell execution tool for rig agents.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::ToolsError;

/// Configuration for the `bash` tool.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BashConfig {
    /// Working directory for commands.
    pub working_dir: PathBuf,
    /// Optional allow-list of command base names. If `Some`, only these
    /// commands may be executed.
    pub allowed_commands: Option<Vec<String>>,
    /// Maximum time to wait for a command in seconds.
    pub timeout_secs: u64,
    /// Optional environment variable allow-list. If `Some`, only these
    /// variables are forwarded from the current process environment.
    pub env_allowlist: Option<Vec<String>>,
}

impl BashConfig {
    /// Create a permissive config that runs commands in `working_dir` with a
    /// 60-second timeout.
    pub fn new(working_dir: impl Into<PathBuf>) -> Self {
        Self {
            working_dir: working_dir.into(),
            allowed_commands: None,
            timeout_secs: 60,
            env_allowlist: None,
        }
    }

    /// Restrict commands to the given base names.
    pub fn allow_commands(mut self, commands: Vec<String>) -> Self {
        self.allowed_commands = Some(commands);
        self
    }

    /// Restrict forwarded environment variables.
    pub fn allow_env(mut self, vars: Vec<String>) -> Self {
        self.env_allowlist = Some(vars);
        self
    }

    /// Set the timeout in seconds.
    pub fn timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

impl Default for BashConfig {
    fn default() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
}

/// Arguments for the `bash` tool.
#[derive(Deserialize, Debug)]
pub struct BashArgs {
    /// Shell command to execute.
    pub command: String,
}

/// Output of the `bash` tool.
#[derive(Serialize, Debug)]
pub struct BashOutput {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Tool that executes a shell command.
#[derive(Clone, Debug)]
pub struct BashTool {
    config: BashConfig,
}

impl BashTool {
    /// Create a new bash tool with the given config.
    pub fn new(config: BashConfig) -> Self {
        Self { config }
    }

    fn check_allowed(&self, command: &str) -> Result<(), ToolsError> {
        if let Some(ref allowed) = self.config.allowed_commands {
            // Parse the first whitespace-delimited token as the command name.
            let base = command.split_whitespace().next().unwrap_or(command);
            if !allowed.iter().any(|c| c == base) {
                return Err(ToolsError::CommandNotAllowed {
                    command: base.to_string(),
                });
            }
        }
        Ok(())
    }

    fn build_env(&self) -> HashMap<String, String> {
        match &self.config.env_allowlist {
            Some(vars) => std::env::vars().filter(|(k, _)| vars.contains(k)).collect(),
            None => std::env::vars().collect(),
        }
    }
}

impl Tool for BashTool {
    const NAME: &'static str = "bash";

    type Error = ToolsError;
    type Args = BashArgs;
    type Output = BashOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Execute a bash shell command. The command runs in the configured working directory.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.check_allowed(&args.command)?;

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", &args.command]);
            c
        } else {
            let mut c = Command::new("bash");
            c.arg("-c").arg(&args.command);
            c
        };

        cmd.current_dir(&self.config.working_dir)
            .env_clear()
            .envs(self.build_env());

        let result = timeout(Duration::from_secs(self.config.timeout_secs), cmd.output())
            .await
            .map_err(|_| ToolsError::Timeout {
                timeout_secs: self.config.timeout_secs,
            })??;

        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();
        let exit_code = result.status.code().unwrap_or(-1);

        Ok(BashOutput {
            command: args.command,
            exit_code,
            stdout,
            stderr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn echoes_output() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(BashConfig::new(dir.path()));
        let output = tool
            .call(BashArgs {
                command: "echo hello".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout.trim(), "hello");
    }

    #[tokio::test]
    async fn respects_allow_list() {
        let dir = TempDir::new().unwrap();
        let config = BashConfig::new(dir.path()).allow_commands(vec!["ls".to_string()]);
        let tool = BashTool::new(config);

        let result = tool
            .call(BashArgs {
                command: "echo hello".to_string(),
            })
            .await;

        assert!(matches!(
            result,
            Err(ToolsError::CommandNotAllowed { command }) if command == "echo"
        ));
    }

    #[tokio::test]
    async fn captures_stderr_and_exit_code() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(BashConfig::new(dir.path()));
        let output = tool
            .call(BashArgs {
                command: "echo error >&2; exit 42".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(output.exit_code, 42);
        assert_eq!(output.stderr.trim(), "error");
    }
}
