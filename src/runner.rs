use crate::shared_history::SharedHistory;
use crate::tools::builtin_tools_with_history;
use llmgraph::Graph;
use tokio::sync::mpsc::UnboundedSender;
use crate::metrics::metrics_collector::MetricsCollector;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

pub enum AppCommand {
    RunWorkflow {
        workflow_name: String,
        prompt: String,
        cfg: crate::nm_config::WorkflowConfig,
        start_agent: Option<i32>,
    },
}

pub enum AppEvent {
    Log(String),
    RunStart(String),
    RunResult(String),
    RunEnd(String),
    Error(String),
}

pub async fn run_workflow(cmd: AppCommand, log_tx: UnboundedSender<AppEvent>, metrics: Option<Arc<Mutex<MetricsCollector>>>) {
    match cmd {
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

            // ✅ Register tools with shared history + tx + working_dir
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

                let agent: Box<dyn llmgraph::models::graph::Agent> = if row.agent_type == crate::nm_config::AgentType::Validator {
                    Box::new(crate::agents::PomlValidatorAgent::new(
                        crate::agents::PomlAgent::new(
                            &format!("ValidatorAgent{}", i + 1),
                            files.clone(),
                            cfg.model.clone(),
                            cfg.temperature,
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
                    ))
                };

                // ✅ Pass shared_history into ChainedAgent
                let chained = crate::agents::ChainedAgent::new(
                    i,
                    agent,
                    row.max_iterations,
                    row.iteration_delay_ms,
                    next_id,
                    log_tx.clone(),
                    shared_history.clone(),
                );

                graph.add_node(i as i32, Box::new(chained));
            }

            // Execute workflow with metrics tracking
            let _ = log_tx.send(AppEvent::Log("Starting workflow execution...".to_string()));
            let mut output = String::new();
            let mut current_input = prompt.clone();
            let mut current_node = start_agent.unwrap_or(0) as i32;
            let mut traversals = 0;
            let max_traversals = cfg.maximum_traversals;

            // Start metrics tracking for workflow
            let metrics_collector = metrics.unwrap_or_else(|| Arc::new(Mutex::new(MetricsCollector::new())));
            let request_id = metrics_collector.lock().unwrap().start_request("workflow_execution".to_string()).await;
            // Track the request ID for cleanup later

            let msg = format!(
                "Traversal {}: Starting at node {} with input: {}",
                traversals + 1,
                current_node,
                current_input
            );
            let _ = log_tx.send(AppEvent::Log(msg.clone()));
            output.push_str(&format!("\n{}", msg));

            // Track each traversal step with metrics
            let step_start = std::time::Instant::now();
            let mut step_output = graph.run(current_node, &current_input).await;
            let step_duration = step_start.elapsed();

            // Record metrics for this traversal step
            // For now, we'll record this as a successful step
            // In a real implementation, we'd need to determine success/failure
            let _ = metrics_collector.lock().unwrap().finish_request(format!("step_{}", traversals), true).await;

            // Try to detect explicit routing marker
            let mut next_node = if let Some(route_idx) = step_output.rfind("\n__ROUTE__=") {
                let route_str = &step_output[route_idx + 11..];
                let route = route_str.trim().parse::<i32>().ok();
                step_output.truncate(route_idx); // remove marker
                route
            } else {
                None
            };

            // ✅ If no explicit marker, fall back to config.nm routing
            if next_node.is_none() {
                next_node = if !step_output.starts_with("Error:") {
                    // Preserve the original prompt and only pass the LLM output to the next agent
                    current_input = step_output.trim().to_string();
                    Some(current_node + 1)
                } else {
                    Some(current_node) // retry same node on error
                };
            }

            // Final metrics update and alert generation
            let final_metrics = metrics_collector.lock().unwrap().get_metrics().await;
            let alerts = metrics_collector.lock().unwrap().get_alerts().await;

            // Log any alerts
            for alert in alerts {
                let _ = log_tx.send(AppEvent::Log(format!(
                    "[ALERT][{}] {}",
                    alert.level,
                    alert.message
                )));
            }

            // Log final performance summary
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