use crate::shared_history::SharedHistory;
use crate::tools::builtin_tools_with_history;
use llmgraph::Graph;
use tokio::sync::mpsc::UnboundedSender;
use crate::metrics::metrics_collector::MetricsCollector;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::Path;

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

pub enum AppCommand {
    RunWorkflow {
        workflow_name: String,
        prompt: String,
        cfg: crate::nm_config::WorkflowConfig,
        start_agent: Option<i32>,
    },
    ShowHistory {
        agent_index: Option<i32>,
        workflow_name: String,
        cfg: crate::nm_config::WorkflowConfig,
    },
    CreateTemplate {
        template_type: String,
        template_name: String,
    },
}

pub enum AppEvent {
    Log(String),
    RunStart(String),
    RunResult(String),
    RunEnd(String),
    Error(String),
}

pub async fn run_workflow(
    cmd: AppCommand,
    log_tx: UnboundedSender<AppEvent>,
    metrics: Option<Arc<Mutex<MetricsCollector>>>,
) {
    match cmd {
        AppCommand::ShowHistory { agent_index, workflow_name, cfg: _ } => {
            let _ = log_tx.send(AppEvent::Log(format!(
                "Showing history for workflow '{}', agent {:?}",
                workflow_name, agent_index
            )));
            let _ = log_tx.send(AppEvent::RunResult("History display not yet implemented".to_string()));
        }
        AppCommand::CreateTemplate { template_type, template_name } => {
            let _ = log_tx.send(AppEvent::Log(format!(
                "Creating {} template: {}", 
                template_type, template_name
            )));

            match template_type.as_str() {
                "mcp" => {
                    // Create MCP template
                    let template_content = generate_mcp_template(&template_name);
                    let file_path = format!("extensions/ext_{}", template_name);
                    
                    // Create directory and files
                    match create_mcp_template_structure(&file_path, &template_content) {
                        Ok(_) => {
                            let _ = log_tx.send(AppEvent::Log(format!(
                                "MCP template '{}' created at {}", 
                                template_name, file_path
                            )));
                        }
                        Err(e) => {
                            let _ = log_tx.send(AppEvent::Error(format!(
                                "Failed to create MCP template: {}", e
                            )));
                        }
                    }
                }
                "tool" => {
                    // Create tool template
                    let template_content = generate_tool_template(&template_name);
                    let file_path = format!("prompts/{}_tool.poml", template_name);
                    
                    // Create tool file
                    match create_tool_template_file(&file_path, &template_content) {
                        Ok(_) => {
                            let _ = log_tx.send(AppEvent::Log(format!(
                                "Tool template '{}' created at {}", 
                                template_name, file_path
                            )));
                        }
                        Err(e) => {
                            let _ = log_tx.send(AppEvent::Error(format!(
                                "Failed to create tool template: {}", e
                            )));
                        }
                    }
                }
                _ => {
                    let _ = log_tx.send(AppEvent::Error(format!(
                        "Unknown template type: {}", template_type
                    )));
                }
            }
        }

        AppCommand::RunWorkflow { workflow_name, prompt, cfg, start_agent } => {
            let _ = log_tx.send(AppEvent::RunStart(workflow_name.clone()));
            let _ = log_tx.send(AppEvent::Log(format!(
                "Starting workflow '{}' with prompt: {}", 
                workflow_name, 
                prompt
            )));

            // ✅ Create shared history
            let shared_history = SharedHistory::new();
            let _ = log_tx.send(AppEvent::Log(
                "[SharedHistory] Initialized global shared history".to_string(),
            ));

            // ✅ Register tools
            let mut graph = Graph::new();
            for (tool, func) in builtin_tools_with_history(
                shared_history.clone(),
                log_tx.clone(),
                cfg.working_dir.clone(),
            ) {
                graph.register_tool(tool, func);
            }

            // Build graph nodes
            for (i, row) in cfg.rows.iter().enumerate() {
                let next_id = if i + 1 < cfg.rows.len() {
                    Some((i + 1) as i32)
                } else {
                    None
                };

                let files: Vec<String> = row
                    .files
                    .split(';')
                    .map(|s| s.trim().to_string())
                    .collect();

                let agent: Box<dyn llmgraph::models::graph::Agent + Send + Sync> =
                    if row.agent_type == crate::nm_config::AgentType::Validator {
                        Box::new(crate::agents::PomlValidatorAgent::new(
                            crate::agents::PomlAgent::new(
                                &format!("ValidatorAgent{}", i + 1),
                                files.clone(),
                                cfg.model.clone(),
                                cfg.temperature,
                                row.max_iterations,
                                log_tx.clone(),
                                shared_history.clone(),
                            ),
                            row.on_success.unwrap_or(-1),
                            row.on_failure.unwrap_or(-1),
                        ))
                    } else {
                        Box::new(crate::agents::PomlAgent::new(
                            &format!("Agent{}", i + 1),
                            files.clone(),
                            cfg.model.clone(),
                            cfg.temperature,
                            row.max_iterations,
                            log_tx.clone(),
                            shared_history.clone(),
                        ))
                    };

                let chained = crate::agents::ChainedAgent::new(
                    i as i32,
                    agent,
                    log_tx.clone(),
                    next_id,
                    row.max_iterations,
                    row.iteration_delay_ms,
                    shared_history.clone(),
                );
                graph.add_node(i as i32, Box::new(chained));
            }

            // ✅ Traversal loop
            let mut current_node = start_agent.unwrap_or(0) as i32;
            let mut current_input = prompt.clone();
            let mut traversals = 0;
            let max_traversals = cfg.maximum_traversals;

            let metrics_collector = metrics.unwrap_or_else(|| Arc::new(Mutex::new(MetricsCollector::new())));
            let _request_id = metrics_collector
                .lock().await
                .start_request("workflow_execution".to_string()).await;

            while traversals < max_traversals {
                traversals += 1;

                let msg = format!(
                    "Traversal {}: Starting at node {} with input: {}",
                    traversals, current_node, current_input
                );
                let _ = log_tx.send(AppEvent::Log(msg.clone()));

                let step_start = std::time::Instant::now();
                let step_output = graph.run(current_node, &current_input).await;
                let _step_duration = step_start.elapsed();

                let _ = metrics_collector
                    .lock().await
                    .finish_request(format!("step_{}", traversals), true).await;

                // Log step result
                let _ = log_tx.send(AppEvent::RunResult(format!(
                    "Traversal {} (node {}):\n{}",
                    traversals, current_node, step_output
                )));

                // Detect explicit routing marker
                if let Some(route_idx) = step_output.rfind("\n__ROUTE__=") {
                    let route_str = &step_output[route_idx + 11..];
                    if let Ok(next) = route_str.trim().parse::<i32>() {
                        current_node = next;
                        current_input = step_output[..route_idx].trim().to_string();
                        continue;
                    }
                }

                // Default routing: go to next node if it exists
                if (current_node as usize) + 1 < cfg.rows.len() {
                    current_node += 1;
                    current_input = step_output.clone();
                    continue;
                }

                // No next node → stop
                break;
            }

            // ✅ Final metrics + alerts
            let final_metrics = metrics_collector.lock().await.get_metrics().await;
            let alerts = metrics_collector.lock().await.get_alerts().await;

            for alert in alerts {
                let _ = log_tx.send(AppEvent::Log(format!(
                    "[ALERT][{}] {}",
                    alert.level,
                    alert.message
                )));
            }

            let _ = log_tx.send(AppEvent::RunResult(format!(
                "Workflow completed. Metrics: {} requests, {:.2}% success rate, avg {:.2}ms response time",
                final_metrics.request_count,
                final_metrics.get_success_rate() * 100.0,
                final_metrics.average_response_time.num_milliseconds()
            )));

            let _ = log_tx.send(AppEvent::RunEnd(workflow_name));
        }
    }
}