use crate::shared_history::SharedHistory;
use crate::tools::builtin_tools_with_history;
use llmgraph::Graph;
use tokio::sync::mpsc::UnboundedSender;
use crate::metrics::metrics_collector::MetricsCollector;
use std::sync::{Arc, Mutex};

pub enum AppCommand {
    RunWorkflow {
        workflow_name: String,
        prompt: String,
        cfg: crate::nm_config::WorkflowConfig,
        start_agent: Option<i32>,
    },
    ShowHistory {
        agent_index: Option<i32>,
        workflow_name: String,
        cfg: crate::nm_config::WorkflowConfig,
    },
}

pub enum AppEvent {
    Log(String),
    RunStart(String),
    RunResult(String),
    RunEnd(String),
    Error(String),
}

pub async fn run_workflow(
    cmd: AppCommand,
    log_tx: UnboundedSender<AppEvent>,
    metrics: Option<Arc<Mutex<MetricsCollector>>>,
) {
    match cmd {
        AppCommand::ShowHistory { agent_index, workflow_name, cfg: _ } => {
            let _ = log_tx.send(AppEvent::Log(format!(
                "Showing history for workflow '{}', agent {:?}",
                workflow_name, agent_index
            )));
            let _ = log_tx.send(AppEvent::RunResult("History display not yet implemented".to_string()));
        }

        AppCommand::RunWorkflow { workflow_name, prompt, cfg, start_agent } => {
            let _ = log_tx.send(AppEvent::RunStart(workflow_name.clone()));
            let _ = log_tx.send(AppEvent::Log(format!(
                "Starting workflow '{}' with prompt: {}", 
                workflow_name, 
                prompt
            )));

            // ✅ Create shared history
            let shared_history = SharedHistory::new();
            let _ = log_tx.send(AppEvent::Log(
                "[SharedHistory] Initialized global shared history".to_string(),
            ));

            // ✅ Register tools
            let mut graph = Graph::new();
            for (tool, func) in builtin_tools_with_history(
                shared_history.clone(),
                log_tx.clone(),
                cfg.working_dir.clone(),
            ) {
                graph.register_tool(tool, func);
            }

            // Build graph nodes
            for (i, row) in cfg.rows.iter().enumerate() {
                let next_id = if i + 1 < cfg.rows.len() {
                    Some((i + 1) as i32)
                } else {
                    None
                };

                let files: Vec<String> = row
                    .files
                    .split(';')
                    .map(|s| s.trim().to_string())
                    .collect();

                let agent: Box<dyn llmgraph::models::graph::Agent + Send + Sync> =
                    if row.agent_type == crate::nm_config::AgentType::Validator {
                        Box::new(crate::agents::PomlValidatorAgent::new(
                            crate::agents::PomlAgent::new(
                                &format!("ValidatorAgent{}", i + 1),
                                files.clone(),
                                cfg.model.clone(),
                                cfg.temperature,
                                row.max_iterations,
                                log_tx.clone(),
                                shared_history.clone(),
                            ),
                            row.on_success.unwrap_or(-1),
                            row.on_failure.unwrap_or(-1),
                        ))
                    } else {
                        Box::new(crate::agents::PomlAgent::new(
                            &format!("Agent{}", i + 1),
                            files.clone(),
                            cfg.model.clone(),
                            cfg.temperature,
                            row.max_iterations,
                            log_tx.clone(),
                            shared_history.clone(),
                        ))
                    };

                let chained = crate::agents::ChainedAgent::new(
                    i as i32,
                    agent,
                    log_tx.clone(),
                    next_id,
                    row.max_iterations,
                    row.iteration_delay_ms,
                    shared_history.clone(),
                );
                graph.add_node(i as i32, Box::new(chained));
            }

            // ✅ Traversal loop
            let mut current_node = start_agent.unwrap_or(0) as i32;
            let mut current_input = prompt.clone();
            let mut traversals = 0;
            let max_traversals = cfg.maximum_traversals;

            let metrics_collector = metrics.unwrap_or_else(|| Arc::new(Mutex::new(MetricsCollector::new())));
            let _request_id = metrics_collector
                .lock().unwrap()
                .start_request("workflow_execution".to_string()).await;

            while traversals < max_traversals {
                traversals += 1;

                let msg = format!(
                    "Traversal {}: Starting at node {} with input: {}",
                    traversals, current_node, current_input
                );
                let _ = log_tx.send(AppEvent::Log(msg.clone()));

                let step_start = std::time::Instant::now();
                let step_output = graph.run(current_node, &current_input).await;
                let _step_duration = step_start.elapsed();

                let _ = metrics_collector
                    .lock().unwrap()
                    .finish_request(format!("step_{}", traversals), true).await;

                // Log step result
                let _ = log_tx.send(AppEvent::RunResult(format!(
                    "Traversal {} (node {}):\n{}",
                    traversals, current_node, step_output
                )));

                // Detect explicit routing marker
                if let Some(route_idx) = step_output.rfind("\n__ROUTE__=") {
                    let route_str = &step_output[route_idx + 11..];
                    if let Ok(next) = route_str.trim().parse::<i32>() {
                        current_node = next;
                        current_input = step_output[..route_idx].trim().to_string();
                        continue;
                    }
                }

                // Default routing: go to next node if it exists
                if (current_node as usize) + 1 < cfg.rows.len() {
                    current_node += 1;
                    current_input = step_output.clone();
                    continue;
                }

                // No next node → stop
                break;
            }

            // ✅ Final metrics + alerts
            let final_metrics = metrics_collector.lock().unwrap().get_metrics().await;
            let alerts = metrics_collector.lock().unwrap().get_alerts().await;

            for alert in alerts {
                let _ = log_tx.send(AppEvent::Log(format!(
                    "[ALERT][{}] {}",
                    alert.level,
                    alert.message
                )));
            }

            let _ = log_tx.send(AppEvent::RunResult(format!(
                "Workflow completed. Metrics: {} requests, {:.2}% success rate, avg {:.2}ms response time",
                final_metrics.request_count,
                final_metrics.get_success_rate() * 100.0,
                final_metrics.average_response_time.num_milliseconds()
            )));

            let _ = log_tx.send(AppEvent::RunEnd(workflow_name));
        }
    }
}