use crate::shared_history::SharedHistory;
use crate::runner::AppEvent;
use llmgraph::models::tools::{Tool, Function, Parameters, Property};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::env;
use std::path::Path;
use tokio::sync::mpsc::UnboundedSender;
use regex::Regex;

/// Helper to define properties
fn prop(typ: &str, desc: &str) -> Property {
    Property {
        prop_type: typ.into(),
        description: Some(desc.into()),
        items: None,
    }
}

/// Built-in + extended tools
pub fn builtin_tools_with_history(
    shared_history: SharedHistory,
    tx: UnboundedSender<AppEvent>,
) -> Vec<(Tool, Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>)> {
    let mut tools: Vec<(Tool, Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>)> = Vec::new();

    // -------------------------
    // Filesystem Tools
    // -------------------------

    // pwd
    {
        let tx_clone = tx.clone();
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
                match env::current_dir() {
                    Ok(p) => {
                        let cwd = p.to_string_lossy().to_string();
                        let _ = tx_clone.send(AppEvent::Log(format!("[TOOL][pwd] {}", cwd)));
                        Ok(json!({ "cwd": cwd }))
                    }
                    Err(e) => Err(e.to_string()),
                }
            });
        tools.push((tool, func));
    }

    // ls
    {
        let tx_clone = tx.clone();
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
                let entries = fs::read_dir(path)
                    .map_err(|e| e.to_string())?
                    .map(|e| e.map(|e| e.file_name().to_string_lossy().to_string()).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<_>, _>>()?;
                let _ = tx_clone.send(AppEvent::Log(format!(
                    "[TOOL][ls] {} entries in {}",
                    entries.len(),
                    path
                )));
                Ok(json!({ "entries": entries }))
            });
        tools.push((tool, func));
    }

    // mkdir (idempotent)
    {
        let tx_clone = tx.clone();
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
                if Path::new(path).exists() {
                    let _ = tx_clone.send(AppEvent::Log(format!(
                        "[TOOL][mkdir] directory {} already exists",
                        path
                    )));
                    return Ok(json!({ "status": "exists", "path": path }));
                }
                match fs::create_dir_all(path) {
                    Ok(_) => {
                        let _ = tx_clone.send(AppEvent::Log(format!(
                            "[TOOL][mkdir] created directory {}",
                            path
                        )));
                        Ok(json!({ "status": "ok", "path": path }))
                    }
                    Err(e) => Err(e.to_string()),
                }
            });
        tools.push((tool, func));
    }

    // touch
    {
        let tx_clone = tx.clone();
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
                match fs::OpenOptions::new().create(true).write(true).open(path) {
                    Ok(_) => {
                        let _ = tx_clone.send(AppEvent::Log(format!(
                            "[TOOL][touch] touched file {}",
                            path
                        )));
                        Ok(json!({ "status": "ok", "path": path }))
                    }
                    Err(e) => Err(e.to_string()),
                }
            });
        tools.push((tool, func));
    }

    // delete_file
    {
        let tx_clone = tx.clone();
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
                match fs::remove_file(path) {
                    Ok(_) => {
                        let _ = tx_clone.send(AppEvent::Log(format!(
                            "[TOOL][delete_file] deleted {}",
                            path
                        )));
                        Ok(json!({ "status": "ok", "path": path }))
                    }
                    Err(e) => Err(e.to_string()),
                }
            });
        tools.push((tool, func));
    }

    // -------------------------
    // File Writing Tools
    // -------------------------

    // write_file (chunked)
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

                let _ = tx_clone.send(AppEvent::Log(format!(
                    "[TOOL][write_file] wrote {} bytes in {} chunks to {} (append={})",
                    total_bytes, chunks, path, append
                )));
                Ok(json!({
                    "status": "ok",
                    "path": path,
                    "bytes_written": total_bytes,
                    "chunks": chunks
                }))
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
                Ok(json!({ "status": "ok", "path": path, "parts": parts.len() }))
            });
        tools.push((tool, func));
    }

    // -------------------------
    // File Reading Tool (NEW)
    // -------------------------

    // read_file_content (like cat, with optional args)
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

                let mut result = selected.join("\n");
                if result.len() > max_bytes {
                    result.truncate(max_bytes);
                    result.push_str("\n...[truncated]");
                }

                let _ = tx_clone.send(AppEvent::Log(format!(
                    "[TOOL][read_file_content] {} lines [{}..{}] from {}",
                    selected.len(),
                    start,
                    end,
                    path
                )));

                Ok(json!({
                    "path": path,
                    "lines": selected.len(),
                    "start": start,
                    "end": end,
                    "content": result
                }))
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
                    let _ = tx_clone.send(AppEvent::Log(format!(
                        "[TOOL][{}] input='{}' -> '{}' ",
                        $name, text, result
                    )));
                    Ok(json!({ "result": result }))
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
                for (i, para) in text.split("\n\n").enumerate() {
                    let decision = if para.to_lowercase().contains("yes") { "yes" } else { "no" };
                    let _ = tx_clone.send(AppEvent::Log(format!(
                        "[TOOL][yes_no_paragraphs] para {} -> {}",
                        i + 1,
                        decision
                    )));
                    results.push(decision.to_string());
                }
                Ok(json!({ "decisions": results }))
            });
        tools.push((tool, func));
    }

    tools
}