//! Tool definitions, registry, and built-in tool implementations.
//!
//! Each tool is a self-contained unit implementing the [`Tool`] trait.
//! The [`ToolRegistry`] collects tools and exposes them to the prompt
//! builder and orchestrator.  New tools are added via `register()`.

use std::collections::HashMap;
use std::process::Command;

/// Description of a tool parameter.
#[derive(Debug, Clone)]
pub struct ToolParam {
    pub name: &'static str,
    pub description: &'static str,
    pub required: bool,
}

/// Machine-readable specification of a tool.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Vec<ToolParam>,
}

/// Parsed arguments for a tool call (name → value).
pub type ToolCallArgs = HashMap<String, serde_json::Value>;

/// Outcome of executing a tool.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Textual output (stdout, file contents, etc.).
    pub output: String,
    /// Whether the execution is considered an error.
    pub is_error: bool,
}

/// Contract that every agent tool must satisfy.
///
/// Implementations are expected to be **stateless** — all context is
/// provided through the `args` and `cwd` parameters.
pub trait Tool: Send + Sync {
    /// Machine-readable spec exposed in the system prompt.
    fn spec(&self) -> ToolSpec;

    /// Execute the tool with the given arguments.
    ///
    /// `cwd` is the shell's current working directory so that
    /// file-relative operations resolve correctly.
    fn execute(&self, args: &ToolCallArgs, cwd: &str) -> ToolResult;
}

/// Central collection of available tools.
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Register a new tool.  Order determines prompt listing order.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.spec().name == name).map(|t| t.as_ref())
    }

    /// Specs of all registered tools (for prompt building).
    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools.iter().map(|t| t.spec()).collect()
    }

    /// Create a registry pre-loaded with the default built-in tools.
    pub fn with_defaults() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(ShellExecTool));
        reg.register(Box::new(ReadFileTool));
        reg.register(Box::new(WriteFileTool));
        reg.register(Box::new(ListDirTool));
        reg.register(Box::new(WebSearchTool));
        reg
    }
}

/// Execute a shell command and capture its output.
pub struct ShellExecTool;

impl Tool for ShellExecTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "shell_exec",
            description: "Execute a shell command and return its stdout/stderr output.",
            parameters: vec![ToolParam {
                name: "command",
                description: "The shell command to execute (passed to sh -c).",
                required: true,
            }],
        }
    }

    fn execute(&self, args: &ToolCallArgs, cwd: &str) -> ToolResult {
        let cmd = match args.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return ToolResult {
                    output: "Error: missing required parameter 'command'.".into(),
                    is_error: true,
                };
            }
        };

        match Command::new("sh").arg("-c").arg(cmd).current_dir(cwd).output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let mut text = stdout.into_owned();
                if !stderr.is_empty() {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&stderr);
                }
                const MAX_OUTPUT: usize = 8192;
                if text.len() > MAX_OUTPUT {
                    text.truncate(MAX_OUTPUT);
                    text.push_str("\n... (output truncated)");
                }
                let code = output.status.code();
                ToolResult {
                    output: text,
                    is_error: code.map_or(true, |c| c != 0),
                }
            }
            Err(e) => ToolResult {
                output: format!("Failed to execute command: {e}"),
                is_error: true,
            },
        }
    }
}

/// Read the contents of a file.
pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_file",
            description: "Read and return the contents of a file.",
            parameters: vec![ToolParam {
                name: "path",
                description: "Path to the file (absolute or relative to cwd).",
                required: true,
            }],
        }
    }

    fn execute(&self, args: &ToolCallArgs, cwd: &str) -> ToolResult {
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ToolResult {
                    output: "Error: missing required parameter 'path'.".into(),
                    is_error: true,
                };
            }
        };

        let full = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            std::path::PathBuf::from(cwd).join(path)
        };

        match std::fs::read_to_string(&full) {
            Ok(contents) => {
                const MAX_READ: usize = 16384;
                let mut text = contents;
                if text.len() > MAX_READ {
                    text.truncate(MAX_READ);
                    text.push_str("\n... (file truncated)");
                }
                ToolResult { output: text, is_error: false }
            }
            Err(e) => ToolResult {
                output: format!("Error reading file: {e}"),
                is_error: true,
            },
        }
    }
}

