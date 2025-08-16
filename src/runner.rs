use crate::agents::{PomlAgent, ChainedAgent, PomlValidatorAgent};
use crate::tools::builtin_tools;
use crate::nm_config::{WorkflowConfig, AgentType};
use llmgraph::Graph;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub enum AppCommand {
    RunWorkflow {
        workflow_name: String,
        prompt: String,
        cfg: WorkflowConfig,
        start_agent: Option<usize>, // Optional starting agent index
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
    if let AppCommand::RunWorkflow { workflow_name, prompt, cfg, start_agent } = cmd {
        let _ = log_tx.send(AppEvent::RunStart(workflow_name.clone()));
        let _ = log_tx.send(AppEvent::Log(format!("Starting workflow: {}", workflow_name)));
        let _ = log_tx.send(AppEvent::Log(format!("Model: {}, Temperature: {}", cfg.model, cfg.temperature)));
        let _ = log_tx.send(AppEvent::Log(format!("Max traversals: {}", cfg.maximum_traversals)));

        let mut graph = Graph::new();

        // Register tools
        let _ = log_tx.send(AppEvent::Log("Registering tools...".to_string()));
        for (tool, func) in builtin_tools() {
            graph.register_tool(tool, func);
        }

        // Build graph nodes
        let _ = log_tx.send(AppEvent::Log(format!("Building graph with {} agents", cfg.rows.len())));
        for (i, row) in cfg.rows.iter().enumerate() {
            let next_id = if i + 1 < cfg.rows.len() {
                Some((i + 1) as i32)
            } else {
                None
            };

            let _ = log_tx.send(AppEvent::Log(format!(
                "Creating agent {} ({:?}): files={}",
                i + 1,
                row.agent_type,
                row.files
            )));

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
                    // Parse the POML file to create a custom validator
                    let files: Vec<String> = row.files.split(';').map(|s| s.trim().to_string()).collect();
                    
                    // Create a PomlAgent to execute the validation logic
                    let poml_validator = PomlAgent::new(
                        &format!("ValidatorAgent{}", i + 1),
                        files,
                        cfg.model.clone(),
                        cfg.temperature,
                    );
                    
                    // Get routing configuration
                    let success_route = row.on_success.unwrap_or_else(|| {
                        // If no success route specified, go to next node or end
                        next_id.unwrap_or(-1)
                    });
                    
                    let failure_route = row.on_failure.unwrap_or_else(|| {
                        // If no failure route specified, go back to previous node
                        if i > 0 { (i - 1) as i32 } else { -1 }
                    });

                    let _ = log_tx.send(AppEvent::Log(format!(
                        "ValidatorAgent{}: success_route={}, failure_route={}",
                        i + 1,
                        if success_route == -1 { "END".to_string() } else { (success_route + 1).to_string() },
                        if failure_route == -1 { "END".to_string() } else { (failure_route + 1).to_string() }
                    )));

                    // Create a custom validator that uses the POML agent for validation logic
                    Box::new(PomlValidatorAgent::new(
                        poml_validator,
                        success_route,
                        failure_route,
                    ))
                }
            };

            let chained = ChainedAgent::new(agent, next_id, i as i32, log_tx.clone());
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
                let msg = format!("[Traversal limit reached: {} traversals]", cfg.maximum_traversals);
                let _ = log_tx.send(AppEvent::Log(msg.clone()));
                output.push_str(&format!("\n{}", msg));
                break;
            }
            traversals += 1;

            let _ = log_tx.send(AppEvent::Log(format!(
                "Traversal {}: Running node {} with input length {}",
                traversals, current_node + 1, current_input.len()
            )));

            // Run the current node - the Graph.run method returns a String
            let mut step_output = graph.run(current_node, &current_input).await;
            
            // Extract routing decision from the special marker
            let next_node = if let Some(route_idx) = step_output.rfind("\n__ROUTE__:") {
                let route_str = &step_output[route_idx + 11..];
                let route = route_str.trim().parse::<i32>().ok();
                // Remove the routing marker from the output
                step_output.truncate(route_idx);
                route
            } else {
                None
            };
            
            // Clean up the output and use it as input for next node
            let clean_output = step_output.clone();
            output.push_str(&clean_output);
            current_input = clean_output;

            // Handle routing decision
            match next_node {
                Some(-1) => {
                    // -1 means end of workflow
                    let _ = log_tx.send(AppEvent::Log(format!(
                        "Traversal {}: Workflow completed (reached END node)",
                        traversals
                    )));
                    break;
                }
                Some(next) if next >= 0 => {
                    // Valid next node
                    let _ = log_tx.send(AppEvent::Log(format!(
                        "Traversal {}: Transitioning from node {} to node {}",
                        traversals, current_node + 1, next + 1
                    )));
                    current_node = next;
                    // Continue to next iteration
                }
                _ => {
                    // No routing decision or invalid node, end workflow
                    let _ = log_tx.send(AppEvent::Log(format!(
                        "Traversal {}: No valid routing from node {}, ending workflow",
                        traversals, current_node + 1
                    )));
                    break;
                }
            }
        }

        // Send final result
        let _ = log_tx.send(AppEvent::RunResult(format!(
            "Workflow completed after {} traversal(s)\nFinal output:\n{}",
            traversals,
            output
        )));
        let _ = log_tx.send(AppEvent::RunEnd(workflow_name));
    }
}