//! Glob tool for rig agents.

use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::ToolsError;
use crate::fs::WorkingDirectory;

/// Arguments for the `glob` tool.
#[derive(Deserialize, Debug)]
pub struct GlobArgs {
    /// Glob pattern relative to the working directory.
    pub pattern: String,
}

/// Output of the `glob` tool.
#[derive(Serialize, Debug)]
pub struct GlobOutput {
    pub pattern: String,
    pub matches: Vec<String>,
}

/// Tool that finds files matching a glob pattern.
#[derive(Clone, Debug)]
pub struct GlobTool {
    working_dir: WorkingDirectory,
}

impl GlobTool {
    /// Create a new glob tool bound to a working directory.
    pub fn new(working_dir: WorkingDirectory) -> Self {
        Self { working_dir }
    }
}

impl Tool for GlobTool {
    const NAME: &'static str = "glob";

    type Error = ToolsError;
    type Args = GlobArgs;
    type Output = GlobOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Find files matching a glob pattern relative to the working directory."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern, e.g. \"src/**/*.rs\"."
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let pattern = self.working_dir.root().join(&args.pattern);
        let matches: Vec<String> = glob::glob(pattern.to_string_lossy().as_ref())?
            .filter_map(Result::ok)
            .map(|p| {
                p.strip_prefix(self.working_dir.root())
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        Ok(GlobOutput {
            pattern: args.pattern,
            matches,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn finds_files() {
        let dir = TempDir::new().unwrap();
        std::fs::File::create(dir.path().join("a.rs")).unwrap();
        std::fs::File::create(dir.path().join("b.txt")).unwrap();
        std::fs::File::create(dir.path().join("c.rs")).unwrap();

        let tool = GlobTool::new(WorkingDirectory::new(dir.path()).unwrap());
        let output = tool
            .call(GlobArgs {
                pattern: "*.rs".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(output.matches.len(), 2);
        assert!(output.matches.contains(&"a.rs".to_string()));
        assert!(output.matches.contains(&"c.rs".to_string()));
    }
}
