//! Edit-file tool for rig agents.

use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::fs;

use crate::error::ToolsError;
use crate::fs::WorkingDirectory;

/// Arguments for the `edit` tool.
#[derive(Deserialize, Debug)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum EditFileArgs {
    /// Create or overwrite a file with the provided content.
    Write { path: String, content: String },
    /// Replace the first occurrence of `old_string` with `new_string`.
    StrReplace {
        path: String,
        old_string: String,
        new_string: String,
    },
    /// Insert `content` at the given 1-based line number.
    Insert {
        path: String,
        line: usize,
        content: String,
    },
    /// Delete a file.
    Delete { path: String },
}

/// Output of the `edit` tool.
#[derive(Serialize, Debug)]
pub struct EditFileOutput {
    pub path: String,
    pub command: String,
    pub success: bool,
}

/// Tool that performs create / replace / insert / delete operations on files.
#[derive(Clone, Debug)]
pub struct EditFileTool {
    working_dir: WorkingDirectory,
}

impl EditFileTool {
    /// Create a new edit tool bound to a working directory.
    pub fn new(working_dir: WorkingDirectory) -> Self {
        Self { working_dir }
    }
}

impl Tool for EditFileTool {
    const NAME: &'static str = "edit";

    type Error = ToolsError;
    type Args = EditFileArgs;
    type Output = EditFileOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Edit files in the working directory. Supports write, str_replace, insert, and delete.".to_string(),
            parameters: json!({
                "type": "object",
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {
                            "command": { "enum": ["write"] },
                            "path": { "type": "string" },
                            "content": { "type": "string" }
                        },
                        "required": ["command", "path", "content"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "command": { "enum": ["str_replace"] },
                            "path": { "type": "string" },
                            "old_string": { "type": "string" },
                            "new_string": { "type": "string" }
                        },
                        "required": ["command", "path", "old_string", "new_string"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "command": { "enum": ["insert"] },
                            "path": { "type": "string" },
                            "line": { "type": "integer" },
                            "content": { "type": "string" }
                        },
                        "required": ["command", "path", "line", "content"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "command": { "enum": ["delete"] },
                            "path": { "type": "string" }
                        },
                        "required": ["command", "path"]
                    }
                ]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match args {
            EditFileArgs::Write { path, content } => {
                let resolved = self.working_dir.resolve(&path)?;
                if let Some(parent) = resolved.parent() {
                    fs::create_dir_all(parent).await?;
                }
                fs::write(&resolved, content).await?;
                Ok(EditFileOutput {
                    path,
                    command: "write".to_string(),
                    success: true,
                })
            }
            EditFileArgs::StrReplace {
                path,
                old_string,
                new_string,
            } => {
                let resolved = self.working_dir.resolve(&path)?;
                if !resolved.exists() {
                    return Err(ToolsError::FileNotFound { path: resolved });
                }
                let content = fs::read_to_string(&resolved).await?;
                if !content.contains(&old_string) {
                    return Err(ToolsError::OldTextNotFound { path: resolved });
                }
                let new_content = content.replacen(&old_string, &new_string, 1);
                fs::write(&resolved, new_content).await?;
                Ok(EditFileOutput {
                    path,
                    command: "str_replace".to_string(),
                    success: true,
                })
            }
            EditFileArgs::Insert {
                path,
                line,
                content,
            } => {
                let resolved = self.working_dir.resolve(&path)?;
                if !resolved.exists() {
                    return Err(ToolsError::FileNotFound { path: resolved });
                }
                let existing = fs::read_to_string(&resolved).await?;
                let mut lines: Vec<&str> = existing.lines().collect();
                let idx = line.saturating_sub(1);
                let idx = idx.min(lines.len());
                lines.insert(idx, &content);
                fs::write(&resolved, lines.join("\n")).await?;
                Ok(EditFileOutput {
                    path,
                    command: "insert".to_string(),
                    success: true,
                })
            }
            EditFileArgs::Delete { path } => {
                let resolved = self.working_dir.resolve(&path)?;
                if !resolved.exists() {
                    return Err(ToolsError::FileNotFound { path: resolved });
                }
                fs::remove_file(&resolved).await?;
                Ok(EditFileOutput {
                    path,
                    command: "delete".to_string(),
                    success: true,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_and_read() {
        let dir = tempfile::TempDir::new().unwrap();
        let tool = EditFileTool::new(WorkingDirectory::new(dir.path()).unwrap());

        tool.call(EditFileArgs::Write {
            path: "foo.txt".to_string(),
            content: "hello".to_string(),
        })
        .await
        .unwrap();

        let content = tokio::fs::read_to_string(dir.path().join("foo.txt"))
            .await
            .unwrap();
        assert_eq!(content, "hello");
    }

    #[tokio::test]
    async fn str_replace() {
        let dir = tempfile::TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("a.txt"), "foo bar baz")
            .await
            .unwrap();

        let tool = EditFileTool::new(WorkingDirectory::new(dir.path()).unwrap());
        tool.call(EditFileArgs::StrReplace {
            path: "a.txt".to_string(),
            old_string: "bar".to_string(),
            new_string: "qux".to_string(),
        })
        .await
        .unwrap();

        let content = tokio::fs::read_to_string(dir.path().join("a.txt"))
            .await
            .unwrap();
        assert_eq!(content, "foo qux baz");
    }
}
