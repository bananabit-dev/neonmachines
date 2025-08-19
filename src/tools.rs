use crate::shared_history::SharedHistory;
use crate::runner::AppEvent;
use llmgraph::models::tools::{Tool, Function, Parameters, Property};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::UnboundedSender;
use std::process::{Command, Stdio};

/// Helper to define properties
fn prop(typ: &str, desc: &str) -> Property {
    Property {
        prop_type: typ.into(),
        description: Some(desc.into()),
        items: None,
    }
}

/// Resolve a path relative to working_dir
fn resolve_path(working_dir: &str, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        Path::new(working_dir).join(p)
    }
}

/// Built-in + extended tools
pub fn builtin_tools_with_history(
    _shared_history: SharedHistory,
    tx: UnboundedSender<AppEvent>,
    working_dir: String,
) -> Vec<(Tool, Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>)> {
    let mut tools: Vec<(Tool, Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>)> = Vec::new();

    // -------------------------
    // Filesystem Tools
    // -------------------------

    // pwd
    {
        let tx_clone = tx.clone();
        let wd = working_dir.clone();
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "pwd".into(),
                description: "Print current working directory".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: HashMap::new(),
                    required: vec![],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |_args| {
                let cwd = wd.clone();
                let result = json!({ "cwd": cwd });
                let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][pwd] result = {}", result)));
                Ok(result)
            });
        tools.push((tool, func));
    }

    // ls
    {
        let tx_clone = tx.clone();
        let wd = working_dir.clone();
        let mut props = HashMap::new();
        props.insert("path".into(), prop("string", "Directory to list"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "ls".into(),
                description: "List directory contents".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec![],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let path = args["path"].as_str().unwrap_or(".");
                let full_path = resolve_path(&wd, path);
                let entries = fs::read_dir(&full_path)
                    .map_err(|e| e.to_string())?
                    .map(|e| e.map(|e| e.file_name().to_string_lossy().to_string()).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<_>, _>>()?;
                let result = json!({ "entries": entries });
                let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][ls] result = {}", result)));
                Ok(result)
            });
        tools.push((tool, func));
    }

    // mkdir
    {
        let tx_clone = tx.clone();
        let wd = working_dir.clone();
        let mut props = HashMap::new();
        props.insert("path".into(), prop("string", "Directory to create"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "mkdir".into(),
                description: "Create a new directory (idempotent)".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["path".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let path = args["path"].as_str().ok_or("Missing path")?;
                let full_path = resolve_path(&wd, path);
                let result = if full_path.exists() {
                    json!({ "status": "exists", "path": full_path })
                } else {
                    fs::create_dir_all(&full_path).map_err(|e| e.to_string())?;
                    json!({ "status": "ok", "path": full_path })
                };
                let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][mkdir] result = {}", result)));
                Ok(result)
            });
        tools.push((tool, func));
    }

    // touch
    {
        let tx_clone = tx.clone();
        let wd = working_dir.clone();
        let mut props = HashMap::new();
        props.insert("path".into(), prop("string", "File to create or update timestamp"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "touch".into(),
                description: "Create an empty file or update its timestamp".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["path".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let path = args["path"].as_str().ok_or("Missing path")?;
                let full_path = resolve_path(&wd, path);
                fs::OpenOptions::new().create(true).write(true).open(&full_path)
                    .map_err(|e| e.to_string())?;
                let result = json!({ "status": "ok", "path": full_path });
                let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][touch] result = {}", result)));
                Ok(result)
            });
        tools.push((tool, func));
    }

    // delete_file
    {
        let tx_clone = tx.clone();
        let wd = working_dir.clone();
        let mut props = HashMap::new();
        props.insert("path".into(), prop("string", "File path to delete"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "delete_file".into(),
                description: "Delete a file".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["path".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let path = args["path"].as_str().ok_or("Missing path")?;
                let full_path = resolve_path(&wd, path);
                fs::remove_file(&full_path).map_err(|e| e.to_string())?;
                let result = json!({ "status": "ok", "path": full_path });
                let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][delete_file] result = {}", result)));
                Ok(result)
            });
        tools.push((tool, func));
    }

    // -------------------------
    // File Writing Tools
    // -------------------------

    // write_file
    {
        let tx_clone = tx.clone();
        let mut props = HashMap::new();
        props.insert("path".into(), prop("string", "File path to write"));
        props.insert("content".into(), prop("string", "Content to write"));
        props.insert("append".into(), prop("boolean", "Append instead of overwrite"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "write_file".into(),
                description: "Write content to a file (splits into chunks if too large)".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["path".into(), "content".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let path = args["path"].as_str().ok_or("Missing path")?;
                let content = args["content"].as_str().ok_or("Missing content")?;
                let append = args["append"].as_bool().unwrap_or(false);

                let mut file = if append {
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                        .map_err(|e| e.to_string())?
                } else {
                    std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open(path)
                        .map_err(|e| e.to_string())?
                };

                use std::io::Write;
                let mut total_bytes = 0;
                let mut chunks = 0;
                for chunk in content.as_bytes().chunks(8192) {
                    file.write_all(chunk).map_err(|e| e.to_string())?;
                    total_bytes += chunk.len();
                    chunks += 1;
                }

                let result = json!({
                    "status": "ok",
                    "path": path,
                    "bytes_written": total_bytes,
                    "chunks": chunks
                });
                let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][write_file] result = {}", result)));
                Ok(result)
            });
        tools.push((tool, func));
    }

    // write_file_parts
    {
        let tx_clone = tx.clone();
        let mut props = HashMap::new();
        props.insert("path".into(), prop("string", "File path to write"));
        props.insert("parts".into(), prop("array", "Array of content parts to write sequentially"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "write_file_parts".into(),
                description: "Write multiple parts sequentially to a file".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["path".into(), "parts".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let path = args["path"].as_str().ok_or("Missing path")?;
                let parts = args["parts"].as_array().ok_or("Missing parts")?;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(path)
                    .map_err(|e| e.to_string())?;
                use std::io::Write;
                for (i, part) in parts.iter().enumerate() {
                    if let Some(s) = part.as_str() {
                        file.write_all(s.as_bytes()).map_err(|e| e.to_string())?;
                        let _ = tx_clone.send(AppEvent::Log(format!(
                            "[TOOL][write_file_parts] wrote part {} ({} bytes) to {}",
                            i + 1,
                            s.len(),
                            path
                        )));
                    }
                }
                let result = json!({ "status": "ok", "path": path, "parts": parts.len() });
                let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][write_file_parts] result = {}", result)));
                Ok(result)
            });
        tools.push((tool, func));
    }

    // -------------------------
    // File Reading Tool
    // -------------------------

    // read_file_content
    {
        let tx_clone = tx.clone();
        let mut props = HashMap::new();
        props.insert("path".into(), prop("string", "File path to read"));
        props.insert("start_line".into(), prop("integer", "Optional start line (0-based)"));
        props.insert("end_line".into(), prop("integer", "Optional end line (exclusive)"));
        props.insert("radius".into(), prop("integer", "Optional radius around a line number"));
        props.insert("line".into(), prop("integer", "Optional line number to center view on"));
        props.insert("max_bytes".into(), prop("integer", "Maximum bytes to return (default 8192)"));

        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "read_file_content".into(),
                description: "Read file content (like cat) with optional line range, radius, and max_bytes".to_string(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["path".into()],
                },
            },
        };

        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let path = args["path"].as_str().ok_or("Missing path")?;
                let start_line = args["start_line"].as_i64().unwrap_or(0).max(0) as usize;
                let end_line = args["end_line"].as_i64().unwrap_or(-1);
                let radius = args["radius"].as_i64().unwrap_or(0).max(0) as usize;
                let line = args["line"].as_i64().unwrap_or(-1);
                let max_bytes = args["max_bytes"].as_i64().unwrap_or(8192).max(1) as usize;

                let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();

                let (start, end) = if line >= 0 {
                    let center = line as usize;
                    let s = center.saturating_sub(radius);
                    let e = (center + radius + 1).min(total_lines);
                    (s, e)
                } else if end_line >= 0 {
                    (start_line, (end_line as usize).min(total_lines))
                } else {
                    (start_line, total_lines)
                };

                let mut selected: Vec<String> = Vec::new();
                for (i, l) in lines.iter().enumerate().take(end).skip(start) {
                    selected.push(format!("{:>5}: {}", i, l));
                }

                let mut result_str = selected.join("\n");
                if result_str.len() > max_bytes {
                    result_str.truncate(max_bytes);
                    result_str.push_str("\n...[truncated]");
                }

                let result = json!({
                    "path": path,
                    "lines": selected.len(),
                    "start": start,
                    "end": end,
                    "content": result_str
                });
                let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][read_file_content] result = {}", result)));
                Ok(result)
            });

        tools.push((tool, func));
    }

    // -------------------------
    // String Manipulation Tools
    // -------------------------

    macro_rules! str_tool {
        ($name:expr, $desc:expr, $func:expr) => {{
            let tx_clone = tx.clone();
            let mut props = HashMap::new();
            props.insert("text".into(), prop("string", "Input text"));
            let tool = Tool {
                tool_type: "function".into(),
                function: Function {
                    name: $name.into(),
                    description: $desc.into(),
                    parameters: Parameters {
                        param_type: "object".into(),
                        properties: props,
                        required: vec!["text".into()],
                    },
                },
            };
            let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
                Box::new(move |args| {
                    let text = args["text"].as_str().unwrap_or("");
                    let result = $func(text);
                    let result_json = json!({ "result": result });
                    let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][{}] result = {}", $name, result_json)));
                    Ok(result_json)
                });
            tools.push((tool, func));
        }};
    }

    str_tool!("to_upper", "Convert text to uppercase", |s: &str| s.to_uppercase());
    str_tool!("to_lower", "Convert text to lowercase", |s: &str| s.to_lowercase());
    str_tool!("trim", "Trim whitespace", |s: &str| s.trim().to_string());
    str_tool!("reverse", "Reverse string", |s: &str| s.chars().rev().collect::<String>());

    // yes_no_paragraphs
    {
        let tx_clone = tx.clone();
        let mut props = HashMap::new();
        props.insert("text".into(), prop("string", "Input text with paragraphs"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "yes_no_paragraphs".into(),
                description: "For each paragraph, decide yes/no".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["text".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let text = args["text"].as_str().unwrap_or("");
                let mut results = Vec::new();
                for para in text.split("\n\n") {
                    let decision = if para.to_lowercase().contains("yes") { "yes" } else { "no" };
                    results.push(decision.to_string());
                }
                let result = json!({ "decisions": results });
                let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][yes_no_paragraphs] result = {}", result)));
                Ok(result)
            });
        tools.push((tool, func));
    }

    // -------------------------
    // Terminal/Command Execution Tool
    // -------------------------

    // execute_terminal
    {
        let tx_clone = tx.clone();
        let wd = working_dir.clone();
        let mut props = HashMap::new();
        props.insert("command".into(), prop("string", "The terminal/bash command to execute. Example: 'ls -la', 'cat file.txt', 'mkdir new_dir'"));
        props.insert("working_directory".into(), prop("string", "Optional working directory where the command should be executed. If not provided, uses current directory"));
        props.insert("timeout_seconds".into(), prop("integer", "Optional timeout in seconds. Default is 30 seconds"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "execute_terminal".into(),
                description: "Execute a terminal/bash command and get the output. Use this for file operations, system commands, or any shell execution.".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["command".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let command = args["command"].as_str().ok_or("Missing 'command' parameter")?;
                let working_dir = args["working_directory"].as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| wd.clone());
                let timeout = args["timeout_seconds"].as_u64().unwrap_or(30);

                let mut cmd = Command::new("sh");
                cmd.arg("-c").arg(command);
                cmd.current_dir(&working_dir);

                // Set up process to capture output
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());

                // Spawn the process
                let child = cmd.spawn()
                    .map_err(|e| format!("Failed to start command: {}", e))?;

                // Wait for the process to complete with timeout
                let result = std::thread::spawn(move || {
                    child.wait_with_output()
                }).join()
                .map_err(|_| "Failed to wait for command execution".to_string())?;

                match result {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let exit_code = output.status.code().unwrap_or(-1);

                        let result = json!({
                            "success": output.status.success(),
                            "exit_code": exit_code,
                            "stdout": stdout.to_string(),
                            "stderr": stderr.to_string(),
                            "command": command,
                            "working_directory": working_dir,
                            "timeout_used": timeout
                        });
                        let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][execute_terminal] result = {}", result)));
                        Ok(result)
                    }
                    Err(e) => {
                        let error_msg = format!("Command execution failed: {}", e);
                        let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][execute_terminal] error = {}", error_msg)));
                        Err(error_msg)
                    }
                }
            });
        tools.push((tool, func));
    }

    tools
}