use crate::shared_history::SharedHistory;
use chrono::Local;
use llmgraph::models::tools::{Function, Message, Parameters, Property, Tool};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::sync::{Arc, Mutex};

use std::path::Path;

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
) -> Vec<(
    Tool,
    Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>,
)> {
    let mut tools: Vec<(
        Tool,
        Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>,
    )> = Vec::new();

    let todo_list = TodoList::default();
    let issue_tracker = IssueTracker::default();

    // -------------------------
    // Filesystem / Terminal Tools
    // -------------------------

    // pwd
    {
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "pwd".into(),
                description: "Print current working directory".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: HashMap::<String, Property>::new(),
                    required: vec![],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|_args| {
            env::current_dir()
                .map(|p| {
                    let cwd = p.to_string_lossy().to_string();
                    println!("[TOOL][pwd] {}", cwd);
                    json!({ "cwd": cwd })
                })
                .map_err(|e| e.to_string())
        });
        tools.push((tool, func));
    }

    // ls
    {
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
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|args| {
            let path = args["path"].as_str().unwrap_or(".");
            let entries = fs::read_dir(path)
                .map_err(|e| e.to_string())?
                .map(|e| {
                    e.map(|e| e.file_name().to_string_lossy().to_string())
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            println!("[TOOL][ls] {} entries in {}", entries.len(), path);
            Ok(json!({ "entries": entries }))
        });
        tools.push((tool, func));
    }

    // -------------------------
    // String Manipulation Tools
    // -------------------------

    macro_rules! str_tool {
        ($name:expr, $desc:expr, $func:expr) => {{
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
                    println!("[TOOL][{}] '{}'", $name, result);
                    Ok(json!({ "result": result }))
                });
            tools.push((tool, func));
        }};
    }

    str_tool!("to_upper", "Convert text to uppercase", |s: &str| s
        .to_uppercase());
    str_tool!("to_lower", "Convert text to lowercase", |s: &str| s
        .to_lowercase());
    str_tool!("trim", "Trim whitespace", |s: &str| s.trim().to_string());
    str_tool!("reverse", "Reverse string", |s: &str| s
        .chars()
        .rev()
        .collect::<String>());

    // yes_no_paragraphs
    {
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
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|args| {
            let text = args["text"].as_str().unwrap_or("");
            let mut results = Vec::new();
            for (i, para) in text.split("\n\n").enumerate() {
                let decision = if para.to_lowercase().contains("yes") {
                    "yes"
                } else {
                    "no"
                };
                println!("[TOOL][yes_no_paragraphs] para {} -> {}", i + 1, decision);
                results.push(decision.to_string());
            }
            Ok(json!({ "decisions": results }))
        });
        tools.push((tool, func));
    }

    // -------------------------
    // History Tools
    // -------------------------

    {
        let sh_clone = shared_history.clone();
        let mut props = HashMap::new();
        props.insert("limit".into(), prop("number", "Number of messages"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "get_history".into(),
                description: "Retrieve recent shared history".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec![],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let limit = args["limit"].as_u64().unwrap_or(10) as usize;
                let msgs = sh_clone.get_last(limit);
                println!("[TOOL][get_history] {} messages", msgs.len());
                Ok(json!({ "history": msgs }))
            });
        tools.push((tool, func));
    }

    {
        let sh_clone2 = shared_history.clone();
        let mut props = HashMap::new();
        props.insert("query".into(), prop("string", "Keyword to search"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "search_history".into(),
                description: "Search shared history".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["query".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let query = args["query"].as_str().unwrap_or("");
                let matches = sh_clone2.search(query);
                println!(
                    "[TOOL][search_history] '{}' -> {} matches",
                    query,
                    matches.len()
                );
                Ok(json!({ "matches": matches }))
            });
        tools.push((tool, func));
    }

    // -------------------------
    // Notes, Todos, Issues
    // -------------------------

    {
        let sh_clone3 = shared_history.clone();
        let mut props = HashMap::new();
        props.insert("text".into(), prop("string", "Note text"));
        props.insert("tag".into(), prop("string", "Optional tag"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "note".into(),
                description: "Store a note".into(),
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
                let tag = args["tag"].as_str().unwrap_or("general");
                println!("[TOOL][note] tag={} text={}", tag, text);
                sh_clone3.append(Message {
                    role: format!("note:{}", tag),
                    content: Some(text.to_string()),
                    tool_calls: None,
                });
                Ok(json!({ "status": "ok", "tag": tag, "note": text }))
            });
        tools.push((tool, func));
    }

    // Todo list
    {
        let todo_clone = todo_list.clone();
        let sh_clone4 = shared_history.clone();
        let mut props = HashMap::new();
        props.insert("task".into(), prop("string", "Task to add"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "add_todo".into(),
                description: "Add a task".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["task".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let task = args["task"].as_str().unwrap_or("");
                todo_clone.add(task);
                println!("[TOOL][add_todo] {}", task);
                sh_clone4.append(Message {
                    role: "todo".into(),
                    content: Some(format!("Added: {}", task)),
                    tool_calls: None,
                });
                Ok(json!({ "status": "ok", "task": task }))
            });
        tools.push((tool, func));
    }

    // -------------------------
    // Reasoning Tool
    // -------------------------

    {
        let sh_clone = shared_history.clone();
        let mut props = HashMap::new();
        props.insert("prompt".into(), prop("string", "Reasoning prompt"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "reason".into(),
                description: "Request the LLM to reason step-by-step".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["prompt".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let prompt = args["prompt"].as_str().unwrap_or("");
                println!("[TOOL][reason] Reasoning requested: {}", prompt);
                sh_clone.append(Message {
                    role: "reason".into(),
                    content: Some(prompt.to_string()),
                    tool_calls: None,
                });
                Ok(json!({ "status": "ok", "reasoning": format!("Thinking about: {}", prompt) }))
            });
        tools.push((tool, func));
    }

    // -------------------------
    // Debugging Tool
    // -------------------------

    {
        let mut props = HashMap::new();
        props.insert("message".into(), prop("string", "Message to log"));
        let tool = Tool {
            tool_type: "function".into(),
            function: Function {
                name: "log_message".into(),
                description: "Log debug message".into(),
                parameters: Parameters {
                    param_type: "object".into(),
                    properties: props,
                    required: vec!["message".into()],
                },
            },
        };
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|args| {
            let msg = args["message"].as_str().unwrap_or("");
            let now = Local::now();
            println!("[TOOL][log_message][{}] {}", now.format("%H:%M:%S"), msg);
            Ok(json!({ "logged": msg }))
        });
        tools.push((tool, func));
    }

    tools
}
/// Load custom tools from `.nmextension` files, ignoring `.nmignore`
pub fn load_extensions() -> Vec<(
    Tool,
    Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>,
)> {
    let mut tools = Vec::new();
    let ignore_patterns = load_ignore_patterns(".nmignore");

    if let Ok(entries) = fs::read_dir(".") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "nmextension" {
                    if is_ignored(&path, &ignore_patterns) {
                        continue;
                    }
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(parsed) = serde_json::from_str::<Vec<Tool>>(&content) {
                            for tool in parsed {
                                let func: Box<
                                    dyn Fn(Value) -> Result<Value, String> + Send + Sync,
                                > = Box::new(|args| Ok(json!({ "echo": args })));
                                tools.push((tool, func));
                            }
                        }
                    }
                }
            }
        }
    }
    tools
}

