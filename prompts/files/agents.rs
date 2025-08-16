use async_trait::async_trait;
use dotenv::dotenv;
use std::env;
use std::process::Command;
use llmgraph::models::graph::Agent;
use llmgraph::models::tools::{Message, ToolRegistryTrait};
use llmgraph::generate::generate::generate_full_response;

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
            } else {
                // fallback: treat as user role
                let out = run_poml_file(entry);
                messages.push(Message {
                    role: "user".into(),
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

        match generate_full_response(
            base_url,
            api_key,
            self.model.clone(),
            self.temperature,
            messages,
            Some(tools),
        )
        .await
        {
            Ok(resp) => {
                if let Some(choice) = resp.choices.first() {
                    if let Some(content) = &choice.message.content {
                        // ✅ Return raw content so Graph can pass it to the next agent
                        return (content.clone(), None);
                    }
                }
                ("".into(), None)
            }
            Err(e) => (format!("Error: {}", e), None),
        }
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}