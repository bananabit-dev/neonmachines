use crate::runner::AppEvent;
use crate::shared_history::SharedHistory;
use async_trait::async_trait;
use dotenv::dotenv;
use llmgraph::generate::generate::generate_full_response;
use llmgraph::models::graph::Agent;
use llmgraph::models::tools::{Message, ToolRegistryTrait};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::process::Command;
use tokio::sync::mpsc::UnboundedSender;
use regex::Regex;

/// Validation result structure for explicit validation responses
#[derive(Debug, Deserialize, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Option<Vec<String>>,
    pub data: Option<serde_json::Value>,
}

/// Inject or overwrite `<let>` variables directly in the `.poml` file
fn inject_let_variables_in_file(
    file: &str,
    vars: &HashMap<String, String>,
    original_input: &str,
    latest_output: &str,
    log_tx: &UnboundedSender<AppEvent>,
) -> std::io::Result<()> {
    let path = format!("./prompts/{}", file);

    let _ = log_tx.send(AppEvent::Log(format!(
        "[DEBUG] Injecting <let> variables into POML file: {}",
        path
    )));

    let content = std::fs::read_to_string(&path)?;
    let mut processed = content.clone();

    let re = Regex::new(
        r#"<let\s+name="([^"]+)"[^>]*>(.*?)</let>|<let\s+name="([^"]+)"[^>]*/>"#,
    )
    .unwrap();

    // ✅ Always overwrite nminput and nmoutput
    let mut replacements: HashMap<String, String> = vars.clone();
    replacements.insert("nminput".to_string(), original_input.to_string());
    replacements.insert("nmoutput".to_string(), latest_output.to_string());

    for value in replacements.values_mut() {
        *value = value.replace('\n', " ").replace('\r', " ");
    }

    for (key, value) in &replacements {
        processed = re
            .replace_all(&processed, |caps: &regex::Captures| {
                let name = caps
                    .get(1)
                    .map(|m| m.as_str())
                    .or_else(|| caps.get(3).map(|m| m.as_str()))
                    .unwrap_or("");

                if name == key {
                    return format!(r#"<let name="{}">{}</let>"#, name, value);
                }
                caps[0].to_string()
            })
            .to_string();
    }

    // Ensure nminput and nmoutput exist
    if !processed.contains("name=\"nminput\"") {
        processed = format!(
            r#"<let name="nminput">{}</let>\n{}"#,
            replacements["nminput"], processed
        );
    }
    if !processed.contains("name=\"nmoutput\"") {
        processed = format!(
            r#"<let name="nmoutput">{}</let>\n{}"#,
            replacements["nmoutput"], processed
        );
    }

    std::fs::write(&path, processed)?;

    let _ = log_tx.send(AppEvent::Log(format!(
        "[DEBUG] Updated POML file written: {}",
        path
    )));

    Ok(())
}

fn run_poml_file_with_vars(
    file: &str,
    vars: &HashMap<String, String>,
    original_input: &str,
    latest_output: &str,
    log_tx: &UnboundedSender<AppEvent>,
) -> String {
    let path = format!("./prompts/{}", file);

    let _ = log_tx.send(AppEvent::Log(format!(
        "[DEBUG] Running POML file: {}",
        path
    )));

    if let Err(e) = inject_let_variables_in_file(
        file,
        vars,
        original_input,
        latest_output,
        log_tx,
    ) {
        return format!("Failed to update {}: {}", file, e);
    }

    let result = match Command::new("python")
        .args(["-m", "poml", "-f", &path])
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

    result
}

/// Agent that executes `.poml` files
pub struct PomlAgent {
    pub name: String,
    pub files: Vec<String>,
    pub model: String,
    pub temperature: f32,
    pub max_iterations: usize,
    pub tx: UnboundedSender<AppEvent>,
    pub original_prompt: Option<String>,
    pub shared_history: SharedHistory,
    pub history: Vec<Message>,   // ✅ local message history
}

impl PomlAgent {
    pub fn new(
        name: &str,
        files: Vec<String>,
        model: String,
        temperature: f32,
        max_iterations: usize,
        tx: UnboundedSender<AppEvent>,
        shared_history: SharedHistory,
    ) -> Self {
        Self {
            name: name.to_string(),
            files,
            model,
            temperature,
            max_iterations,
            tx,
            original_prompt: None,
            shared_history,
            history: Vec::new(),
        }
    }

    fn load_system_message(&self, user_input: &str) -> Message {
        let mut system_content = String::new();
        let mut vars = HashMap::new();

        if let Some(orig) = &self.original_prompt {
            vars.insert("nminput".to_string(), orig.clone());
        }

        for entry in &self.files {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let parts: Vec<&str> = entry.splitn(3, ':').collect();
            if parts.len() == 3 {
                let role = parts[1].trim();
                let file = parts[2].trim();

                let out = run_poml_file_with_vars(
                    file,
                    &vars,
                    self.original_prompt.as_deref().unwrap_or(user_input),
                    "",
                    &self.tx,
                );

                system_content.push_str(&format!("=== {} ===\n{}\n\n", role, out));
            }
        }

        Message {
            role: "system".to_string(),
            content: Some(system_content),
            tool_calls: None,
        }
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

        if self.original_prompt.is_none() {
            self.original_prompt = Some(input.to_string());
        }

        // ✅ Rehydrate messages from local history
        let mut messages = vec![self.load_system_message(input)];
        for msg in &self.history {
            messages.push(msg.clone());
        }

        // ✅ Add the new user input once
        let user_msg = Message {
            role: "user".into(),
            content: Some(input.to_string()),
            tool_calls: None,
        };
        messages.push(user_msg.clone());
        self.history.push(user_msg.clone());
        self.shared_history.append(user_msg);

        let tools = tool_registry.get_tools();
        let mut iteration = 0;
        let mut final_output = String::new();

        loop {
            iteration += 1;
            if iteration > self.max_iterations {
                return (final_output, None);
            }

            let resp = generate_full_response(
                base_url.clone(),
                api_key.clone(),
                self.model.clone(),
                self.temperature,
                messages.clone(),   // ✅ full conversation
                Some(tools.clone()),
            )
            .await;

            match resp {
                Ok(llm) => {
                    let choice = &llm.choices[0];
                    let msg = &choice.message;

                    if let Some(content) = &msg.content {
                        final_output = content.clone();
                        let assistant_msg = Message {
                            role: "assistant".into(),
                            content: Some(content.clone()),
                            tool_calls: None,
                        };
                        messages.push(assistant_msg.clone());
                        self.history.push(assistant_msg.clone());
                        self.shared_history.append(assistant_msg.clone());

                        // ✅ Update nmoutput in poml with latest content
                        for entry in &self.files {
                            let parts: Vec<&str> = entry.splitn(3, ':').collect();
                            if parts.len() == 3 {
                                let file = parts[2].trim();
                                let mut vars = HashMap::new();
                                if let Some(orig) = &self.original_prompt {
                                    vars.insert("nminput".to_string(), orig.clone());
                                }
                                let _ = inject_let_variables_in_file(
                                    file,
                                    &vars,
                                    self.original_prompt.as_deref().unwrap_or(""),
                                    content,   // ✅ latest output
                                    &self.tx,
                                );
                            }
                        }
                    }

                    if let Some(tool_calls) = &msg.tool_calls {
                        for tc in tool_calls {
                            let result = tool_registry
                                .execute_tool(&tc.function.name, &tc.function.arguments);

                            let content = match result {
                                Ok(v) => serde_json::to_string(&v).unwrap(),
                                Err(e) => format!("Error: {}", e),
                            };

                            let tool_msg = Message {
                                role: "tool".into(),
                                content: Some(content.clone()),
                                tool_calls: None,
                            };
                            messages.push(tool_msg.clone());
                            self.history.push(tool_msg.clone());
                            self.shared_history.append(tool_msg.clone());
                        }
                        continue; // loop again after tool call
                    }
                }
                Err(e) => {
                    let err = format!("Error: {}", e);
                    return (err, None);
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
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json_str) {
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

/// ChainedAgent with history + verbose logging + shared history
pub struct ChainedAgent {
    inner: Box<dyn Agent>,
    next: Option<i32>,
    id: i32,
    tx: UnboundedSender<AppEvent>,
    history: Vec<Message>,
    shared_history: SharedHistory, // ✅ NEW
}

impl ChainedAgent {
    pub fn new(
        inner: Box<dyn Agent>,
        next: Option<i32>,
        id: i32,
        tx: UnboundedSender<AppEvent>,
        shared_history: SharedHistory, // ✅ pass in
    ) -> Self {
        Self {
            inner,
            next,
            id,
            tx,
            history: Vec::new(),
            shared_history,
        }
    }
}

#[async_trait]
impl Agent for ChainedAgent {
    async fn run(
        &mut self,
        input: &str,
        tool_registry: &(dyn ToolRegistryTrait + Send + Sync),
    ) -> (String, Option<i32>) {
        let _ = self.tx.send(AppEvent::Log(format!(
            "[Agent {}] Starting run with input: {}",
            self.id + 1,
            input
        )));

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

        // Build input with full history (including user messages)
        let mut combined_input = String::new();
        for msg in &self.history {
            if let Some(content) = &msg.content {
                // Include all messages in the history (including user messages)
                combined_input.push_str(&format!("{}: {}\n", msg.role, content));
            }
        }

        // Add the current input as a user message
        if !input.starts_with("__ROUTE__") {
            combined_input.push_str(&format!("user: {}\n", input));
        }

        let (output, route_decision) = self.inner.run(&combined_input, tool_registry).await;

        // Save to local + shared history
        let user_msg = Message {
            role: "user".into(),
            content: Some(input.to_string()),
            tool_calls: None,
        };
        let assistant_msg = Message {
            role: "assistant".into(),
            content: Some(output.clone()),
            tool_calls: None,
        };

        self.history.push(user_msg.clone());
        self.history.push(assistant_msg.clone());

        // ✅ Append to shared history
        self.shared_history.append(user_msg.clone());
        self.shared_history.append(assistant_msg.clone());

        let _ = self.tx.send(AppEvent::Log(format!(
            "[Agent {}] Saved to history (local + shared). Local history length now {}",
            self.id + 1,
            self.history.len()
        )));

        // ✅ Log separately
        if output.starts_with("Error:") {
            let _ = self.tx.send(AppEvent::Log(format!(
                "[Agent {}] encountered an error: {}",
                self.id + 1,
                output
            )));
        } else {
            let _ = self.tx.send(AppEvent::Log(format!(
                "[Agent {}] produced output ({} chars)",
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
        if let Some(next) = next_node {
            let _ = self.tx.send(AppEvent::Log(format!(
                "[Agent {}] Routing to node {}",
                self.id + 1,
                if next == -1 {
                    "END".to_string()
                } else {
                    (next + 1).to_string()
                }
            )));
        }

        (output, next_node)
    }

    fn get_name(&self) -> &str {
        self.inner.get_name()
    }
}
