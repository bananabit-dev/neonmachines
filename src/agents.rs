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
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) => return format!("Failed to read {}: {}", path, e),
    };
    let mut processed_content = content;
    for (key, value) in vars.iter() {
        let template = format!("{{{{{}}}}}", key);
        processed_content = processed_content.replace(&template, value);
    }
    let temp_path = format!("{}.tmp", path);
    if let Err(e) = std::fs::write(&temp_path, &processed_content) {
        return format!("Failed to write temp file: {}", e);
    }
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
    let _ = std::fs::remove_file(&temp_path);
    result
}

/// Agent that executes `.poml` files
pub struct PomlAgent {
    pub name: String,
    pub files: Vec<String>,
    pub model: String,
    pub temperature: f32,
    pub max_iterations: usize,
    pub tx: UnboundedSender<AppEvent>, // ✅ logging channel
}

impl PomlAgent {
    pub fn new(
        name: &str,
        files: Vec<String>,
        model: String,
        temperature: f32,
        max_iterations: usize,
        tx: UnboundedSender<AppEvent>,
    ) -> Self {
        Self {
            name: name.to_string(),
            files,
            model,
            temperature,
            max_iterations,
            tx,
        }
    }

    fn load_messages(&self, input: &str) -> Vec<Message> {
        let mut messages = Vec::new();
        let mut vars = HashMap::new();
        vars.insert("prompt".to_string(), input.to_string());
        vars.insert("input".to_string(), input.to_string());

        for entry in &self.files {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
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

        let mut messages = self.load_messages(input);
        if !input.trim().is_empty() {
            messages.push(Message {
                role: "user".into(),
                content: Some(input.to_string()),
                tool_calls: None,
            });
        }

        let tools = tool_registry.get_tools();
        let mut iteration = 0;

        loop {
            iteration += 1;
            if iteration > self.max_iterations {
                return ("Error: Max iterations reached".into(), None);
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
                    messages.push(Message {
                        role: "assistant".into(),
                        content: msg.content.clone(),
                        tool_calls: msg.tool_calls.clone(),
                    });

                    if let Some(tool_calls) = &msg.tool_calls {
                        let mut tool_outputs = Vec::new();
                        for tc in tool_calls {
                            // ✅ Log tool call
                            let _ = self.tx.send(AppEvent::Log(format!(
                                "Agent {} calling tool '{}' with args: {}",
                                self.name,
                                tc.function.name,
                                tc.function.arguments
                            )));

                            let result = tool_registry.execute_tool(
                                &tc.function.name,
                                &tc.function.arguments,
                            );
                            let content = match result {
                                Ok(v) => {
                                    let json = serde_json::to_string(&v).unwrap();
                                    let _ = self.tx.send(AppEvent::Log(format!(
                                        "Agent {} tool '{}' result: {}",
                                        self.name,
                                        tc.function.name,
                                        json
                                    )));
                                    json
                                }
                                Err(e) => {
                                    let err = format!("Error: {}", e);
                                    let _ = self.tx.send(AppEvent::Log(format!(
                                        "Agent {} tool '{}' failed: {}",
                                        self.name,
                                        tc.function.name,
                                        e
                                    )));
                                    err
                                }
                            };
                            tool_outputs.push(format!("Tool {} result: {}", tc.function.name, content));
                            messages.push(Message {
                                role: "tool".into(),
                                content: Some(content),
                                tool_calls: None,
                            });
                        }

                        // If no assistant content, return tool outputs
                        if msg.content.is_none() {
                            return (tool_outputs.join("\n"), None);
                        }

                        continue;
                    }

                    if let Some(content) = &msg.content {
                        return (content.clone(), None);
                    }
                }
                Err(e) => {
                    return (format!("Error: {}", e), None);
                }
            }
        }
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}

/// Validator agent
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
        let (validation_result, _) = self.poml_agent.run(input, tool_registry).await;
        let is_valid = {
            if let Ok(json_result) = serde_json::from_str::<ValidationResult>(&validation_result) {
                json_result.valid
            } else {
                let mut candidates = vec![
                    Some(validation_result.clone()),
                    extract_json(&validation_result, '{', '}'),
                    extract_json(&validation_result, '[', ']'),
                ];
                let mut found = false;
                for candidate in candidates.drain(..) {
                    if let Some(json_str) = candidate {
                        if let Ok(value) = serde_json::from_str::<Value>(&json_str) {
                            if let Some(valid) = value.get("valid").and_then(|v| v.as_bool()) {
                                found = valid;
                                break;
                            } else {
                                found = true;
                                break;
                            }
                        }
                    }
                }
                found
            }
        };
        if is_valid {
            (validation_result, Some(self.success_route))
        } else {
            let failure_msg = format!(
                "Validation failed\nInput: {}\nResponse: {}",
                input, validation_result
            );
            (failure_msg, Some(self.failure_route))
        }
    }

    fn get_name(&self) -> &str {
        self.poml_agent.get_name()
    }
}

