//! Grep tool for rig agents.

use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::fs;

use crate::error::ToolsError;
use crate::fs::WorkingDirectory;

/// Arguments for the `grep` tool.
#[derive(Deserialize, Debug)]
pub struct GrepArgs {
    /// Regex pattern to search for.
    pub pattern: String,
    /// Optional relative path to a file or directory. If omitted, searches the working directory recursively.
    pub path: Option<String>,
}

/// A single matching line.
#[derive(Serialize, Debug)]
pub struct GrepMatch {
    pub path: String,
    pub line: usize,
    pub text: String,
}

/// Output of the `grep` tool.
#[derive(Serialize, Debug)]
pub struct GrepOutput {
    pub pattern: String,
    pub matches: Vec<GrepMatch>,
}

/// Tool that searches file contents with a regex.
#[derive(Clone, Debug)]
pub struct GrepTool {
    working_dir: WorkingDirectory,
}

impl GrepTool {
    /// Create a new grep tool bound to a working directory.
    pub fn new(working_dir: WorkingDirectory) -> Self {
        Self { working_dir }
    }
}

impl Tool for GrepTool {
    const NAME: &'static str = "grep";

    type Error = ToolsError;
    type Args = GrepArgs;
    type Output = GrepOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Search text files for lines matching a regular expression.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regular expression to search for."
                    },
                    "path": {
                        "type": "string",
                        "description": "Optional relative file or directory path. Defaults to the working directory."
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let re = regex::Regex::new(&args.pattern)?;

        let start = match args.path {
            Some(p) => self.working_dir.resolve(&p)?,
            None => self.working_dir.root().to_path_buf(),
        };

        let mut matches = Vec::new();

        if start.is_file() {
            search_file(&start, &re, self.working_dir.root(), &mut matches).await?;
        } else if start.is_dir() {
            search_dir_iter(&start, &re, self.working_dir.root(), &mut matches).await?;
        }

        Ok(GrepOutput {
            pattern: args.pattern,
            matches,
        })
    }
}

async fn search_file(
    path: &std::path::Path,
    re: &regex::Regex,
    root: &std::path::Path,
    matches: &mut Vec<GrepMatch>,
) -> Result<(), ToolsError> {
    let content = fs::read_to_string(path).await?;
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    for (idx, line) in content.lines().enumerate() {
        if re.is_match(line) {
            matches.push(GrepMatch {
                path: relative.clone(),
                line: idx + 1,
                text: line.to_string(),
            });
        }
    }

    Ok(())
}

async fn search_dir_iter(
    root_dir: &std::path::Path,
    re: &regex::Regex,
    root: &std::path::Path,
    matches: &mut Vec<GrepMatch>,
) -> Result<(), ToolsError> {
    let mut dirs = vec![root_dir.to_path_buf()];

    while let Some(dir) = dirs.pop() {
        let mut entries = fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                search_file(&path, re, root, matches).await?;
            } else if path.is_dir() {
                dirs.push(path);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    async fn greps_file() {
        let dir = TempDir::new().unwrap();
        std::fs::File::create(dir.path().join("a.txt"))
            .unwrap()
            .write_all(b"foo\nbar\nbaz\n")
            .unwrap();

        let tool = GrepTool::new(WorkingDirectory::new(dir.path()).unwrap());
        let output = tool
            .call(GrepArgs {
                pattern: "ba.*".to_string(),
                path: None,
            })
            .await
            .unwrap();

        assert_eq!(output.matches.len(), 2);
        assert_eq!(output.matches[0].line, 2);
        assert_eq!(output.matches[1].line, 3);
    }
}
