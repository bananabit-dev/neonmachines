use crate::nm_config::{save_nm, WorkflowConfig};
use crate::runner::AppCommand;
use crate::app::ChatMessage;
use tokio::sync::mpsc::UnboundedSender;
use std::collections::HashMap;

pub fn handle_command(
    line: &str,
    workflows: &mut HashMap<String, WorkflowConfig>,
    active_workflow: &mut String,
    tx: &UnboundedSender<AppCommand>,
    messages: &mut Vec<ChatMessage>,
) {
    let mut it = line.split_whitespace();
    let cmd = it.next().unwrap_or("");
    match cmd {
        "/run" => {
            if let Some(name) = it.next() {
                if name == "all" {
                    for wf in workflows.values().cloned() {
                        let _ = tx.send(AppCommand::RunWorkflow {
                            workflow_name: wf.name.clone(),
                            prompt: "Run all".into(),
                            cfg: wf,
                        });
                    }
                    messages.push(ChatMessage {
                        from: "system",
                        text: "Running all workflows".into(),
                    });
                } else if let Some(cfg) = workflows.get(name).cloned() {
                    let _ = tx.send(AppCommand::RunWorkflow {
                        workflow_name: cfg.name.clone(),
                        prompt: "Run".into(),
                        cfg,
                    });
                    *active_workflow = name.to_string();
                    messages.push(ChatMessage {
                        from: "system",
                        text: format!("Running workflow '{}'", name),
                    });
                } else {
                    messages.push(ChatMessage {
                        from: "system",
                        text: format!("Workflow '{}' not found", name),
                    });
                }
            } else {
                messages.push(ChatMessage {
                    from: "system",
                    text: "Usage: /run <workflow>|all".into(),
                });
            }
        }
        "/save" => {
            if let Some(cfg) = workflows.get(active_workflow) {
                if let Err(e) = save_nm(cfg) {
                    messages.push(ChatMessage {
                        from: "system",
                        text: format!("Save error: {}", e),
                    });
                } else {
                    messages.push(ChatMessage {
                        from: "system",
                        text: "Saved config.nm".into(),
                    });
                }
            }
        }
        _ => {
            messages.push(ChatMessage {
                from: "system",
                text: "Unknown command".into(),
            });
        }
    }
}