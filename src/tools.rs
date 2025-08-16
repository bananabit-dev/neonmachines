use crate::shared_history::SharedHistory;
use crate::runner::AppEvent;
use llmgraph::models::tools::{Tool, Function, Parameters, Property, Message};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::env;
use std::path::Path;
use chrono::Local;
use std::sync::{Arc, Mutex};
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

/// Global state for todos and issues
#[derive(Clone, Default)]
struct TodoList {
    items: Arc<Mutex<Vec<(String, bool)>>>, // (task, done)
}
impl TodoList {
    fn add(&self, task: &str) {
        if let Ok(mut list) = self.items.lock() {
            list.push((task.to_string(), false));
        }
    }
    fn check(&self, idx: usize) -> bool {
        if let Ok(mut list) = self.items.lock() {
            if idx < list.len() {
                list[idx].1 = true;
                return true;
            }
        }
        false
    }
    fn list(&self) -> Vec<(String, bool)> {
        if let Ok(list) = self.items.lock() {
            list.clone()
        } else {
            vec![]
        }
    }
}

#[derive(Clone, Default)]
struct IssueTracker {
    issues: Arc<Mutex<Vec<(String, bool)>>>, // (issue, closed)
}
impl IssueTracker {
    fn create(&self, issue: &str) {
        if let Ok(mut list) = self.issues.lock() {
            list.push((issue.to_string(), false));
        }
    }
    fn close(&self, idx: usize) -> bool {
        if let Ok(mut list) = self.issues.lock() {
            if idx < list.len() {
                list[idx].1 = true;
                return true;
            }
        }
        false
    }
    fn list(&self) -> Vec<(String, bool)> {
        if let Ok(list) = self.issues.lock() {
            list.clone()
        } else {
            vec![]
        }
    }
}

/// Built-in + extended tools
pub fn builtin_tools_with_history(
    shared_history: SharedHistory,
    tx: UnboundedSender<AppEvent>,
) -> Vec<(Tool, Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>)> {
    let mut tools: Vec<(Tool, Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>)> = Vec::new();

    let todo_list = TodoList::default();
    let issue_tracker = IssueTracker::default();

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

    // mkdir
    {
        let tx_clone = tx.clone();
        let mut props = HashMap::new();
        props.insert("path".into(), prop("string", "Directory to create"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "mkdir".into(),
                description: "Create a new directory".into(),
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
                description: "Write content to a file (overwrite or append)".into(),
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

                let res = if append {
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                        .and_then(|mut f| std::io::Write::write_all(&mut f, content.as_bytes()))
                } else {
                    std::fs::write(path, content)
                };

                match res {
                    Ok(_) => {
                        let _ = tx_clone.send(AppEvent::Log(format!(
                            "[TOOL][write_file] wrote {} bytes to {} (append={})",
                            content.len(),
                            path,
                            append
                        )));
                        Ok(json!({ "status": "ok", "path": path }))
                    }
                    Err(e) => Err(e.to_string()),
                }
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
                for (i, part) in parts.iter().enumerate() {
                    if let Some(s) = part.as_str() {
                        use std::io::Write;
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
    // File Search/Replace Tools
    // -------------------------

    // search_in_file
    {
        let tx_clone = tx.clone();
        let mut props = HashMap::new();
        props.insert("path".into(), prop("string", "File to search"));
        props.insert("pattern".into(), prop("string", "Pattern to search for"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "search_in_file".into(),
                description: "Search for a pattern in a file".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["path".into(), "pattern".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let path = args["path"].as_str().ok_or("Missing path")?;
                let pattern = args["pattern"].as_str().ok_or("Missing pattern")?;
                let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
                let matches: Vec<String> = content
                    .lines()
                    .filter(|line| line.contains(pattern))
                    .map(|s| s.to_string())
                    .collect();
                let _ = tx_clone.send(AppEvent::Log(format!(
                    "[TOOL][search_in_file] found {} matches for '{}' in {}",
                    matches.len(),
                    pattern,
                    path
                )));
                Ok(json!({ "matches": matches }))
            });
        tools.push((tool, func));
    }

    // replace_in_file
    {
        let tx_clone = tx.clone();
        let mut props = HashMap::new();
        props.insert("path".into(), prop("string", "File path"));
        props.insert("search".into(), prop("string", "String or regex to search for"));
        props.insert("replace".into(), prop("string", "Replacement string"));
        props.insert("regex".into(), prop("boolean", "Use regex (default: false)"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "replace_in_file".into(),
                description: "Replace text in a file with optional regex".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["path".into(), "search".into(), "replace".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let path = args["path"].as_str().ok_or("Missing path")?;
                let search = args["search"].as_str().ok_or("Missing search")?;
                let replace = args["replace"].as_str().ok_or("Missing replace")?;
                let regex_mode = args["regex"].as_bool().unwrap_or(false);

                let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
                let new_content = if regex_mode {
                    let re = Regex::new(search).map_err(|e| e.to_string())?;
                    re.replace_all(&content, replace).to_string()
                } else {
                    content.replace(search, replace)
                };
                fs::write(path, &new_content).map_err(|e| e.to_string())?;
                let _ = tx_clone.send(AppEvent::Log(format!(
                    "[TOOL][replace_in_file] replaced '{}' with '{}' in {}",
                    search, replace, path
                )));
                Ok(json!({ "status": "ok", "path": path }))
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
                        "[TOOL][{}] input='{}' -> '{}'",
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

    // -------------------------
    // History, Notes, Todos, Issues, Reasoning, Debugging
    // -------------------------
    // (same as before, omitted here for brevity — keep your existing implementations)

    tools
}