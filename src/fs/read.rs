//! Read-file tool for rig agents.

use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::fs;

use crate::error::ToolsError;
use crate::fs::WorkingDirectory;

/// Arguments for the `read` tool.
#[derive(Deserialize, Debug)]
pub struct ReadFileArgs {
    /// Relative path to the file to read.
    pub path: String,
    /// Optional 1-based starting line number.
    pub offset: Option<usize>,
    /// Optional maximum number of lines to read.
    pub limit: Option<usize>,
}

/// Output of the `read` tool.
#[derive(Serialize, Debug)]
pub struct ReadFileOutput {
    pub path: String,
    pub content: String,
    pub total_lines: usize,
    pub read_lines: usize,
}

/// Tool that reads the contents of a file.
#[derive(Clone, Debug)]
pub struct ReadFileTool {
    working_dir: WorkingDirectory,
}

impl ReadFileTool {
    /// Create a new read tool bound to a working directory.
    pub fn new(working_dir: WorkingDirectory) -> Self {
        Self { working_dir }
    }
}

impl Tool for ReadFileTool {
    const NAME: &'static str = "read";

    type Error = ToolsError;
    type Args = ReadFileArgs;
    type Output = ReadFileOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Read the contents of a text file. Paths are relative to the working directory."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file to read."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Optional 1-based starting line number."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Optional maximum number of lines to read."
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = self.working_dir.resolve(&args.path)?;

        if !path.exists() {
            return Err(ToolsError::FileNotFound { path });
        }

        let content = fs::read_to_string(&path).await?;
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let offset = args.offset.unwrap_or(1).saturating_sub(1);
        let limit = args.limit.unwrap_or(total_lines);

        let selected: Vec<&str> = lines.into_iter().skip(offset).take(limit).collect();

        let read_lines = selected.len();
        let content = selected.join("\n");

        Ok(ReadFileOutput {
            path: args.path,
            content,
            total_lines,
            read_lines,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    async fn reads_whole_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hello.txt");
        std::fs::File::create(&path)
            .unwrap()
            .write_all(b"line1\nline2\nline3\n")
            .unwrap();

        let tool = ReadFileTool::new(WorkingDirectory::new(dir.path()).unwrap());
        let output = tool
            .call(ReadFileArgs {
                path: "hello.txt".to_string(),
                offset: None,
                limit: None,
            })
            .await
            .unwrap();

        assert_eq!(output.content, "line1\nline2\nline3");
        assert_eq!(output.total_lines, 3);
        assert_eq!(output.read_lines, 3);
    }

    #[tokio::test]
    async fn reads_with_offset_and_limit() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hello.txt");
        std::fs::File::create(&path)
            .unwrap()
            .write_all(b"a\nb\nc\nd\ne\n")
            .unwrap();

        let tool = ReadFileTool::new(WorkingDirectory::new(dir.path()).unwrap());
        let output = tool
            .call(ReadFileArgs {
                path: "hello.txt".to_string(),
                offset: Some(2),
                limit: Some(2),
            })
            .await
            .unwrap();

        assert_eq!(output.content, "b\nc");
        assert_eq!(output.total_lines, 5);
        assert_eq!(output.read_lines, 2);
    }
}
