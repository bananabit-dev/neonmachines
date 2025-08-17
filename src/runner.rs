use crate::agents::{ChainedAgent, PomlAgent, PomlValidatorAgent};
use crate::nm_config::{AgentType, WorkflowConfig};
use crate::shared_history::SharedHistory;
use crate::tools::builtin_tools_with_history;
use llmgraph::Graph;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub enum AppCommand {
    RunWorkflow {
        workflow_name: String,
        prompt: String,
        cfg: WorkflowConfig,
        start_agent: Option<usize>,
    },
    ShowHistory {
        workflow_name: String,
        agent_index: Option<usize>,
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

pub async fn run_workflow(cmd: AppCommand, log_tx: UnboundedSender<AppEvent>) {
    match cmd {
        AppCommand::RunWorkflow {
            workflow_name,
            prompt,
            cfg,
            start_agent,
        } => {
            let _ = log_tx.send(AppEvent::RunStart(workflow_name.clone()));
            let _ = log_tx.send(AppEvent::Log(format!(
                "Starting workflow: {}",
                workflow_name
            )));
            let _ = log_tx.send(AppEvent::Log(format!(
                "Model: {}, Temperature: {}",
                cfg.model, cfg.temperature
            )));
            let _ = log_tx.send(AppEvent::Log(format!(
                "Max traversals: {}",
                cfg.maximum_traversals
            )));

            let mut graph = Graph::new();

            // ✅ Create shared history
            let shared_history = SharedHistory::new();
            let _ = log_tx.send(AppEvent::Log(
                "[SharedHistory] Initialized global shared history".to_string(),
            ));

            // ✅ Register tools with shared history + tx
            for (tool, func) in builtin_tools_with_history(shared_history.clone(), log_tx.clone()) {
                graph.register_tool(tool, func);
            }

            // Build graph nodes
            let _ = log_tx.send(AppEvent::Log(format!(
                "Building graph with {} agents",
                cfg.rows.len()
            )));
            for (i, row) in cfg.rows.iter().enumerate() {
                let next_id = if i + 1 < cfg.rows.len() {
                    Some((i + 1) as i32)
                } else {
                    None
                };

                let _ = log_tx.send(AppEvent::Log(format!(
                    "Creating agent {} ({:?}): files={}, max_iterations={}",
                    i + 1,
                    row.agent_type,
                    row.files,
                    row.max_iterations
                )));

                let agent: Box<dyn llmgraph::Agent> = match row.agent_type {
                    AgentType::Agent | AgentType::ParallelAgent => {
                        let files: Vec<String> =
                            row.files.split(';').map(|s| s.trim().to_string()).collect();
                        Box::new(PomlAgent::new(
                            &format!("Agent{}", i + 1),
                            files,
                            cfg.model.clone(),
                            cfg.temperature,
                            row.max_iterations,
                            log_tx.clone(),
                            shared_history.clone(), // ✅ FIX
                        ))
                    }
                    AgentType::ValidatorAgent => {
                        let files: Vec<String> =
                            row.files.split(';').map(|s| s.trim().to_string()).collect();
                        let poml_validator = PomlAgent::new(
                            &format!("ValidatorAgent{}", i + 1),
                            files,
                            cfg.model.clone(),
                            cfg.temperature,
                            row.max_iterations,
                            log_tx.clone(),
                            shared_history.clone(), // ✅ FIX
                        );
                        let success_route = row.on_success.unwrap_or(next_id.unwrap_or(-1));
                        let failure_route =
                            row.on_failure
                                .unwrap_or(if i > 0 { (i - 1) as i32 } else { -1 });

                        let _ = log_tx.send(AppEvent::Log(format!(
                            "ValidatorAgent{}: success_route={}, failure_route={}",
                            i + 1,
                            if success_route == -1 {
                                "END".to_string()
                            } else {
                                (success_route + 1).to_string()
                            },
                            if failure_route == -1 {
                                "END".to_string()
                            } else {
                                (failure_route + 1).to_string()
                            }
                        )));

                        Box::new(PomlValidatorAgent::new(
                            poml_validator,
                            success_route,
                            failure_route,
                        ))
                    }
                };

                // ✅ Pass shared_history into ChainedAgent
                let chained = ChainedAgent::new(
                    agent,
                    next_id,
                    i as i32,
                    log_tx.clone(),
                    shared_history.clone(),
                );
                graph.add_node(i as i32, Box::new(chained));
            }

            // Execute workflow
            let _ = log_tx.send(AppEvent::Log("Starting workflow execution...".to_string()));
            let mut traversals = 0;
            let mut output = String::new();
            let mut current_input = prompt.clone();
            let mut current_node = start_agent.unwrap_or(0) as i32;

            loop {
                if traversals >= cfg.maximum_traversals {
                    let msg = format!(
                        "[Traversal limit reached: {} traversals]",
                        cfg.maximum_traversals
                    );
                    let _ = log_tx.send(AppEvent::Log(msg.clone()));
                    output.push_str(&format!("\n{}", msg));
                    break;
                }
                traversals += 1;

                let mut step_output = graph.run(current_node, &current_input).await;

                // Try to detect explicit routing marker
                let mut next_node = if let Some(route_idx) = step_output.rfind("\n__ROUTE__=") {
                    let route_str = &step_output[route_idx + 10..];
                    let route = route_str.trim().parse::<i32>().ok();
                    step_output.truncate(route_idx); // remove marker
                    route
                } else {
                    None
                };

                // ✅ If no explicit marker, fall back to config.nm routing
                if next_node.is_none() {
                    let row = &cfg.rows[current_node as usize];
                    if !step_output.starts_with("Error:") {
                        next_node = row.on_success;
                    } else {
                        next_node = row.on_failure;
                    }
                }

                let clean_output = step_output.trim().to_string();
                output.push_str(&clean_output);

                // Preserve the original prompt and only pass the LLM output to the next agent
                current_input = clean_output;

                match next_node {
                    Some(-1) => {
                        let _ = log_tx.send(AppEvent::Log(format!(
                            "Traversal {}: Workflow completed (reached END node)",
                            traversals
                        )));
                        break;
                    }
                    Some(next) if next >= 0 => {
                        let _ = log_tx.send(AppEvent::Log(format!(
                            "Traversal {}: Transitioning from node {} to node {}",
                            traversals,
                            current_node + 1,
                            next + 1
                        )));
                        current_node = next;
                    }
                    _ => {
                        let _ = log_tx.send(AppEvent::Log(format!(
                            "Traversal {}: No valid routing from node {}, ending workflow",
                            traversals,
                            current_node + 1
                        )));
                        break;
                    }
                }
            }

            let _ = log_tx.send(AppEvent::RunResult(format!(
                "Workflow completed after {} traversal(s)\nFinal output:\n{}",
                traversals, output
            )));
            let _ = log_tx.send(AppEvent::RunEnd(workflow_name));
        }

        AppCommand::ShowHistory {
            workflow_name,
            agent_index,
            cfg,
        } => {
            let _ = log_tx.send(AppEvent::Log(format!(
                "Showing history for workflow '{}'",
                workflow_name
            )));

            let mut graph = Graph::new();
            let shared_history = SharedHistory::new();
            for (i, row) in cfg.rows.iter().enumerate() {
                let next_id = if i + 1 < cfg.rows.len() {
                    Some((i + 1) as i32)
                } else {
                    None
                };
                let agent: Box<dyn llmgraph::Agent> = match row.agent_type {
                    AgentType::Agent | AgentType::ParallelAgent => {
                        let files: Vec<String> =
                            row.files.split(';').map(|s| s.trim().to_string()).collect();
                        Box::new(PomlAgent::new(
                            &format!("Agent{}", i + 1),
                            files,
                            cfg.model.clone(),
                            cfg.temperature,
                            row.max_iterations,
                            log_tx.clone(),
                            shared_history.clone(), // ✅ FIX
                        ))
                    }
                    AgentType::ValidatorAgent => {
                        let files: Vec<String> =
                            row.files.split(';').map(|s| s.trim().to_string()).collect();
                        let poml_validator = PomlAgent::new(
                            &format!("ValidatorAgent{}", i + 1),
                            files,
                            cfg.model.clone(),
                            cfg.temperature,
                            row.max_iterations,
                            log_tx.clone(),
                            shared_history.clone(), // ✅ FIX
                        );
                        Box::new(PomlValidatorAgent::new(
                            poml_validator,
                            row.on_success.unwrap_or(-1),
                            row.on_failure.unwrap_or(-1),
                        ))
                    }
                };
                let chained = ChainedAgent::new(
                    agent,
                    next_id,
                    i as i32,
                    log_tx.clone(),
                    shared_history.clone(),
                );
                graph.add_node(i as i32, Box::new(chained));
            }

            if let Some(idx) = agent_index {
                let dump = graph.run(idx as i32, "__SHOW_HISTORY__").await;
                let _ = log_tx.send(AppEvent::Log(dump));
            } else {
                for i in 0..cfg.rows.len() {
                    let dump = graph.run(i as i32, "__SHOW_HISTORY__").await;
                    let _ = log_tx.send(AppEvent::Log(dump));
                }
            }
        }
    }
}
