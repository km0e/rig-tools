# rig-tools

Basic filesystem and shell tools for the [rig](https://crates.io/crates/rig-core) LLM agent framework.

## Provided tools

| Tool   | Description |
|--------|-------------|
| `read` | Read a text file (with optional line offset / limit). |
| `edit` | Write, string-replace, insert, or delete files. |
| `glob` | Find files matching a glob pattern. |
| `grep` | Search file contents with a regular expression. |
| `bash` | Execute a shell command. |

## Usage

Add the crate to your project:

```bash
cargo add rig-tools
```

Register all tools with a Rig agent:

```rust
use rig_core::{completion::Prompt, providers::openai};
use rig_tools::{default_toolset, BashConfig};

#[tokio::main]
async fn main() {
    let bash_config = BashConfig::new("/tmp/agent-workspace");
    let toolset = default_toolset("/tmp/agent-workspace", bash_config);

    let agent = openai::Client::from_env()
        .agent("gpt-4o")
        .preamble("You are a helpful coding assistant.")
        .tools(toolset)
        .build();

    let response = agent.prompt("List the Rust files in the workspace").await.unwrap();
    println!("{}", response);
}
```

## Safety

- All filesystem tools resolve paths relative to a configured `WorkingDirectory` and reject paths that escape it.
- `BashTool` accepts a `BashConfig` with optional command allow-list, environment variable allow-list, and timeout.

## License

MIT