fn extract_json(text: &str, start_char: char, end_char: char) -> Option<String> {
    let start = text.find(start_char)?;
    let end = text.rfind(end_char)?;
    if start <= end {
        Some(text[start..=end].to_string())
    } else {
        None
    }
}

/// ChainedAgent with history + logging
pub struct ChainedAgent {
    inner: Box<dyn Agent>,
    next: Option<i32>,
    id: i32,
    tx: UnboundedSender<AppEvent>,
    history: Vec<Message>,
}

impl ChainedAgent {
    pub fn new(inner: Box<dyn Agent>, next: Option<i32>, id: i32, tx: UnboundedSender<AppEvent>) -> Self {
        Self { inner, next, id, tx, history: Vec::new() }
    }
}

#[async_trait]
impl Agent for ChainedAgent {
    async fn run(
        &mut self,
        input: &str,
        tool_registry: &(dyn ToolRegistryTrait + Send + Sync),
    ) -> (String, Option<i32>) {
        // Special case: show history
        if input == "__SHOW_HISTORY__" {
            let mut dump = format!("--- History for Agent {} ---\n", self.id + 1);
            for msg in &self.history {
                if let Some(content) = &msg.content {
                    dump.push_str(&format!("{}: {}\n", msg.role, content));
                }
            }
            return (dump, None);
        }

        // Build input with history
        let mut combined_input = String::new();
        for msg in &self.history {
            if let Some(content) = &msg.content {
                combined_input.push_str(&format!("{}: {}\n", msg.role, content));
            }
        }
        combined_input.push_str(&format!("user: {}\n", input));

        let (output, route_decision) = self.inner.run(&combined_input, tool_registry).await;

        // Save to history
        self.history.push(Message {
            role: "user".into(),
            content: Some(input.to_string()),
            tool_calls: None,
        });
        self.history.push(Message {
            role: "assistant".into(),
            content: Some(output.clone()),
            tool_calls: None,
        });

        // ✅ Log separately
        if output.starts_with("Error:") {
            let _ = self.tx.send(AppEvent::Log(format!(
                "Agent {} encountered an error: {}",
                self.id + 1,
                output
            )));
        } else {
            let _ = self.tx.send(AppEvent::Log(format!(
                "Agent {} produced output ({} chars)",
                self.id + 1,
                output.len()
            )));
            let _ = self.tx.send(AppEvent::RunResult(format!(
                "Agent {} output:\n{}",
                self.id + 1,
                output
            )));
        }

        let next_node = route_decision.or(self.next);
        let output_with_routing = if let Some(next) = next_node {
            if output.starts_with("Error:") {
                output // don’t append __ROUTE__ on errors
            } else {
                format!("{}\n__ROUTE__:{}", output, next)
            }
        } else {
            output
        };
        (output_with_routing, next_node)
    }

    fn get_name(&self) -> &str {
        self.inner.get_name()
    }
}