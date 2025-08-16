use llmgraph::models::tools::{Tool, Function, Parameters, Property};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::env;
use std::path::{Path, PathBuf};
use filetime;

/// Built-in tools (pwd, cd, ls, grep, mkdir, touch)
pub fn builtin_tools() -> Vec<(Tool, Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>)> {
    let mut tools: Vec<(Tool, Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>)> = Vec::new();

    fn prop(typ: &str, desc: &str) -> Property {
        Property {
            prop_type: typ.into(),
            description: Some(desc.into()),
            items: None,
        }
    }

    // pwd
    let pwd_tool = Tool {
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
    let pwd_fn: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|_args| {
        env::current_dir()
            .map(|p| json!({ "cwd": p.to_string_lossy() }))
            .map_err(|e| e.to_string())
    });
    tools.push((pwd_tool, pwd_fn));

    // cd
    let mut cd_props = HashMap::new();
    cd_props.insert("path".into(), prop("string", "Path to change directory to"));
    let cd_tool = Tool {
        tool_type: "function".into(),
        function: Function {
            name: "cd".into(),
            description: "Change current working directory".into(),
            parameters: Parameters {
                param_type: "object".into(),
                properties: cd_props,
                required: vec!["path".into()],
            },
        },
    };
    let cd_fn: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|args| {
        let path = args["path"].as_str().ok_or("Missing path")?;
        env::set_current_dir(path)
            .map(|_| json!({ "status": "ok", "cwd": env::current_dir().unwrap().to_string_lossy() }))
            .map_err(|e| e.to_string())
    });
    tools.push((cd_tool, cd_fn));

    // ls
    let mut ls_props = HashMap::new();
    ls_props.insert("path".into(), prop("string", "Directory to list (default: .)"));
    ls_props.insert("limit".into(), prop("number", "Optional max number of entries to return"));
    let ls_tool = Tool {
        tool_type: "function".into(),
        function: Function {
            name: "ls".into(),
            description: "List directory contents".into(),
            parameters: Parameters {
                param_type: "object".into(),
                properties: ls_props,
                required: vec![],
            },
        },
    };
    let ls_fn: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|args| {
        let path = args["path"].as_str().unwrap_or(".");
        let limit = args["limit"].as_u64().unwrap_or(50) as usize;
        let entries = fs::read_dir(path)
            .map_err(|e| e.to_string())?
            .take(limit)
            .map(|e| e.map(|e| e.file_name().to_string_lossy().to_string()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({ "entries": entries }))
    });
    tools.push((ls_tool, ls_fn));

    // grep
    let mut grep_props = HashMap::new();
    grep_props.insert("path".into(), prop("string", "File to search"));
    grep_props.insert("pattern".into(), prop("string", "Pattern to search for"));
    grep_props.insert("limit".into(), prop("number", "Optional max number of matches"));
    let grep_tool = Tool {
        tool_type: "function".into(),
        function: Function {
            name: "grep".into(),
            description: "Search for a pattern in a file".into(),
            parameters: Parameters {
                param_type: "object".into(),
                properties: grep_props,
                required: vec!["path".into(), "pattern".into()],
            },
        },
    };
    let grep_fn: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|args| {
        let path = args["path"].as_str().ok_or("Missing path")?;
        let pattern = args["pattern"].as_str().ok_or("Missing pattern")?;
        let limit = args["limit"].as_u64().unwrap_or(20) as usize;
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut matches = Vec::new();
        for line in content.lines() {
            if line.contains(pattern) {
                matches.push(line.to_string());
                if matches.len() >= limit {
                    break;
                }
            }
        }
        Ok(json!({ "matches": matches }))
    });
    tools.push((grep_tool, grep_fn));

    // mkdir
    let mut mkdir_props = HashMap::new();
    mkdir_props.insert("path".into(), prop("string", "Directory to create"));
    let mkdir_tool = Tool {
        tool_type: "function".into(),
        function: Function {
            name: "mkdir".into(),
            description: "Create a new directory".into(),
            parameters: Parameters {
                param_type: "object".into(),
                properties: mkdir_props,
                required: vec!["path".into()],
            },
        },
    };
    let mkdir_fn: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|args| {
        let path = args["path"].as_str().ok_or("Missing path")?;
        fs::create_dir_all(path)
            .map(|_| json!({ "status": "ok" }))
            .map_err(|e| e.to_string())
    });
    tools.push((mkdir_tool, mkdir_fn));

    // touch
    let mut touch_props = HashMap::new();
    touch_props.insert("path".into(), prop("string", "File to create or update timestamp"));
    let touch_tool = Tool {
        tool_type: "function".into(),
        function: Function {
            name: "touch".into(),
            description: "Create an empty file or update its timestamp".into(),
            parameters: Parameters {
                param_type: "object".into(),
                properties: touch_props,
                required: vec!["path".into()],
            },
        },
    };
    let touch_fn: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|args| {
        let path = args["path"].as_str().ok_or("Missing path")?;
        if Path::new(path).exists() {
            let now = filetime::FileTime::now();
            filetime::set_file_times(path, now, now).map_err(|e| e.to_string())?;
        } else {
            fs::File::create(path).map_err(|e| e.to_string())?;
        }
        Ok(json!({ "status": "ok" }))
    });
    tools.push((touch_tool, touch_fn));

    tools
}

/// Load custom tools from `.nmextension` files, ignoring `.nmignore`
pub fn load_extensions() -> Vec<(Tool, Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>)> {
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
                                let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
                                    Box::new(|args| Ok(json!({ "echo": args })));
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
pub fn load_mcp_extensions() -> Vec<(Tool, Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>)> {
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
                                // Default MCP function: echo args
                                let func: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> =
                                    Box::new(|args| Ok(json!({ "mcp_call": args })));
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