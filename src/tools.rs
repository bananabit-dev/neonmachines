use llmgraph::models::tools::{Tool, Function, Parameters, Property};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;

pub fn builtin_tools() -> Vec<(Tool, Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>)> {
    let mut tools: Vec<(Tool, Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>)> = Vec::new();

    // read_file
    let read_tool = Tool {
        tool_type: "function".into(),
        function: Function {
            name: "read_file".into(),
            description: "Read a file from disk".into(),
            parameters: Parameters {
                param_type: "object".into(),
                properties: {
                    let mut p = HashMap::new();
                    p.insert("path".into(), Property {
                        prop_type: "string".into(),
                        description: Some("Path to file".into()),
                        items: None,
                    });
                    p
                },
                required: vec!["path".into()],
            },
        },
    };
    let read_fn: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|args| {
        let path = args["path"].as_str().ok_or("Missing path")?;
        fs::read_to_string(path)
            .map(|c| json!({ "content": c }))
            .map_err(|e| e.to_string())
    });
    tools.push((read_tool, read_fn));

    // write_file
    let write_tool = Tool {
        tool_type: "function".into(),
        function: Function {
            name: "write_file".into(),
            description: "Write content to a file".into(),
            parameters: Parameters {
                param_type: "object".into(),
                properties: {
                    let mut p = HashMap::new();
                    p.insert("path".into(), Property {
                        prop_type: "string".into(),
                        description: Some("Path to file".into()),
                        items: None,
                    });
                    p.insert("content".into(), Property {
                        prop_type: "string".into(),
                        description: Some("Content to write".into()),
                        items: None,
                    });
                    p
                },
                required: vec!["path".into(), "content".into()],
            },
        },
    };
    let write_fn: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|args| {
        let path = args["path"].as_str().ok_or("Missing path")?;
        let content = args["content"].as_str().ok_or("Missing content")?;
        fs::write(path, content)
            .map(|_| json!({"status": "ok"}))
            .map_err(|e| e.to_string())
    });
    tools.push((write_tool, write_fn));

    // str_replace
    let replace_tool = Tool {
        tool_type: "function".into(),
        function: Function {
            name: "str_replace".into(),
            description: "Replace substring in text".into(),
            parameters: Parameters {
                param_type: "object".into(),
                properties: {
                    let mut p = HashMap::new();
                    p.insert("text".into(), Property {
                        prop_type: "string".into(),
                        description: Some("Original text".into()),
                        items: None,
                    });
                    p.insert("from".into(), Property {
                        prop_type: "string".into(),
                        description: Some("Substring to replace".into()),
                        items: None,
                    });
                    p.insert("to".into(), Property {
                        prop_type: "string".into(),
                        description: Some("Replacement string".into()),
                        items: None,
                    });
                    p
                },
                required: vec!["text".into(), "from".into(), "to".into()],
            },
        },
    };
    let replace_fn: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync> = Box::new(|args| {
        let text = args["text"].as_str().ok_or("Missing text")?;
        let from = args["from"].as_str().ok_or("Missing from")?;
        let to = args["to"].as_str().ok_or("Missing to")?;
        Ok(json!({ "result": text.replace(from, to) }))
    });
    tools.push((replace_tool, replace_fn));

    tools
}