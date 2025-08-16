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

    {
        let tx_clone = tx.clone();
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

    // -------------------------
    // History Tools
    // -------------------------

    {
        let sh_clone = shared_history.clone();
        let tx_clone = tx.clone();
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
                let _ = tx_clone.send(AppEvent::Log(format!(
                    "[TOOL][get_history] returning {} messages",
                    msgs.len()
                )));
                Ok(json!({ "history": msgs }))
            });
        tools.push((tool, func));
    }

    {
        let sh_clone2 = shared_history.clone();
        let tx_clone = tx.clone();
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
                let _ = tx_clone.send(AppEvent::Log(format!(
                    "[TOOL][search_history] '{}' -> {} matches",
                    query,
                    matches.len()
                )));
                Ok(json!({ "matches": matches }))
            });
        tools.push((tool, func));
    }

    // -------------------------
    // Notes, Todos, Issues, Reasoning
    // -------------------------

    {
        let sh_clone3 = shared_history.clone();
        let tx_clone = tx.clone();
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
                let _ = tx_clone.send(AppEvent::Log(format!(
                    "[TOOL][note] tag={} text={}",
                    tag, text
                )));
                sh_clone3.append(Message {
                    role: format!("note:{}", tag),
                    content: Some(text.to_string()),
                    tool_calls: None,
                });
                Ok(json!({ "status": "ok", "tag": tag, "note": text }))
            });
        tools.push((tool, func));
    }

    {
        let sh_clone = shared_history.clone();
        let tx_clone = tx.clone();
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
                let _ = tx_clone.send(AppEvent::Log(format!(
                    "[TOOL][reason] Reasoning requested: {}",
                    prompt
                )));
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
        let tx_clone = tx.clone();
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
        let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
            Box::new(move |args| {
                let msg = args["message"].as_str().unwrap_or("");
                let now = Local::now();
                let _ = tx_clone.send(AppEvent::Log(format!(
                    "[TOOL][log_message][{}] {}",
                    now.format("%H:%M:%S"),
                    msg
                )));
                Ok(json!({ "logged": msg }))
            });
        tools.push((tool, func));
    }

    tools
}