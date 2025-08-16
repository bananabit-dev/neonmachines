use crate::nm_config::{save_all_nm, WorkflowConfig};
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
    selected_agent: &mut Option<usize>, // Pass selected_agent directly
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
                            start_agent: *selected_agent,
                        });
                    }
                    messages.push(ChatMessage {
                        from: "system",
                        text: "Running all workflows".into(),
                    });
                } else if let Some(cfg) = workflows.get(name).cloned() {
                    // ✅ Collect the rest of the line as optional prompt
                    let custom_prompt: String = it.collect::<Vec<&str>>().join(" ");
                    let prompt = if custom_prompt.is_empty() {
                        "Run".to_string()
                    } else {
                        custom_prompt
                    };

                    let _ = tx.send(AppCommand::RunWorkflow {
                        workflow_name: cfg.name.clone(),
                        prompt: prompt.clone(),
                        cfg,
                        start_agent: *selected_agent,
                    });
                    *active_workflow = name.to_string();
                    messages.push(ChatMessage {
                        from: "system",
                        text: format!("Running workflow '{}' with prompt: {}", name, prompt),
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
                    text: "Usage: /run <workflow>|all [optional prompt]".into(),
                });
            }
        }
        "/save" => {
            // ✅ Save all workflows instead of just one
            let all: Vec<WorkflowConfig> = workflows.values().cloned().collect();
            if let Err(e) = save_all_nm(&all) {
                messages.push(ChatMessage {
                    from: "system",
                    text: format!("Save error: {}", e),
                });
            } else {
                messages.push(ChatMessage {
                    from: "system",
                    text: "Saved all workflows to config.nm".into(),
                });
            }
        }
        "/create" => {
            // handled in app.rs (switches mode)
            messages.push(ChatMessage {
                from: "system",
                text: "Entering create workflow mode".into(),
            });
        }
        "/workflow" => {
            messages.push(ChatMessage {
                from: "system",
                text: "Entering workflow selection mode".into(),
            });
        }
        "/chat" => {
            messages.push(ChatMessage {
                from: "system",
                text: "Entering interactive chat mode with current workflow. Type your message directly.".into(),
            });
        }
        "/agent" => {
            let mut parts = it;
            if let Some(agent_num) = parts.next() {
                if agent_num == "list" {
                    if let Some(cfg) = workflows.get(active_workflow) {
                        let agent_list: Vec<String> = cfg
                            .rows
                            .iter()
                            .enumerate()
                            .map(|(i, row)| {
                                format!("{}. {:?} - {}", i, row.agent_type, row.files)
                            })
                            .collect();
                        messages.push(ChatMessage {
                            from: "system",
                            text: format!(
                                "Available agents in workflow '{}':\n{}",
                                active_workflow,
                                agent_list.join("\n")
                            ),
                        });
                    } else {
                        messages.push(ChatMessage {
                            from: "system",
                            text: "No active workflow selected.".into(),
                        });
                    }
                } else if agent_num == "none" {
                    *selected_agent = None;
                    messages.push(ChatMessage {
                        from: "system",
                        text: "Cleared agent selection. Will use default workflow routing.".into(),
                    });
                } else if let Ok(agent_idx) = agent_num.parse::<usize>() {
                    if let Some(cfg) = workflows.get(active_workflow) {
                        if agent_idx < cfg.rows.len() {
                            *selected_agent = Some(agent_idx);
                            messages.push(ChatMessage {
                                from: "system",
                                text: format!(
                                    "Selected agent {} for chat. Messages will be routed to this agent.",
                                    agent_idx
                                ),
                            });
                        } else {
                            messages.push(ChatMessage {
                                from: "system",
                                text: format!(
                                    "Agent {} not found. Workflow has {} agents (0-indexed).",
                                    agent_idx,
                                    cfg.rows.len()
                                ),
                            });
                        }
                    } else {
                        messages.push(ChatMessage {
                            from: "system",
                            text: "No active workflow selected.".into(),
                        });
                    }
                } else {
                    messages.push(ChatMessage {
                        from: "system",
                        text: "Invalid agent number. Use /agent <number> or /agent none.".into(),
                    });
                }
            } else {
                if let Some(cfg) = workflows.get(active_workflow) {
                    let current = if let Some(idx) = *selected_agent {
                        format!("Currently selected: Agent {}", idx)
                    } else {
                        "Currently selected: Default routing".to_string()
                    };
                    messages.push(ChatMessage {
                        from: "system",
                        text: format!("Usage: /agent <number|none|list>\n{}", current),
                    });
                } else {
                    messages.push(ChatMessage {
                        from: "system",
                        text: "Usage: /agent <number|none|list>".into(),
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