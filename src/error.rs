//! Shared error type for rig-tools.

use std::path::PathBuf;

/// Errors that can be returned by the tools in this crate.
#[derive(Debug, thiserror::Error)]
pub enum ToolsError {
    /// An I/O operation failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A requested file was not found.
    #[error("file not found: {path}")]
    FileNotFound { path: PathBuf },

    /// A path attempted to escape the configured working directory.
    #[error("path escapes the working directory: {path}")]
    PathEscapesWorkingDirectory { path: PathBuf },

    /// An absolute path was provided where a relative path is required.
    #[error("absolute paths are not allowed: {path}")]
    AbsolutePath { path: PathBuf },

    /// A string replacement operation failed because the old text was not found.
    #[error("old text not found in file: {path}")]
    OldTextNotFound { path: PathBuf },

    /// A shell command was denied by the allow-list.
    #[error("command not allowed: {command}")]
    CommandNotAllowed { command: String },

    /// A shell command exceeded its configured timeout.
    #[error("command timed out after {timeout_secs}s")]
    Timeout { timeout_secs: u64 },

    /// A shell command returned a non-zero exit code.
    #[error("command failed with exit code {exit_code}: {stderr}")]
    CommandFailed { exit_code: i32, stderr: String },

    /// A glob pattern was invalid.
    #[error("invalid glob pattern: {0}")]
    InvalidGlob(#[from] glob::PatternError),

    /// A regular expression was invalid.
    #[error("invalid regex: {0}")]
    InvalidRegex(#[from] regex::Error),

    /// A UTF-8 conversion failed.
    #[error("invalid utf-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}
