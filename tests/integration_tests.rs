use rig_tools::{BashConfig, default_toolset};
use std::io::Write;
use tempfile::TempDir;

#[tokio::test]
async fn toolset_dispatches_read_and_edit() {
    let dir = TempDir::new().unwrap();
    let toolset = default_toolset(dir.path(), BashConfig::new(dir.path()));

    // Create a file via edit tool.
    let create_args = serde_json::json!({
        "command": "write",
        "path": "hello.txt",
        "content": "Hello, world!"
    });
    toolset.call("edit", create_args.to_string()).await.unwrap();

    // Read it back via read tool.
    let read_args = serde_json::json!({
        "path": "hello.txt"
    });
    let result = toolset.call("read", read_args.to_string()).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["content"], "Hello, world!");
}

#[tokio::test]
async fn path_traversal_is_blocked() {
    let dir = TempDir::new().unwrap();
    let toolset = default_toolset(dir.path(), BashConfig::new(dir.path()));

    let args = serde_json::json!({
        "path": "../secret.txt"
    });
    let result = toolset.call("read", args.to_string()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn bash_tool_runs_in_toolset() {
    let dir = TempDir::new().unwrap();
    let toolset = default_toolset(dir.path(), BashConfig::new(dir.path()));

    let args = serde_json::json!({
        "command": "pwd"
    });
    let result = toolset.call("bash", args.to_string()).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["exit_code"], 0);
    assert!(
        parsed["stdout"]
            .as_str()
            .unwrap()
            .contains(dir.path().to_str().unwrap())
    );
}

#[tokio::test]
async fn glob_tool_lists_files() {
    let dir = TempDir::new().unwrap();
    std::fs::File::create(dir.path().join("a.rs")).unwrap();
    std::fs::File::create(dir.path().join("b.txt")).unwrap();

    let toolset = default_toolset(dir.path(), BashConfig::new(dir.path()));
    let args = serde_json::json!({
        "pattern": "*.rs"
    });
    let result = toolset.call("glob", args.to_string()).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["matches"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["matches"][0], "a.rs");
}

#[tokio::test]
async fn list_directory_tool_lists_recursively() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("src/fs")).unwrap();
    std::fs::File::create(dir.path().join("src/lib.rs")).unwrap();
    std::fs::File::create(dir.path().join("src/fs/mod.rs")).unwrap();

    let toolset = default_toolset(dir.path(), BashConfig::new(dir.path()));
    let args = serde_json::json!({
        "path": "src",
        "recursive": true
    });
    let result = toolset
        .call("list_directory", args.to_string())
        .await
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let entries = parsed["entries"].as_array().unwrap();
    let paths: Vec<&str> = entries
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"src/lib.rs"));
    assert!(paths.contains(&"src/fs"));
    assert!(paths.contains(&"src/fs/mod.rs"));
}

#[tokio::test]
async fn grep_tool_finds_lines() {
    let dir = TempDir::new().unwrap();
    std::fs::File::create(dir.path().join("data.txt"))
        .unwrap()
        .write_all(b"apple\nbanana\napricot\n")
        .unwrap();

    let toolset = default_toolset(dir.path(), BashConfig::new(dir.path()));
    let args = serde_json::json!({
        "pattern": "^a.*"
    });
    let result = toolset.call("grep", args.to_string()).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["matches"].as_array().unwrap().len(), 2);
}
