//! Directory listing tool for rig agents.

use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::fs;

use crate::error::ToolsError;
use crate::fs::WorkingDirectory;

const DEFAULT_MAX_DEPTH: usize = 3;
const MAX_ENTRIES_LIMIT: usize = 1000;

/// Arguments for the `list_directory` tool.
#[derive(Deserialize, Debug)]
pub struct ListDirectoryArgs {
    /// Relative path to the directory. Defaults to the working directory.
    pub path: Option<String>,
    /// Whether to list recursively. Defaults to false.
    pub recursive: Option<bool>,
    /// Maximum recursion depth when recursive is true. Defaults to 3.
    pub max_depth: Option<usize>,
    /// Optional glob pattern to filter entry names.
    pub pattern: Option<String>,
}

/// A single directory entry.
#[derive(Serialize, Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size: Option<u64>,
}

/// Output of the `list_directory` tool.
#[derive(Serialize, Debug)]
pub struct ListDirectoryOutput {
    pub path: String,
    pub entries: Vec<DirEntry>,
}

/// Tool that lists directory contents.
#[derive(Clone, Debug)]
pub struct ListDirectoryTool {
    working_dir: WorkingDirectory,
}

impl ListDirectoryTool {
    /// Create a new list_directory tool bound to a working directory.
    pub fn new(working_dir: WorkingDirectory) -> Self {
        Self { working_dir }
    }
}

impl Tool for ListDirectoryTool {
    const NAME: &'static str = "list_directory";

    type Error = ToolsError;
    type Args = ListDirectoryArgs;
    type Output = ListDirectoryOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "List the contents of a directory. Paths are relative to the working directory."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the directory. Defaults to the working directory."
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Whether to list recursively. Defaults to false."
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum recursion depth when recursive is true. Defaults to 3."
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Optional glob pattern to filter entry names."
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let input_path = args.path.unwrap_or_else(|| ".".to_string());
        let resolved = self.working_dir.resolve(&input_path)?;

        if !resolved.exists() {
            return Err(ToolsError::FileNotFound { path: resolved });
        }

        if !resolved.is_dir() {
            return Err(ToolsError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{} is not a directory", input_path),
            )));
        }

        let recursive = args.recursive.unwrap_or(false);
        let max_depth = args.max_depth.unwrap_or(DEFAULT_MAX_DEPTH);
        let pattern = args
            .pattern
            .as_ref()
            .map(|p| glob::Pattern::new(p))
            .transpose()?;

        let entries = if recursive {
            list_recursive(&resolved, &input_path, max_depth, pattern.as_ref()).await?
        } else {
            list_flat(&resolved, &input_path, pattern.as_ref()).await?
        };

        Ok(ListDirectoryOutput {
            path: input_path,
            entries,
        })
    }
}

