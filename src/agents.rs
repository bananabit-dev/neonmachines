use async_trait::async_trait;
use dotenv::dotenv;
use std::env;
use std::process::Command;
use llmgraph::models::graph::Agent;
use llmgraph::models::tools::{Message, ToolRegistryTrait};
use llmgraph::generate::generate::generate_full_response;
use crate::runner::AppEvent;
use tokio::sync::mpsc::UnboundedSender;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Validation result structure for explicit validation responses
#[derive(Debug, Deserialize, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Option<Vec<String>>,
    pub data: Option<Value>,
}

/// Run a `.poml` file using Python with variable substitution
fn run_poml_file_with_vars(file: &str, vars: &HashMap<String, String>) -> String {
    let path = format!("./prompts/{}", file);
    
    // First read the file and substitute variables
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) => return format!("Failed to read {}: {}", path, e),
    };
    
    // Replace template variables like {{prompt}} with actual values
    let mut processed_content = content;
    for (key, value) in vars.iter() {
        let template = format!("{{{{{}}}}}", key);
        processed_content = processed_content.replace(&template, value);
    }
    
    // Write to a temporary file
    let temp_path = format!("{}.tmp", path);
    if let Err(e) = std::fs::write(&temp_path, &processed_content) {
        return format!("Failed to write temp file: {}", e);
    }
    
    // Run the processed POML file
    let result = match Command::new("python")
        .args(["-m", "poml", "-f", &temp_path])
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout).to_string()
            } else {
                format!(
                    "Error running {}: {}",
                    path,
                    String::from_utf8_lossy(&output.stderr)
                )
            }
        }
        Err(e) => format!("Failed to run {}: {}", path, e),
    };
    
    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);
    
    result
}

/// Run a `.poml` file using Python (backward compatibility)
fn run_poml_file(file: &str) -> String {
    run_poml_file_with_vars(file, &HashMap::new())
}

/// Agent that executes `.poml` files and uses their output as context
pub struct PomlAgent {
    pub name: String,
    pub files: Vec<String>,   // semicolon-separated role:file entries
    pub model: String,
    pub temperature: f32,
}

impl PomlAgent {
    pub fn new(name: &str, files: Vec<String>, model: String, temperature: f32) -> Self {
        Self {
            name: name.to_string(),
            files,
            model,
            temperature,
        }
    }

    fn load_messages(&self, input: &str) -> Vec<Message> {
        let mut messages = Vec::new();

        // Create variables map for template substitution
        let mut vars = HashMap::new();
        vars.insert("prompt".to_string(), input.to_string());
        vars.insert("input".to_string(), input.to_string());

        for entry in &self.files {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }

            // Expect format: role:system:file.poml
            let parts: Vec<&str> = entry.splitn(3, ':').collect();
            if parts.len() == 3 {
                let role = parts[1].trim();
                let file = parts[2].trim();

                let out = run_poml_file_with_vars(file, &vars);

                messages.push(Message {
                    role: role.to_string(),
                    content: Some(out),
                    tool_calls: None,
                });
            }
        }

        messages
    }
}

