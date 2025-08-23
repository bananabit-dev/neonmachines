use warp::ws::{Message, WebSocket};
use futures_util::stream::StreamExt;
use futures_util::sink::SinkExt;
use tokio::sync::{mpsc, Mutex};
use crate::app::App;
use crate::runner::{AppEvent, AppCommand};
use crate::nm_config::{load_all_nm, preset_workflows, WorkflowConfig};
use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Process input for preprompting with secondary agent support
/// Handles inputs in the format: "primary task input2=\"secondary task\""
fn process_preprompting_input(input: &str) -> String {
    // Check if input contains preprompting syntax
    if input.contains("input2=") {
        // Extract primary task (everything before input2=)
        let parts: Vec<&str> = input.split("input2=").collect();
        if parts.len() >= 2 {
            let primary_task = parts[0].trim();
            let secondary_task_raw = parts[1];
            
            // Extract secondary task (remove quotes if present)
            let secondary_task = if secondary_task_raw.starts_with('"') && secondary_task_raw.ends_with('"') && secondary_task_raw.len() >= 2 {
                &secondary_task_raw[1..secondary_task_raw.len()-1]
            } else {
                secondary_task_raw
            };
            
            // Return formatted input that includes both tasks
            return format!("Primary Task: {}\nSecondary Task: {}", primary_task, secondary_task);
        }
    }
    
    // Return original input if no preprompting syntax found
    input.to_string()
}

/// Generate MCP template content
fn generate_mcp_template(name: &str) -> String {
    format!(r#"{{
  "name": "{}",
  "version": "1.0.0",
  "description": "A sample MCP extension for {}",
  "author": "Your Name",
  "entry_point": "main.py",
  "dependencies": [],
  "tools": [
    {{
      "name": "{}_tool",
      "description": "A sample tool for {}",
      "parameters": {{
        "required": ["input"],
        "optional": ["option"],
        "types": {{
          "input": "string",
          "option": "string"
        }}
      }},
      "input_schema": {{
        "type": "object",
        "properties": {{
          "input": {{
            "type": "string",
            "description": "Input text for processing"
          }},
          "option": {{
            "type": "string",
            "description": "Optional parameter"
          }}
        }},
        "required": ["input"]
      }},
      "output_schema": {{
        "type": "object",
        "properties": {{
          "result": {{
            "type": "string",
            "description": "Processing result"
          }}
        }},
        "required": ["result"]
      }}
    }}
  ],
  "capabilities": {{
    "model_control": true,
    "tool_integration": true,
    "file_operations": false,
    "system_access": false
  }}
}}"#, name, name, name, name)
}

