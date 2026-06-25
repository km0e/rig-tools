//! Filesystem tools for rig agents.

use std::path::{Path, PathBuf};

use crate::error::ToolsError;

pub mod edit;
pub mod glob;
pub mod grep;
pub mod list_directory;
pub mod read;

pub use edit::{EditFileArgs, EditFileOutput, EditFileTool};
pub use glob::{GlobArgs, GlobOutput, GlobTool};
pub use grep::{GrepArgs, GrepMatch, GrepOutput, GrepTool};
pub use list_directory::{DirEntry, ListDirectoryArgs, ListDirectoryOutput, ListDirectoryTool};
pub use read::{ReadFileArgs, ReadFileOutput, ReadFileTool};

/// A validated working directory. All filesystem tools resolve paths relative to
/// this directory and reject paths that escape it.
#[derive(Clone, Debug)]
pub struct WorkingDirectory {
    root: PathBuf,
}

impl WorkingDirectory {
    /// Create a new working directory. The path is canonicalized if it exists.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, ToolsError> {
        let path = path.into();
        let root = if path.exists() {
            std::fs::canonicalize(&path)?
        } else {
            path
        };
        Ok(Self { root })
    }

    /// Resolve a relative path against the working directory and verify that it
    /// does not escape.
    pub fn resolve(&self, input: impl AsRef<Path>) -> Result<PathBuf, ToolsError> {
        let input = input.as_ref();

        if input.is_absolute() {
            return Err(ToolsError::AbsolutePath {
                path: input.to_path_buf(),
            });
        }

        // Reject components that go above the root explicitly.
        for component in input.components() {
            if matches!(component, std::path::Component::ParentDir) {
                return Err(ToolsError::PathEscapesWorkingDirectory {
                    path: input.to_path_buf(),
                });
            }
        }

        let resolved = self.root.join(input);

        // Canonicalize the resolved path if possible. If the final component
        // does not exist, canonicalize the deepest existing prefix and append
        // the remainder. This catches symlink escapes in parent directories.
        let normalized = match std::fs::canonicalize(&resolved) {
            Ok(canonical) => canonical,
            Err(_) => {
                // Find the deepest existing ancestor of the resolved path.
                let mut prefix = resolved.clone();
                while !prefix.exists() {
                    if !prefix.pop() {
                        break;
                    }
                }

                let canonical_prefix = std::fs::canonicalize(&prefix).map_err(|_| {
                    ToolsError::PathEscapesWorkingDirectory {
                        path: input.to_path_buf(),
                    }
                })?;

                if !canonical_prefix.starts_with(&self.root) {
                    return Err(ToolsError::PathEscapesWorkingDirectory {
                        path: input.to_path_buf(),
                    });
                }

                // The suffix is the part of the original resolved path after the
                // existing prefix.
                let suffix = resolved
                    .strip_prefix(&prefix)
                    .unwrap_or(std::path::Path::new(""));
                canonical_prefix.join(suffix)
            }
        };

        // Ensure normalized path starts with root.
        if !normalized.starts_with(&self.root) {
            return Err(ToolsError::PathEscapesWorkingDirectory {
                path: input.to_path_buf(),
            });
        }

        Ok(normalized)
    }

    /// Return the root path.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl Default for WorkingDirectory {
    fn default() -> Self {
        Self {
            root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolves_relative_path() {
        let dir = TempDir::new().unwrap();
        let wd = WorkingDirectory::new(dir.path()).unwrap();
        let resolved = wd.resolve("foo/bar.txt").unwrap();
        assert_eq!(resolved, dir.path().join("foo/bar.txt"));
    }

    #[test]
    fn rejects_absolute_path() {
        let dir = TempDir::new().unwrap();
        let wd = WorkingDirectory::new(dir.path()).unwrap();
        assert!(matches!(
            wd.resolve("/etc/passwd"),
            Err(ToolsError::AbsolutePath { .. })
        ));
    }

    #[test]
    fn rejects_path_with_parent_dir() {
        let dir = TempDir::new().unwrap();
        let wd = WorkingDirectory::new(dir.path()).unwrap();
        assert!(matches!(
            wd.resolve("../secret.txt"),
            Err(ToolsError::PathEscapesWorkingDirectory { .. })
        ));
    }

    #[test]
    fn rejects_symlink_escape() {
        let dir = TempDir::new().unwrap();
        let wd = WorkingDirectory::new(dir.path()).unwrap();

        // Create a symlink inside the working dir pointing outside.
        let target = TempDir::new().unwrap();
        let link = dir.path().join("escape");
        #[cfg(unix)]
        std::os::unix::fs::symlink(target.path(), &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(target.path(), &link).unwrap();

        // Attempting to resolve through the symlink should detect the escape.
        assert!(matches!(
            wd.resolve("escape/secret.txt"),
            Err(ToolsError::PathEscapesWorkingDirectory { .. })
        ));
    }
}