#[async_trait]
impl Agent for PomlAgent {
    async fn run(
        &mut self,
        input: &str,
        tool_registry: &(dyn ToolRegistryTrait + Send + Sync),
    ) -> (String, Option<i32>) {
        dotenv().ok();
        let api_key = env::var("API_KEY").unwrap_or_default();
        let base_url = "https://openrouter.ai/api/v1/chat/completions".to_string();

        // Load messages with input variables substituted
        let mut messages = self.load_messages(input);

        // Always append the incoming input (from user or previous agent) if not already in messages
        if !input.trim().is_empty() {
            messages.push(Message {
                role: "user".into(),
                content: Some(input.to_string()),
                tool_calls: None,
            });
        }

        let tools = tool_registry.get_tools();
        let max_iterations = 5;
        let mut iteration = 0;

        loop {
            iteration += 1;
            if iteration > max_iterations {
                return ("Max iterations reached".into(), None);
            }

            let resp = generate_full_response(
                base_url.clone(),
                api_key.clone(),
                self.model.clone(),
                self.temperature,
                messages.clone(),
                Some(tools.clone()),
            )
            .await;

            match resp {
                Ok(llm) => {
                    let choice = &llm.choices[0];
                    let msg = &choice.message;

                    // Add assistant message
                    messages.push(Message {
                        role: "assistant".into(),
                        content: msg.content.clone(),
                        tool_calls: msg.tool_calls.clone(),
                    });

                    if let Some(tool_calls) = &msg.tool_calls {
                        // Execute each tool
                        for tc in tool_calls {
                            let result = tool_registry.execute_tool(
                                &tc.function.name,
                                &tc.function.arguments,
                            );
                            let content = match result {
                                Ok(v) => serde_json::to_string(&v).unwrap(),
                                Err(e) => format!("Error: {}", e),
                            };
                            messages.push(Message {
                                role: "tool".into(),
                                content: Some(content),
                                tool_calls: None,
                            });
                        }
                        // Continue loop to let LLM process tool results
                        continue;
                    }

                    if let Some(content) = &msg.content {
                        return (content.clone(), None);
                    }
                }
                Err(e) => return (format!("Error: {}", e), None),
            }
        }
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}

/// Wraps an agent and handles routing decisions
pub struct ChainedAgent {
    inner: Box<dyn Agent>,
    next: Option<i32>,
    id: i32,
    tx: UnboundedSender<AppEvent>,
}

impl ChainedAgent {
    pub fn new(inner: Box<dyn Agent>, next: Option<i32>, id: i32, tx: UnboundedSender<AppEvent>) -> Self {
        Self { inner, next, id, tx }
    }
}

/// A ValidatorAgent that uses POML files for validation logic
pub struct PomlValidatorAgent {
    poml_agent: PomlAgent,
    success_route: i32,
    failure_route: i32,
}

impl PomlValidatorAgent {
    pub fn new(poml_agent: PomlAgent, success_route: i32, failure_route: i32) -> Self {
        Self {
            poml_agent,
            success_route,
            failure_route,
        }
    }
}

#[async_trait]
impl Agent for PomlValidatorAgent {
    async fn run(
        &mut self,
        input: &str,
        tool_registry: &(dyn ToolRegistryTrait + Send + Sync),
    ) -> (String, Option<i32>) {
        // Run the POML validation logic
        let (validation_result, _) = self.poml_agent.run(input, tool_registry).await;
        
        // Determine if the response is valid
        let is_valid = {
            // First, check if it's an explicit ValidationResult with a "valid" field
            if let Ok(json_result) = serde_json::from_str::<ValidationResult>(&validation_result) {
                json_result.valid
            } else {
                // Try to extract and validate any JSON structure from the response
                let mut json_candidates = vec![
                    // Try the entire response as JSON
                    Some(validation_result.clone()),
                    // Try extracting JSON object
                    extract_json(&validation_result, '{', '}'),
                    // Try extracting JSON array
                    extract_json(&validation_result, '[', ']'),
                ];
                
                // Check if any candidate is valid JSON
                let mut found_valid_json = false;
                for candidate in json_candidates.drain(..) {
                    if let Some(json_str) = candidate {
                        if let Ok(value) = serde_json::from_str::<Value>(&json_str) {
                            // Check if it has an explicit "valid" field
                            if let Some(valid) = value.get("valid").and_then(|v| v.as_bool()) {
                                found_valid_json = valid;
                                break;
                            } else {
                                // Any well-formed JSON without explicit "valid": false is considered valid
                                found_valid_json = true;
                                break;
                            }
                        }
                    }
                }
                found_valid_json
            }
        };
        
        if is_valid {
            // Validation passed - route to success path
            (validation_result, Some(self.success_route))
        } else {
            // Validation failed - route to failure path
            let failure_msg = format!(
                "Validation failed: No valid JSON structure found or explicit validation failure\n\nInput: {}\n\nValidator response: {}",
                input.lines().next().unwrap_or(input),
                validation_result
            );
            (failure_msg, Some(self.failure_route))
        }
    }

    fn get_name(&self) -> &str {
        self.poml_agent.get_name()
    }
}

/// Helper function to extract JSON from text
fn extract_json(text: &str, start_char: char, end_char: char) -> Option<String> {
    let start = text.find(start_char)?;
    let end = text.rfind(end_char)?;
    if start <= end {
        Some(text[start..=end].to_string())
    } else {
        None
    }
}

#[async_trait]
impl Agent for ChainedAgent {
    async fn run(
        &mut self,
        input: &str,
        tool_registry: &(dyn ToolRegistryTrait + Send + Sync),
    ) -> (String, Option<i32>) {
        let (output, route_decision) = self.inner.run(input, tool_registry).await;

        // Log intermediate output
        let _ = self.tx.send(AppEvent::RunResult(format!(
            "Agent {} output:\n{}",
            self.id + 1,
            output
        )));

        // Use the agent's routing decision if it provided one (e.g., ValidatorAgent)
        // Otherwise fall back to the configured next node
        let next_node = route_decision.or(self.next);
        
        // Log routing decision
        if let Some(next) = next_node {
            let _ = self.tx.send(AppEvent::Log(format!(
                "Agent {} routing to node {}",
                self.id + 1,
                if next == -1 { "END".to_string() } else { (next + 1).to_string() }
            )));
        } else {
            let _ = self.tx.send(AppEvent::Log(format!(
                "Agent {} completed with no routing decision",
                self.id + 1
            )));
        }

        // Encode the routing decision in the output for the Graph to use
        // Add a special marker at the end that can be parsed
        let output_with_routing = if let Some(next) = next_node {
            format!("{}\n__ROUTE__:{}", output, next)
        } else {
            output
        };

        (output_with_routing, next_node)
    }

    fn get_name(&self) -> &str {
        self.inner.get_name()
    }
}