/// Write content to a file (creates or overwrites).
pub struct WriteFileTool;

impl Tool for WriteFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "write_file",
            description: "Write content to a file (creates or overwrites).",
            parameters: vec![
                ToolParam {
                    name: "path",
                    description: "Path to the file (absolute or relative to cwd).",
                    required: true,
                },
                ToolParam {
                    name: "content",
                    description: "The text content to write.",
                    required: true,
                },
            ],
        }
    }

    fn execute(&self, args: &ToolCallArgs, cwd: &str) -> ToolResult {
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ToolResult {
                    output: "Error: missing required parameter 'path'.".into(),
                    is_error: true,
                };
            }
        };
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return ToolResult {
                    output: "Error: missing required parameter 'content'.".into(),
                    is_error: true,
                };
            }
        };

        let full = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            std::path::PathBuf::from(cwd).join(path)
        };

        if let Some(parent) = full.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match std::fs::write(&full, content) {
            Ok(()) => ToolResult {
                output: format!("Written {} bytes to {}", content.len(), full.display()),
                is_error: false,
            },
            Err(e) => ToolResult {
                output: format!("Error writing file: {e}"),
                is_error: true,
            },
        }
    }
}

/// List files and directories in a given path.
pub struct ListDirTool;

impl Tool for ListDirTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "list_dir",
            description: "List files and directories in a path.",
            parameters: vec![ToolParam {
                name: "path",
                description: "Directory path (absolute or relative to cwd). Defaults to cwd if omitted.",
                required: false,
            }],
        }
    }

    fn execute(&self, args: &ToolCallArgs, cwd: &str) -> ToolResult {
        let dir = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let full = if std::path::Path::new(dir).is_absolute() {
            std::path::PathBuf::from(dir)
        } else {
            std::path::PathBuf::from(cwd).join(dir)
        };

        match std::fs::read_dir(&full) {
            Ok(entries) => {
                let mut lines: Vec<String> = Vec::new();
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let is_dir = entry.file_type().map_or(false, |ft| ft.is_dir());
                    if is_dir {
                        lines.push(format!("{name}/"));
                    } else {
                        lines.push(name);
                    }
                }
                lines.sort();
                ToolResult {
                    output: lines.join("\n"),
                    is_error: false,
                }
            }
            Err(e) => ToolResult {
                output: format!("Error listing directory: {e}"),
                is_error: true,
            },
        }
    }
}

/// Search the web via DuckDuckGo and return results.
pub struct WebSearchTool;

