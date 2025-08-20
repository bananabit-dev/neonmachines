use crate::nm_config::{save_all_nm, WorkflowConfig};
use crate::runner::AppCommand;
use crate::app::{ChatMessage, Mode};
use tokio::sync::mpsc::UnboundedSender;
use std::collections::HashMap;

pub fn handle_command(
    line: &str,
    workflows: &mut HashMap<String, WorkflowConfig>,
    active_workflow: &mut String,
    tx: &UnboundedSender<AppCommand>,
    messages: &mut Vec<ChatMessage>,
    selected_agent: &mut Option<usize>,
    mode: &mut Mode, // Add this parameter
) {
    let mut it = line.split_whitespace();
    let cmd = it.next().unwrap_or("");
    match cmd {
        "/cwd" => {
            if let Some(path) = it.next() {
                if let Some(cfg) = workflows.get_mut(active_workflow) {
                    cfg.working_dir = path.to_string();
                    let all: Vec<WorkflowConfig> = workflows.values().cloned().collect();
                    let _ = save_all_nm(&all);
                    messages.push(ChatMessage {
                        from: "system",
                        text: format!("Working directory set to '{}'", path),
                    });
                } else {
                    messages.push(ChatMessage {
                        from: "system",
                        text: "No active workflow selected.".into(),
                    });
                }
            } else {
                if let Some(cfg) = workflows.get(active_workflow) {
                    messages.push(ChatMessage {
                        from: "system",
                        text: format!("Current working directory: {}", cfg.working_dir),
                    });
                } else {
                    messages.push(ChatMessage {
                        from: "system",
                        text: "No active workflow selected.".into(),
                    });
                }
            }
        }
        "/run" => {
            if let Some(name) = it.next() {
                if name == "all" {
                    for wf in workflows.values().cloned() {
                        let _ = tx.send(AppCommand::RunWorkflow {
                            workflow_name: wf.name.clone(),
                            prompt: "Run all".into(),
                            cfg: wf,
                            start_agent: selected_agent.map(|idx| idx as i32),
                        });
                    }
                    messages.push(ChatMessage {
                        from: "system",
                        text: "Running all workflows".into(),
                    });
                } else if let Some(cfg) = workflows.get(name).cloned() {
                    // Collect the rest of the line as optional prompt
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
                        start_agent: selected_agent.map(|idx| idx as i32),
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
            if let Some(name) = it.next() {
                // ✅ If workflow exists, edit it. Otherwise, create new.
                if workflows.contains_key(name) {
                    *active_workflow = name.to_string();
                    messages.push(ChatMessage {
                        from: "system",
                        text: format!("Editing existing workflow '{}'", name),
                    });
                } else {
                    let mut new_cfg = WorkflowConfig::default();
                    new_cfg.name = name.to_string();
                    workflows.insert(name.to_string(), new_cfg);
                    *active_workflow = name.to_string();
                    messages.push(ChatMessage {
                        from: "system",
                        text: format!("Created new workflow '{}'", name),
                    });
                }
            } else {
                messages.push(ChatMessage {
                    from: "system",
                    text: "Entering create workflow mode".into(),
                });
            }
            *mode = Mode::Create;
        }
        "/workflow" => {
            messages.push(ChatMessage {
                from: "system",
                text: "Entering workflow selection mode".into(),
            });
            *mode = Mode::Workflow;
        }
        "/options" => {
            messages.push(ChatMessage {
                from: "system",
                text: "Entering options mode - type your input to send to poml template".into(),
            });
            *mode = Mode::Options;
        }
        "/chat" => {
            messages.push(ChatMessage {
                from: "system",
                text: "Entering interactive chat mode with current workflow. Type your message directly.".into(),
            });
            *mode = Mode::InteractiveChat;
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
                if let Some(_cfg) = workflows.get(active_workflow) {
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
        "/history" => {
            if let Some(cfg) = workflows.get(active_workflow).cloned() {
                if let Some(arg) = it.next() {
                    if let Ok(agent_idx) = arg.parse::<usize>() {
                        let _ = tx.send(AppCommand::ShowHistory {
                            workflow_name: active_workflow.clone(),
                            agent_index: Some(agent_idx as i32),
                            cfg,
                        });
                        messages.push(ChatMessage {
                            from: "system",
                            text: format!("Requested history for agent {}", agent_idx),
                        });
                    } else {
                        messages.push(ChatMessage {
                            from: "system",
                            text: "Usage: /history <agent_index>|all".into(),
                        });
                    }
                } else {
                    let _ = tx.send(AppCommand::ShowHistory {
                        workflow_name: active_workflow.clone(),
                        agent_index: None,
                        cfg,
                    });
                    messages.push(ChatMessage {
                        from: "system",
                        text: "Requested history for all agents".into(),
                    });
                }
            } else {
                messages.push(ChatMessage {
                    from: "system",
                    text: "No active workflow selected.".into(),
                });
            }
        }
        "/trace" => {
            let mut parts = it;
            if let Some(action) = parts.next() {
                match action.to_lowercase().as_str() {
                    "on" | "enable" => {
                        // Create trace log file to enable tracing
                        let trace_file_path = "neonmachines/.neonmachines_data/trace.log";
                        if let Err(e) = std::fs::File::create(trace_file_path) {
                            messages.push(ChatMessage {
                                from: "system",
                                text: format!("Failed to enable tracing: {}", e),
                            });
                        } else {
                            messages.push(ChatMessage {
                                from: "system",
                                text: "Tracing enabled. AI API calls will be logged to .neonmachines_data/trace.log".to_string(),
                            });
                        }
                    }
                    "off" | "disable" => {
                        // Remove trace log file to disable tracing
                        let trace_file_path = "neonmachines/.neonmachines_data/trace.log";
                        if let Err(e) = std::fs::remove_file(trace_file_path) {
                            messages.push(ChatMessage {
                                from: "system",
                                text: format!("Failed to disable tracing: {}", e),
                            });
                        } else {
                            messages.push(ChatMessage {
                                from: "system",
                                text: "Tracing disabled".to_string(),
                            });
                        }
                    }
                    "status" => {
                        let trace_file_path = "neonmachines/.neonmachines_data/trace.log";
                        let status = if std::path::Path::new(trace_file_path).exists() {
                            "enabled"
                        } else {
                            "disabled"
                        };
                        messages.push(ChatMessage {
                            from: "system",
                            text: format!("Tracing is {}", status),
                        });
                    }
                    "show" => {
                        let trace_file_path = "neonmachines/.neonmachines_data/trace.log";
                        if std::path::Path::new(trace_file_path).exists() {
                            match std::fs::read_to_string(trace_file_path) {
                                Ok(content) => {
                                    messages.push(ChatMessage {
                                        from: "system",
                                        text: format!("Trace log:\n\n{}", content),
                                    });
                                }
                                Err(e) => {
                                    messages.push(ChatMessage {
                                        from: "system",
                                        text: format!("Failed to read trace log: {}", e),
                                    });
                                }
                            }
                        } else {
                            messages.push(ChatMessage {
                                from: "system",
                                text: "Tracing is disabled. No trace log available.".to_string(),
                            });
                        }
                    }
                    _ => {
                        messages.push(ChatMessage {
                            from: "system",
                            text: "Usage: /trace [on|off|status|show]".to_string(),
                        });
                    }
                }
            } else {
                messages.push(ChatMessage {
                    from: "system",
                    text: "Usage: /trace [on|off|status|show]".to_string(),
                });
            }
        }
        "/help" => {
            // Clear messages and show help in full screen
            messages.clear();
            messages.push(ChatMessage {
                from: "system",
                text: help_command_fullscreen(),
            });
        }
        _ => {
            messages.push(ChatMessage {
                from: "system",
                text: "Unknown command. Type /help for available commands.".into(),
            });
        }
    }
}

fn help_command_fullscreen() -> String {
    r#"┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                                                                         │
│  🚀 NEONMACHINES - AI WORKFLOW ORCHESTRATION FRAMEWORK                                                                                │
│                                                                                                                                         │
│  📋 AVAILABLE COMMANDS:                                                                                                                │
│                                                                                                                                         │
│  /cwd [path]          - Show or set working directory                                                                                   │
│  /run [workflow|all] [prompt] - Run a workflow or all workflows                                                                        │
│  /save                - Save all workflows to config.nm                                                                                 │
│  /create [name]       - Create or edit a workflow                                                                                      │
│  /workflow            - Enter workflow selection mode                                                                                   │
│  /options             - Enter options mode for poml template input                                                                       │
│  /chat                - Enter interactive chat mode                                                                                     │
│  /agent [number|none|list] - Select agent for routing                                                                                    │
│  /history [agent|all] - Show execution history                                                                                         │
│  /trace [on|off|show] - Enable/disable/view tracing                                                                                    │
│  /help                - Show this help message (you're here!)                                                                          │
│                                                                                                                                         │
│  🎮 NAVIGATION:                                                                                                                        │
│  Enter - Submit message                                                                                                                 │
│  Shift+Enter - Insert newline                                                                                                           │
│  Ctrl+C or Ctrl+D - Quit                                                                                                                │
│  Ctrl+L - Clear screen                                                                                                                  │
│  Tab - Command completion                                                                                                               │
│                                                                                                                                         │
│  💡 EXAMPLES:                                                                                                                           │
│  /run myworkflow "Process this data"                                                                                                   │
│  /agent 2 - Select agent 2 for routing                                                                                                  │
│  /agent none - Use default routing                                                                                                      │
│  /create newworkflow - Create new workflow named 'newworkflow'                                                                           │
│  /options - Enter options mode for poml template input                                                                                  │
│  /trace on - Enable API call tracing                                                                                                    │
│                                                                                                                                         │
│  🔄 WORKFLOW MODE:                                                                                                                     │
│  - Press LEFT/RIGHT arrows to navigate between workflows                                                                               │
│  - Press Enter to select a workflow                                                                                                    │
│  - Press Esc to exit workflow mode                                                                                                      │
│                                                                                                                                         │
│  🎨 CREATE MODE:                                                                                                                       │
│  - Use arrow keys to navigate fields                                                                                                   │
│  - Press Enter to submit changes                                                                                                        │
│  - Press Esc to exit create mode                                                                                                       │
│                                                                                                                                         │
│  Press any key to continue...                                                                                                           │
│                                                                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
"#.to_string()
}

fn help_command(messages: &mut Vec<ChatMessage>) {
    let help_text = r#"
Available commands:

/cwd [path]          - Show or set working directory
/run [workflow|all] [prompt] - Run a workflow or all workflows
/save                - Save all workflows to config.nm
/create [name]       - Create or edit a workflow
/workflow            - Enter workflow selection mode
/options             - Enter options mode for poml template input
/chat                - Enter interactive chat mode
/agent [number|none|list] - Select agent for routing
/history [agent|all] - Show execution history
/trace [on|off|show] - Enable/disable/view tracing
/help                - Show this help message

Navigation:
Enter - Submit message
Shift+Enter - Insert newline
Ctrl+C or Ctrl+D - Quit
Ctrl+L - Clear screen
Tab - Command completion

Examples:
/run myworkflow "Process this data"
/agent 2 - Select agent 2 for routing
/agent none - Use default routing
/create newworkflow - Create new workflow named 'newworkflow'
/options - Enter options mode for poml template input
/trace on - Enable API call tracing
"#;
    messages.push(ChatMessage {
        from: "system",
        text: help_text.to_string(),
    });
}