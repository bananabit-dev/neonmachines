use crate::agents::{PomlAgent, ChainedAgent};
use crate::tools::builtin_tools;
use crate::nm_config::WorkflowConfig;
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

        // Register builtin tools
        for (tool, func) in builtin_tools() {
            graph.register_tool(tool, func);
        }

        // Build agents from workflow config
        for (i, row) in cfg.rows.iter().enumerate() {
            let files: Vec<String> = row.files.split(';').map(|s| s.trim().to_string()).collect();
            let poml_agent = PomlAgent::new(
                &format!("Agent{}", i + 1),
                files,
                cfg.model.clone(),
                cfg.temperature,
            );

            // Determine next agent id
            let next_id = if i + 1 < cfg.rows.len() {
                Some((i + 1) as i32)
            } else {
                None
            };

            // Wrap PomlAgent in ChainedAgent
            let chained = ChainedAgent::new(Box::new(poml_agent), next_id, i as i32, log_tx.clone());

            graph.add_node(i as i32, Box::new(chained));
        }

        let output = graph.run(0, &prompt).await;

        // Final result
        let _ = log_tx.send(AppEvent::RunResult(format!("Final output:\n{}", output)));

        // ✅ Add finished progress
        let _ = log_tx.send(AppEvent::Log("finished".into()));

        let _ = log_tx.send(AppEvent::RunEnd(workflow_name));
    }
}