/// Load MCP servers from `.nmmcpextension` files
pub fn load_mcp_extensions() -> Vec<(
    Tool,
    Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>,
)> {
    let mut tools = Vec::new();
    let ignore_patterns = load_ignore_patterns(".nmignore");

    if let Ok(entries) = fs::read_dir(".") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "nmmcpextension" {
                    if is_ignored(&path, &ignore_patterns) {
                        continue;
                    }
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(parsed) = serde_json::from_str::<Vec<Tool>>(&content) {
                            for tool in parsed {
                                let func: Box<
                                    dyn Fn(Value) -> Result<Value, String> + Send + Sync,
                                > = Box::new(|args| Ok(json!({ "mcp_call": args })));
                                tools.push((tool, func));
                            }
                        }
                    }
                }
            }
        }
    }
    tools
}

/// Load ignore patterns from `.nmignore`
fn load_ignore_patterns(file: &str) -> Vec<String> {
    if let Ok(content) = fs::read_to_string(file) {
        content
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    } else {
        Vec::new()
    }
}

/// Check if a file path matches ignore patterns
fn is_ignored(path: &Path, patterns: &[String]) -> bool {
    let fname = path.file_name().unwrap_or_default().to_string_lossy();
    for pat in patterns {
        if fname.contains(pat) {
            return true;
        }
    }
    false
}