/// Generate tool template content
fn generate_tool_template(name: &str) -> String {
    format!(r#"name: {name}_tool
description: A sample tool for {name}

prompt: |
  You are a helpful assistant that processes user input.
  
  Input: {{input}}
  Option: {{option}}
  
  Please process this input and provide a helpful response.

input_variables:
  - name: input
    description: "Input text for processing"
    required: true
  - name: option
    description: "Optional parameter"
    required: false

output_format: |
  {{result}}

examples:
  - input: "Hello world"
    output: "{{result: \"Processed: Hello world\"}}"
  - input: "Test input"
    option: "with option"
    output: "{{result: \"Processed with option: Test input\"}}"
"#, name = name)
}

/// Create MCP template directory structure
fn create_mcp_template_structure(path: &str, metadata: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create directory
    std::fs::create_dir_all(path)?;
    
    // Create metadata file
    let metadata_path = format!("{}/nmmcp.json", path);
    std::fs::write(&metadata_path, metadata)?;
    
    // Create main.py entry point
    let main_py_content = r#"#!/usr/bin/env python3
"""
Sample MCP Extension Entry Point
"""
import json
import sys

def main():
    """Main entry point for the extension"""
    try:
        # Read input from stdin
        input_data = json.load(sys.stdin)
        
        # Process the input
        result = process_input(input_data)
        
        # Output result to stdout
        json.dump(result, sys.stdout)
        
    except Exception as e:
        # Error handling
        error_result = {
            "error": str(e)
        }
        json.dump(error_result, sys.stdout)

def process_input(data):
    """Process input data and return result"""
    # Extract parameters
    input_text = data.get("input", "")
    option = data.get("option", "")
    
    # Process the input (this is where your logic would go)
    if option:
        result = f"Processed: {input_text} with option: {option}"
    else:
        result = f"Processed: {input_text}"
    
    return {
        "result": result
    }

if __name__ == "__main__":
    main()
"#;
    
    let main_py_path = format!("{}/main.py", path);
    std::fs::write(&main_py_path, main_py_content)?;
    
    // Create README.md
    let readme_content = format!(r#"# {} MCP Extension

This is a sample MCP extension for {}.

## Installation

1. Copy this directory to your extensions folder
2. Register the extension in your configuration

## Usage

This extension provides a sample tool that processes input text.

## Files

- `nmmcp.json` - Extension metadata
- `main.py` - Entry point script
- `README.md` - This file
"#, path.split("/").last().unwrap_or("extension"), path.split("/").last().unwrap_or("extension"));
    
    let readme_path = format!("{}/README.md", path);
    std::fs::write(&readme_path, readme_content)?;
    
    Ok(())
}

/// Create tool template file
fn create_tool_template_file(path: &str, content: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Ensure prompts directory exists
    let prompts_dir = Path::new("prompts");
    if !prompts_dir.exists() {
        std::fs::create_dir_all(prompts_dir)?;
    }
    
    // Write the template file
    std::fs::write(path, content)?;
    
    Ok(())
}

#[derive(Deserialize)]
struct UiCommand {
    command: String,
    payload: serde_json::Value,
}

#[derive(Serialize)]
struct UiResponse {
    status: String,
    data: serde_json::Value,
}

#[derive(Serialize)]
struct WebMetrics {
    requests_count: u64,
    success_rate: f64,
    average_response_time: f64,
    active_requests: usize,
    alerts: Vec<String>,
}

#[derive(Serialize)]
struct WebTrace {
    id: String,
    timestamp: String,
    service: String,
    status: String,
    duration: String,
    details: String,
}

/// Get list of available POML files
pub async fn get_poml_files() -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut poml_files = Vec::new();
    let prompts_dir = Path::new("prompts");
    
    if prompts_dir.exists() {
        for entry in fs::read_dir(prompts_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("poml") {
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    poml_files.push(file_name.to_string());
                }
            }
        }
        poml_files.sort();
    }
    
    Ok(poml_files)
}

/// Load POML file content
pub async fn load_poml_file(file_name: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let file_path = Path::new("prompts").join(file_name);
    
    if !file_path.exists() {
        return Err(format!("POML file not found: {}", file_name).into());
    }
    
    let content = fs::read_to_string(&file_path)?;
    Ok(content)
}

/// Generate a temporary POML file and return its name
fn generate_temp_poml_file(content: &str) -> String {
    use uuid::Uuid;
    
    let temp_dir = Path::new("prompts");
    if !temp_dir.exists() {
        let _ = std::fs::create_dir_all(temp_dir);
    }
    
    let temp_filename = format!("temp_{}.poml", Uuid::new_v4());
    let temp_path = temp_dir.join(&temp_filename);
    
    if let Err(e) = std::fs::write(&temp_path, content) {
        eprintln!("Failed to write temp POML file {}: {}", temp_path.display(), e);
        return "temp_error.poml".to_string();
    }
    
    temp_filename
}