async fn list_flat(
    dir: &std::path::Path,
    relative_prefix: &str,
    pattern: Option<&glob::Pattern>,
) -> Result<Vec<DirEntry>, ToolsError> {
    let mut entries = Vec::new();
    let mut read_dir = fs::read_dir(dir).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        if let Some(e) = entry_to_dir_entry(&entry, relative_prefix, pattern).await? {
            entries.push(e);
        }
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

async fn list_recursive(
    root: &std::path::Path,
    relative_prefix: &str,
    max_depth: usize,
    pattern: Option<&glob::Pattern>,
) -> Result<Vec<DirEntry>, ToolsError> {
    let mut entries = Vec::new();
    let mut queue: Vec<(std::path::PathBuf, String, usize)> =
        vec![(root.to_path_buf(), relative_prefix.to_string(), 0)];

    while let Some((dir, rel_prefix, depth)) = queue.pop() {
        if depth > max_depth {
            continue;
        }

        let mut read_dir = fs::read_dir(&dir).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            if entries.len() >= MAX_ENTRIES_LIMIT {
                return Ok(entries);
            }

            if let Some(e) = entry_to_dir_entry(&entry, &rel_prefix, pattern).await? {
                let is_dir = e.kind == "directory";
                if is_dir && depth < max_depth {
                    let child_path = dir.join(&e.name);
                    let child_rel = if rel_prefix == "." {
                        e.name.clone()
                    } else {
                        format!("{}/{}", rel_prefix, e.name)
                    };
                    queue.push((child_path, child_rel, depth + 1));
                }
                entries.push(e);
            }
        }
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

async fn entry_to_dir_entry(
    entry: &fs::DirEntry,
    relative_prefix: &str,
    pattern: Option<&glob::Pattern>,
) -> Result<Option<DirEntry>, ToolsError> {
    let name = entry.file_name().to_string_lossy().to_string();

    if let Some(pattern) = pattern {
        if !pattern.matches(&name) {
            return Ok(None);
        }
    }

    let metadata = fs::symlink_metadata(entry.path()).await?;
    let kind = if metadata.is_symlink() {
        "symlink".to_string()
    } else if metadata.is_dir() {
        "directory".to_string()
    } else {
        "file".to_string()
    };

    let size = if metadata.is_file() {
        Some(metadata.len())
    } else {
        None
    };

    let path = if relative_prefix == "." {
        name.clone()
    } else {
        format!("{}/{}", relative_prefix, name)
    };

    Ok(Some(DirEntry {
        name,
        path,
        kind,
        size,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn lists_flat_directory() {
        let dir = TempDir::new().unwrap();
        std::fs::File::create(dir.path().join("a.txt")).unwrap();
        std::fs::File::create(dir.path().join("b.rs")).unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();

        let tool = ListDirectoryTool::new(WorkingDirectory::new(dir.path()).unwrap());
        let output = tool
            .call(ListDirectoryArgs {
                path: None,
                recursive: None,
                max_depth: None,
                pattern: None,
            })
            .await
            .unwrap();

        assert_eq!(output.entries.len(), 3);
        let names: Vec<String> = output.entries.iter().map(|e| e.name.clone()).collect();
        assert!(names.contains(&"a.txt".to_string()));
        assert!(names.contains(&"b.rs".to_string()));
        assert!(names.contains(&"sub".to_string()));
    }

    #[tokio::test]
    async fn lists_recursively() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("sub/nested")).unwrap();
        std::fs::File::create(dir.path().join("sub/nested/c.txt")).unwrap();

        let tool = ListDirectoryTool::new(WorkingDirectory::new(dir.path()).unwrap());
        let output = tool
            .call(ListDirectoryArgs {
                path: None,
                recursive: Some(true),
                max_depth: Some(3),
                pattern: None,
            })
            .await
            .unwrap();

        let paths: Vec<String> = output.entries.iter().map(|e| e.path.clone()).collect();
        assert!(paths.contains(&"sub".to_string()));
        assert!(paths.contains(&"sub/nested".to_string()));
        assert!(paths.contains(&"sub/nested/c.txt".to_string()));
    }

    #[tokio::test]
    async fn filters_by_pattern() {
        let dir = TempDir::new().unwrap();
        std::fs::File::create(dir.path().join("a.txt")).unwrap();
        std::fs::File::create(dir.path().join("b.rs")).unwrap();
        std::fs::File::create(dir.path().join("c.rs")).unwrap();

        let tool = ListDirectoryTool::new(WorkingDirectory::new(dir.path()).unwrap());
        let output = tool
            .call(ListDirectoryArgs {
                path: None,
                recursive: None,
                max_depth: None,
                pattern: Some("*.rs".to_string()),
            })
            .await
            .unwrap();

        assert_eq!(output.entries.len(), 2);
        assert!(output.entries.iter().all(|e| e.name.ends_with(".rs")));
    }

    #[tokio::test]
    async fn rejects_path_escape() {
        let dir = TempDir::new().unwrap();
        let tool = ListDirectoryTool::new(WorkingDirectory::new(dir.path()).unwrap());
        let result = tool
            .call(ListDirectoryArgs {
                path: Some("../escape".to_string()),
                recursive: None,
                max_depth: None,
                pattern: None,
            })
            .await;
        assert!(matches!(
            result,
            Err(ToolsError::PathEscapesWorkingDirectory { .. })
        ));
    }
}
