use crate::agents::{FirstAgent, SecondAgent};
use crate::nm_config::{AgentType, WorkflowConfig};
use dotenv::dotenv;
use llmgraph::generate::generate::generate_full_response;
use llmgraph::models::tools::{Function, LLMResponse, Parameters};
use llmgraph::{Graph, Message, Tool};
use serde_json::json;
use std::collections::HashMap;
use std::process::Command;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub enum AppCommand {
    RunWorkflow {
        workflow_name: String,
        prompt: String,
        cfg: WorkflowConfig,
    },
}

#[derive(Debug)]
pub enum AppEvent {
    RunStart(String),
    Log(String),
    RunResult(String),
    RunEnd(String),
}

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

pub async fn run_workflow(cmd: AppCommand, log_tx: UnboundedSender<AppEvent>) {
    match cmd {
        AppCommand::RunWorkflow {
            workflow_name,
            prompt,
            cfg,
        } => {
            let _ = log_tx.send(AppEvent::RunStart(workflow_name.clone()));

            // Build messages from files mapping
            let mut messages: Vec<Message> = Vec::new();

            if let Some(row) = cfg.rows.get(cfg.active_agent_index) {
                if !row.files.trim().is_empty() {
                    for part in row.files.split(';') {
                        let part = part.trim();
                        if part.is_empty() {
                            continue;
                        }
                        if let Some(rest) = part.strip_prefix("role:") {
                            if let Some((role, files)) = rest.split_once(':') {
                                let mut content = String::new();
                                for file in files.split(',') {
                                    let file = file.trim();
                                    if !file.is_empty() {
                                        let out = run_poml_file(file);
                                        content.push_str(&format!(
                                            "\n[{} -> {}]\n{}",
                                            role, file, out
                                        ));
                                    }
                                }
                                messages.push(Message {
                                    role: role.trim().to_string(),
                                    content: Some(content),
                                    tool_calls: None,
                                });
                            }
                        }
                    }
                }
            }

            // Always prepend the user input as the first message
            messages.insert(
                0,
                Message {
                    role: "user".into(),
                    content: Some(prompt),
                    tool_calls: None,
                },
            );

            // Log that we are sending to AI
            let _ = log_tx.send(AppEvent::Log("Sending API request…".into()));
            dotenv().ok();

            // Call the LLM
            let api_key = std::env::var("API_KEY").unwrap_or_default();
            //dbg let _ = log_tx.send(AppEvent::RunResult(format!("Error: {}", api_key)));
            let base_url = "https://openrouter.ai/api/v1/chat/completions".to_string();
            let model = "z-ai/glm-4.5".to_string();
            let temperature = 0.1;

            match generate(
                base_url,
                api_key,
                model,
                temperature,
                messages.clone(),
                None,
            )
            .await
            {
                Ok(resp) => {
                    if let Some(choice) = resp.choices.first() {
                        if let Some(content) = &choice.message.content {
                            let _ = log_tx.send(AppEvent::RunResult(content.clone()));
                        }
                    }
                }
                Err(e) => {
                    let _ = log_tx.send(AppEvent::RunResult(format!("Error: {}", e)));
                }
            }

            let _ = log_tx.send(AppEvent::RunEnd(workflow_name));
        }
    }
}

async fn generate(
    base_url: String,
    api_key: String,
    model: String,
    temperature: f32,
    messages: Vec<Message>,
    tools: Option<Vec<Tool>>,
) -> Result<LLMResponse, reqwest::Error> {
    let response = generate_full_response(
        base_url,
        api_key,
        model,
        temperature,
        messages.clone(),
        tools,
    )
    .await;
    dbg!(&response);
    response
}