pub async fn handle_websocket_connection(ws: WebSocket) {
    let (mut tx, mut rx) = ws.split();

    let loaded_workflows = load_all_nm().unwrap_or_else(|_| preset_workflows());
    let mut workflows = HashMap::new();
    for wf in loaded_workflows {
        workflows.insert(wf.name.clone(), wf.clone());
    }
    let active_name = workflows.keys().next().map(|name| name.clone()).unwrap_or_else(|| "default".to_string());
    let (tx_cmd, mut rx_cmd) = mpsc::unbounded_channel();
    let (tx_evt, rx_evt) = mpsc::unbounded_channel();
    let metrics_collector = Arc::new(tokio::sync::Mutex::new(crate::metrics::metrics_collector::MetricsCollector::new()));
    let app = Arc::new(Mutex::new(App::new(tx_cmd, rx_evt, workflows, active_name, Some(metrics_collector.clone()))));

    tokio::spawn(async move {
        while let Some(cmd) = rx_cmd.recv().await {
            crate::runner::run_workflow(cmd, tx_evt.clone(), Some(metrics_collector.clone())).await;
        }
    });

    let (ws_tx, mut ws_rx) = mpsc::unbounded_channel();

    // Task to send outgoing messages to WebSocket
    tokio::spawn(async move {
        while let Some(message) = ws_rx.recv().await {
            if tx.send(message).await.is_err() {
                // connection closed
                break;
            }
        }
    });

    let app_clone = app.clone();
    let ws_tx_clone = ws_tx.clone();

    // Task to handle app events and forward to WebSocket
    tokio::spawn(async move {
        let mut app = app_clone.lock().await;
        while let Some(event) = app.rx.recv().await {
            let msg = match event {
                AppEvent::Log(line) => Message::text(serde_json::to_string(&UiResponse { status: "log".to_string(), data: serde_json::Value::String(line) }).unwrap()),
                AppEvent::RunStart(name) => Message::text(serde_json::to_string(&UiResponse { status: "run_start".to_string(), data: serde_json::Value::String(name) }).unwrap()),
                AppEvent::RunResult(line) => Message::text(serde_json::to_string(&UiResponse { status: "run_result".to_string(), data: serde_json::Value::String(line) }).unwrap()),
                AppEvent::RunEnd(name) => Message::text(serde_json::to_string(&UiResponse { status: "run_end".to_string(), data: serde_json::Value::String(name) }).unwrap()),
                AppEvent::Error(line) => Message::text(serde_json::to_string(&UiResponse { status: "error".to_string(), data: serde_json::Value::String(line) }).unwrap()),
            };
            if ws_tx_clone.send(msg).is_err() {
                // connection closed
                break;
            }
        }
    });

    // Main loop to handle incoming WebSocket messages
    while let Some(result) = rx.next().await {
        if let Ok(msg) = result {
            if msg.is_text() {
                if let Ok(text) = msg.to_str() {
                    if let Ok(cmd) = serde_json::from_str::<UiCommand>(text) {
                        let mut app = app.lock().await;
                        match cmd.command.as_str() {
                            "submit" => {
                                if let Some(input) = cmd.payload.as_str() {
                                    // Handle preprompting with secondary agent inputs
                                    let processed_input = process_preprompting_input(input);
                                    app.input = processed_input;
                                    app.submit();
                                }
                            }
                            "add_node" => {
                                // Logic to add a node based on payload
                                // This requires expanding App's functionality
                            }
                            "get_poml_files" => {
                                // Get list of POML files
                                match get_poml_files().await {
                                    Ok(files) => {
                                        let files_json = serde_json::to_value(files).unwrap();
                                        let response = UiResponse {
                                            status: "poml_files".to_string(),
                                            data: files_json,
                                        };
                                        let msg = Message::text(serde_json::to_string(&response).unwrap());
                                        if ws_tx.send(msg).is_err() {
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        let response = UiResponse {
                                            status: "error".to_string(),
                                            data: serde_json::Value::String(format!("Failed to load POML files: {}", e)),
                                        };
                                        let msg = Message::text(serde_json::to_string(&response).unwrap());
                                        if ws_tx.send(msg).is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                            "load_poml" => {
                                // Load a specific POML file
                                if let Some(file_name) = cmd.payload.get("file").and_then(|v| v.as_str()) {
                                    match load_poml_file(file_name).await {
                                        Ok(content) => {
                                            let response = UiResponse {
                                                status: "poml_content".to_string(),
                                                data: serde_json::Value::String(content),
                                            };
                                            let msg = Message::text(serde_json::to_string(&response).unwrap());
                                            if ws_tx.send(msg).is_err() {
                                                break;
                                            }
                                        }
                                        Err(e) => {
                                            let response = UiResponse {
                                                status: "error".to_string(),
                                                data: serde_json::Value::String(format!("Failed to load POML file: {}", e)),
                                            };
                                            let msg = Message::text(serde_json::to_string(&response).unwrap());
                                            if ws_tx.send(msg).is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            "run_workflow" => {
                                // Run a specific workflow
                                if let Some(workflow_name) = cmd.payload.get("workflow_name").and_then(|v| v.as_str()) {
                                    let prompt = cmd.payload.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
                                    
                                    // Get the workflow configuration
                                    if let Some(cfg) = app.workflows.get(workflow_name) {
                                        let _ = app.tx.send(AppCommand::RunWorkflow {
                                            workflow_name: workflow_name.to_string(),
                                            prompt: prompt.to_string(),
                                            cfg: cfg.clone(),
                                            start_agent: None,
                                        });
                                        
                                        let response = UiResponse {
                                            status: "workflow_run_started".to_string(),
                                            data: serde_json::Value::String(format!("Started workflow: {}", workflow_name)),
                                        };
                                        let msg = Message::text(serde_json::to_string(&response).unwrap());
                                        if ws_tx.send(msg).is_err() {
                                            break;
                                        }
                                    } else {
                                        let response = UiResponse {
                                            status: "error".to_string(),
                                            data: serde_json::Value::String(format!("Workflow '{}' not found", workflow_name)),
                                        };
                                        let msg = Message::text(serde_json::to_string(&response).unwrap());
                                        if ws_tx.send(msg).is_err() {
                                            break;
                                        }
                                    }
                                } else {
                                    let response = UiResponse {
                                        status: "error".to_string(),
                                        data: serde_json::Value::String("Missing workflow_name parameter".to_string()),
                                    };
                                    let msg = Message::text(serde_json::to_string(&response).unwrap());
                                    if ws_tx.send(msg).is_err() {
                                        break;
                                    }
                                }
                            }
                            "run_all_workflows" => {
                                // Run all available workflows
                                let workflow_names: Vec<String> = app.workflows.keys().cloned().collect();
                                let mut started_count = 0;
                                
                                for workflow_name in &workflow_names {
                                    if let Some(cfg) = app.workflows.get(workflow_name) {
                                        let _ = app.tx.send(AppCommand::RunWorkflow {
                                            workflow_name: workflow_name.to_string(),
                                            prompt: "Run all".to_string(),
                                            cfg: cfg.clone(),
                                            start_agent: None,
                                        });
                                        started_count += 1;
                                    }
                                }
                                
                                let response = UiResponse {
                                    status: "all_workflows_run_started".to_string(),
                                    data: serde_json::Value::String(format!("Started {} workflows", started_count)),
                                };
                                let msg = Message::text(serde_json::to_string(&response).unwrap());
                                if ws_tx.send(msg).is_err() {
                                    break;
                                }
                            }
                            "run_poml" => {
                                // Run POML content by creating a temporary workflow
                                let content = if let Some(content_str) = cmd.payload.get("content").and_then(|v| v.as_str()) {
                                    content_str.to_string()
                                } else if let Some(obj) = cmd.payload.get("content") {
                                    // Handle object payload with content and format
                                    obj.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string()
                                } else {
                                    "".to_string()
                                };
                                
                                // Get format parameter if provided
                                let _format = if let Some(obj) = cmd.payload.get("content") {
                                    obj.get("format").and_then(|v| v.as_str()).unwrap_or("html").to_string()
                                } else {
                                    "html".to_string()
                                };
                                
                                if !content.is_empty() {
                                    // Try to create a simple workflow from POML content
                                    let temp_workflow_name = "temp_poml_workflow";
                                    
                                    // Create a basic workflow config
                                    let temp_cfg = WorkflowConfig {
                                        name: temp_workflow_name.to_string(),
                                        model: "gpt-3.5-turbo".to_string(),
                                        temperature: 0.7,
                                        maximum_traversals: 10,
                                        working_dir: ".".to_string(),
                                        active_agent_index: 0,
                                        rows: vec![crate::nm_config::AgentRow {
                                            agent_type: crate::nm_config::AgentType::Agent,
                                            files: format!("user:temp_poml:{}", generate_temp_poml_file(&content)),
                                            max_iterations: 3,
                                            iteration_delay_ms: 200,
                                            on_success: None,
                                            on_failure: None,
                                            input_injections: Vec::new(),
                                            output_injections: Vec::new(),
                                        }],
                                    };
                                    
                                    // Save the temp workflow
                                    app.workflows.insert(temp_workflow_name.to_string(), temp_cfg.clone());
                                    
                                    // Run the temp workflow
                                    let _ = app.tx.send(AppCommand::RunWorkflow {
                                        workflow_name: temp_workflow_name.to_string(),
                                        prompt: cmd.payload.get("prompt").and_then(|v| v.as_str()).unwrap_or("Execute POML workflow").to_string(),
                                        cfg: temp_cfg,
                                        start_agent: None,
                                    });
                                    
                                    let response = UiResponse {
                                        status: "poml_run_started".to_string(),
                                        data: serde_json::Value::String("POML workflow started".to_string()),
                                    };
                                    let msg = Message::text(serde_json::to_string(&response).unwrap());
                                    if ws_tx.send(msg).is_err() {
                                        break;
                                    }
                                }
                            }
                            "save_poml" => {
                                // Save POML content (placeholder - would need file system access)
                                let response = UiResponse {
                                    status: "poml_save_success".to_string(),
                                    data: serde_json::Value::String("POML save functionality not yet implemented".to_string()),
                                };
                                let msg = Message::text(serde_json::to_string(&response).unwrap());
                                if ws_tx.send(msg).is_err() {
                                    break;
                                }
                            }
                            "validate_poml" => {
                                // Validate POML content (placeholder)
                                let response = UiResponse {
                                    status: "poml_validate_success".to_string(),
                                    data: serde_json::Value::String("POML validation functionality not yet implemented".to_string()),
                                };
                                let msg = Message::text(serde_json::to_string(&response).unwrap());
                                if ws_tx.send(msg).is_err() {
                                    break;
                                }
                            }
                            "send_poml_to_editor" => {
                                // Send POML content to the editor
                                if let Some(content) = cmd.payload.get("content").and_then(|v| v.as_str()) {
                                    let file_name = cmd.payload.get("file_name").and_then(|v| v.as_str()).unwrap_or("");
                                    let response = UiResponse {
                                        status: "load_poml_content".to_string(),
                                        data: serde_json::json!({
                                            "content": content,
                                            "file_name": file_name
                                        }),
                                    };
                                    let msg = Message::text(serde_json::to_string(&response).unwrap());
                                    if ws_tx.send(msg).is_err() {
                                        break;
                                    }
                                }
                            }
                            "create_template" => {
                                // Create MCP or tool template
                                let template_type = cmd.payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                let template_name = cmd.payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                
                                if template_type.is_empty() || template_name.is_empty() {
                                    let response = UiResponse {
                                        status: "error".to_string(),
                                        data: serde_json::Value::String("Missing template type or name".to_string()),
                                    };
                                    let msg = Message::text(serde_json::to_string(&response).unwrap());
                                    if ws_tx.send(msg).is_err() {
                                        break;
                                    }
                                } else {
                                    match template_type {
                                        "mcp" => {
                                            // Create MCP template
                                            let template_content = generate_mcp_template(template_name);
                                            let file_path = format!("extensions/ext_{}", template_name);
                                            
                                            // Create directory and files
                                            if let Err(e) = create_mcp_template_structure(&file_path, &template_content) {
                                                let response = UiResponse {
                                                    status: "error".to_string(),
                                                    data: serde_json::Value::String(format!("Failed to create MCP template: {}", e)),
                                                };
                                                let msg = Message::text(serde_json::to_string(&response).unwrap());
                                                if ws_tx.send(msg).is_err() {
                                                    break;
                                                }
                                            } else {
                                                let response = UiResponse {
                                                    status: "template_created".to_string(),
                                                    data: serde_json::Value::String(format!("MCP template '{}' created at {}", template_name, file_path)),
                                                };
                                                let msg = Message::text(serde_json::to_string(&response).unwrap());
                                                if ws_tx.send(msg).is_err() {
                                                    break;
                                                }
                                            }
                                        }
                                        "tool" => {
                                            // Create tool template
                                            let template_content = generate_tool_template(template_name);
                                            let file_path = format!("prompts/{}_tool.poml", template_name);
                                            
                                            // Create tool file
                                            if let Err(e) = create_tool_template_file(&file_path, &template_content) {
                                                let response = UiResponse {
                                                    status: "error".to_string(),
                                                    data: serde_json::Value::String(format!("Failed to create tool template: {}", e)),
                                                };
                                                let msg = Message::text(serde_json::to_string(&response).unwrap());
                                                if ws_tx.send(msg).is_err() {
                                                    break;
                                                }
                                            } else {
                                                let response = UiResponse {
                                                    status: "template_created".to_string(),
                                                    data: serde_json::Value::String(format!("Tool template '{}' created at {}", template_name, file_path)),
                                                };
                                                let msg = Message::text(serde_json::to_string(&response).unwrap());
                                                if ws_tx.send(msg).is_err() {
                                                    break;
                                                }
                                            }
                                        }
                                        _ => {
                                            let response = UiResponse {
                                                status: "error".to_string(),
                                                data: serde_json::Value::String(format!("Unknown template type: {}", template_type)),
                                            };
                                            let msg = Message::text(serde_json::to_string(&response).unwrap());
                                            if ws_tx.send(msg).is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            // Handle other commands like "connect_nodes", "delete_node", etc.
                            _ => {
                                // unhandled command
                            }
                        }
                    }
                }
            }
        }
    }
}
