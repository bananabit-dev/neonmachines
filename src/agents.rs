use async_trait::async_trait;
use dotenv::dotenv;
use std::env;
use std::process::Command;
use llmgraph::models::graph::Agent;
use llmgraph::models::tools::{Message, ToolRegistryTrait};
use llmgraph::generate::generate::generate_full_response;
use crate::runner::AppEvent;
use tokio::sync::mpsc::UnboundedSender;

/// Run a `.poml` file using Python
fn run_poml_file(file: &str) -> String {
    let path = format!("./prompts/{}", file);
    match Command::new("python")
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
    }
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

    fn load_messages(&self) -> Vec<Message> {
        let mut messages = Vec::new();

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

                let out = run_poml_file(file);

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

        let mut messages = self.load_messages();

        // Always append the incoming input (from user or previous agent)
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

/// Wraps an agent and forces it to continue to a specific next node
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

#[async_trait]
impl Agent for ChainedAgent {
    async fn run(
        &mut self,
        input: &str,
        tool_registry: &(dyn ToolRegistryTrait + Send + Sync),
    ) -> (String, Option<i32>) {
        let (output, _) = self.inner.run(input, tool_registry).await;

        // Log intermediate output
        let _ = self.tx.send(AppEvent::RunResult(format!(
            "Agent {} output:\n{}",
            self.id + 1,
            output
        )));

        // Force next agent
        (output, self.next)
    }

    fn get_name(&self) -> &str {
        self.inner.get_name()
    }
}