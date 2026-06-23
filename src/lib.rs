//! # rig-tools
//!
//! Basic filesystem and shell tools for the [`rig`](https://crates.io/crates/rig-core)
//! LLM agent framework.
//!
//! Tools can be registered directly with a Rig [`ToolSet`] and used by agents.
//!
//! ## Example
//!
//! ```rust,no_run
//! use rig_core::tool::ToolSet;
//! use rig_tools::{default_toolset, BashConfig};
//!
//! # fn main() {
//! let bash_config = BashConfig::new("/tmp/agent-workspace");
//! let toolset: ToolSet = default_toolset("/tmp/agent-workspace", bash_config);
//! # }
//! ```

pub mod bash;
pub mod error;
pub mod fs;

pub use bash::{BashArgs, BashConfig, BashOutput, BashTool};
pub use error::ToolsError;
pub use fs::{
    EditFileArgs, EditFileOutput, EditFileTool, GlobArgs, GlobOutput, GlobTool, GrepArgs,
    GrepMatch, GrepOutput, GrepTool, ReadFileArgs, ReadFileOutput, ReadFileTool, WorkingDirectory,
};

use rig_core::tool::ToolSet;

/// Convenience constructor that returns a [`ToolSet`] pre-loaded with all tools.
///
/// The `working_dir` parameter becomes the root for all filesystem tools and the
/// default working directory for the bash tool.
pub fn default_toolset(
    working_dir: impl Into<std::path::PathBuf>,
    bash_config: BashConfig,
) -> ToolSet {
    let working_dir = WorkingDirectory::new(working_dir.into()).unwrap_or_default();

    let mut toolset = ToolSet::default();
    toolset.add_tool(ReadFileTool::new(working_dir.clone()));
    toolset.add_tool(EditFileTool::new(working_dir.clone()));
    toolset.add_tool(GlobTool::new(working_dir.clone()));
    toolset.add_tool(GrepTool::new(working_dir.clone()));
    toolset.add_tool(BashTool::new(bash_config));
    toolset
}

/// Convenience constructor that returns a [`rig_core::tool::ToolSetBuilder`] pre-loaded with all tools.
pub fn default_toolset_builder(
    working_dir: impl Into<std::path::PathBuf>,
    bash_config: BashConfig,
) -> rig_core::tool::ToolSetBuilder {
    let working_dir = WorkingDirectory::new(working_dir.into()).unwrap_or_default();

    rig_core::tool::ToolSet::builder()
        .static_tool(ReadFileTool::new(working_dir.clone()))
        .static_tool(EditFileTool::new(working_dir.clone()))
        .static_tool(GlobTool::new(working_dir.clone()))
        .static_tool(GrepTool::new(working_dir.clone()))
        .static_tool(BashTool::new(bash_config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_toolset_contains_all_tools() {
        let dir = TempDir::new().unwrap();
        let toolset = default_toolset(dir.path(), BashConfig::new(dir.path()));

        assert!(toolset.contains("read"));
        assert!(toolset.contains("edit"));
        assert!(toolset.contains("glob"));
        assert!(toolset.contains("grep"));
        assert!(toolset.contains("bash"));
    }
}
