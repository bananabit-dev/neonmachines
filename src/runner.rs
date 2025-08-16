use crate::agents::{PomlAgent, ChainedAgent};
use crate::tools::builtin_tools;
use crate::nm_config::{WorkflowConfig, AgentType};
use llmgraph::agents::ValidatorAgent;
use llmgraph::Graph;
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

pub async fn run_workflow(cmd: AppCommand, log_tx: UnboundedSender<AppEvent>) {
    if let AppCommand::RunWorkflow { workflow_name, prompt, cfg } = cmd {
        let _ = log_tx.send(AppEvent::RunStart(workflow_name.clone()));

        let mut graph = Graph::new();

        for (tool, func) in builtin_tools() {
            graph.register_tool(tool, func);
        }

        for (i, row) in cfg.rows.iter().enumerate() {
            let next_id = if i + 1 < cfg.rows.len() {
                Some((i + 1) as i32)
            } else {
                None
            };

            let agent: Box<dyn llmgraph::Agent> = match row.agent_type {
                AgentType::Agent => {
                    let files: Vec<String> = row.files.split(';').map(|s| s.trim().to_string()).collect();
                    Box::new(PomlAgent::new(
                        &format!("Agent{}", i + 1),
                        files,
                        cfg.model.clone(),
                        cfg.temperature,
                    ))
                }
                AgentType::ParallelAgent => {
                    let files: Vec<String> = row.files.split(';').map(|s| s.trim().to_string()).collect();
                    Box::new(PomlAgent::new(
                        &format!("ParallelAgent{}", i + 1),
                        files,
                        cfg.model.clone(),
                        cfg.temperature,
                    ))
                }
                AgentType::ValidatorAgent => {
                    let validator = ValidatorAgent::new()
                        .add_length_rule(Some(5), Some(500), true)
                        .with_success_route(row.on_success.unwrap_or(next_id.unwrap_or(-1)))
                        .with_failure_route(row.on_failure.unwrap_or((i as i32).saturating_sub(2)));
                    Box::new(validator)
                }
            };

            let chained = ChainedAgent::new(agent, next_id, i as i32, log_tx.clone());
            graph.add_node(i as i32, Box::new(chained));
        }

        // ✅ Traversal limiter
        let mut traversals = 0;
        let mut output = String::new();
        let mut current_input = prompt.clone();
        let mut current_node = 0;

        loop {
            if traversals >= cfg.maximum_traversals {
                output.push_str("\n[Traversal limit reached]");
                break;
            }
            traversals += 1;

            let step = graph.run(current_node, &current_input).await;
            output.push_str(&step);

            // For now, stop after one full run
            break;
        }

        let _ = log_tx.send(AppEvent::RunResult(format!("Final output:\n{}", output)));
        let _ = log_tx.send(AppEvent::RunEnd(workflow_name));
    }
}