impl Tool for WebSearchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "web_search",
            description: "Search the internet and return relevant results with titles, URLs, and snippets.",
            parameters: vec![ToolParam {
                name: "query",
                description: "The search query string.",
                required: true,
            }],
        }
    }

    fn execute(&self, args: &ToolCallArgs, _cwd: &str) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => {
                return ToolResult {
                    output: "Error: missing required parameter 'query'.".into(),
                    is_error: true,
                };
            }
        };

        match crate::ai::web_search::search_structured(query, 5) {
            Some(results) => {
                let mut out = String::new();
                for (i, r) in results.iter().enumerate() {
                    out.push_str(&format!("{}. {}\n", i + 1, r.title));
                    if !r.url.is_empty() {
                        out.push_str(&format!("   {}\n", r.url));
                    }
                    if !r.snippet.is_empty() {
                        out.push_str(&format!("   {}\n", r.snippet));
                    }
                    out.push('\n');
                }
                ToolResult {
                    output: out.trim_end().to_string(),
                    is_error: false,
                }
            }
            None => ToolResult {
                output: format!("No search results found for '{query}'."),
                is_error: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_with_defaults_has_tools() {
        let reg = ToolRegistry::with_defaults();
        assert!(reg.specs().len() >= 4);
    }

    #[test]
    fn registry_get_by_name() {
        let reg = ToolRegistry::with_defaults();
        assert!(reg.get("shell_exec").is_some());
        assert!(reg.get("read_file").is_some());
        assert!(reg.get("write_file").is_some());
        assert!(reg.get("list_dir").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn all_specs_have_names_and_descriptions() {
        let reg = ToolRegistry::with_defaults();
        for spec in reg.specs() {
            assert!(!spec.name.is_empty());
            assert!(!spec.description.is_empty());
        }
    }

    #[test]
    fn shell_exec_runs_echo() {
        let tool = ShellExecTool;
        let mut args = ToolCallArgs::new();
        args.insert("command".into(), serde_json::Value::String("echo hello".into()));
        let result = tool.execute(&args, "/tmp");
        assert!(!result.is_error);
        assert!(result.output.trim() == "hello");
    }

    #[test]
    fn shell_exec_missing_arg() {
        let tool = ShellExecTool;
        let args = ToolCallArgs::new();
        let result = tool.execute(&args, "/tmp");
        assert!(result.is_error);
        assert!(result.output.contains("missing"));
    }

    #[test]
    fn shell_exec_bad_command() {
        let tool = ShellExecTool;
        let mut args = ToolCallArgs::new();
        args.insert(
            "command".into(),
            serde_json::Value::String("nonexistent_cmd_12345".into()),
        );
        let result = tool.execute(&args, "/tmp");
        assert!(result.is_error);
    }

    #[test]
    fn read_file_missing_arg() {
        let tool = ReadFileTool;
        let args = ToolCallArgs::new();
        let result = tool.execute(&args, "/tmp");
        assert!(result.is_error);
    }

    #[test]
    fn read_file_nonexistent() {
        let tool = ReadFileTool;
        let mut args = ToolCallArgs::new();
        args.insert(
            "path".into(),
            serde_json::Value::String("/tmp/__awebo_nonexistent_test__".into()),
        );
        let result = tool.execute(&args, "/tmp");
        assert!(result.is_error);
    }

    #[test]
    fn write_and_read_file_roundtrip() {
        let path = "/tmp/__awebo_tool_test_roundtrip__.txt";
        let _ = std::fs::remove_file(path);

        let write_tool = WriteFileTool;
        let mut args = ToolCallArgs::new();
        args.insert("path".into(), serde_json::Value::String(path.into()));
        args.insert("content".into(), serde_json::Value::String("test content 123".into()));
        let wr = write_tool.execute(&args, "/tmp");
        assert!(!wr.is_error);

        let read_tool = ReadFileTool;
        let mut args2 = ToolCallArgs::new();
        args2.insert("path".into(), serde_json::Value::String(path.into()));
        let rd = read_tool.execute(&args2, "/tmp");
        assert!(!rd.is_error);
        assert_eq!(rd.output, "test content 123");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn list_dir_works() {
        let tool = ListDirTool;
        let mut args = ToolCallArgs::new();
        args.insert("path".into(), serde_json::Value::String("/tmp".into()));
        let result = tool.execute(&args, "/tmp");
        assert!(!result.is_error);
    }

    #[test]
    fn list_dir_nonexistent() {
        let tool = ListDirTool;
        let mut args = ToolCallArgs::new();
        args.insert(
            "path".into(),
            serde_json::Value::String("/tmp/__awebo_nonexistent_dir__".into()),
        );
        let result = tool.execute(&args, "/tmp");
        assert!(result.is_error);
    }

    #[test]
    fn tool_names_are_unique() {
        let reg = ToolRegistry::with_defaults();
        let specs = reg.specs();
        let mut names: Vec<&str> = specs.iter().map(|s| s.name).collect();
        let len_before = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), len_before, "duplicate tool names");
    }
}
