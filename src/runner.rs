use crate::agents::PomlAgent;
use crate::tools::builtin_tools;
use crate::nm_config::WorkflowConfig;
use llmgraph::Graph;
use llmgraph::agents::StatefulAgent;
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

            // Wrap PomlAgent in StatefulAgent to forward output
            let mut stateful = StatefulAgent::new(format!("StatefulAgent{}", i + 1));
            stateful = stateful.with_processor(Box::new(move |input, _state| {
                // run PomlAgent synchronously? no, we can't here
                // Instead, just forward input (Graph will call PomlAgent::run)
                (input.to_string(), Some(-1))
            }));

            // Instead of using processor, we can just add PomlAgent directly
            // but we want chaining, so we wrap
            graph.add_node(i as i32, Box::new(poml_agent));
            if i > 0 {
                let _ = graph.add_edge((i - 1) as i32, i as i32);
            }
        }

        let output = graph.run(0, &prompt).await;
        let _ = log_tx.send(AppEvent::RunResult(output));
        let _ = log_tx.send(AppEvent::RunEnd(workflow_name));
    }
}