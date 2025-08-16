mod tui;
mod agents;
mod nm_config;
mod runner;
mod commands;
mod create_ui;
mod workflow_ui;
mod app;
mod tools;

use color_eyre::Result;
use crossterm::{event, terminal};
use std::time::Duration;
use tokio::sync::mpsc;

use app::App;
use nm_config::{load_all_nm, preset_workflows};
use runner::{run_workflow, AppCommand, AppEvent};
use tui::{restore_terminal, setup_terminal};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let mut terminal = setup_terminal()?;

    let (tx_cmd, mut rx_cmd) = mpsc::unbounded_channel::<AppCommand>();
    let (tx_evt, rx_evt) = mpsc::unbounded_channel::<AppEvent>();

    // ✅ Load all workflows instead of just one
    let loaded_workflows = load_all_nm().unwrap_or_else(|_| preset_workflows());
    let mut workflows = std::collections::HashMap::new();
    for wf in &loaded_workflows {
        workflows.insert(wf.name.clone(), wf.clone());
    }

    // Pick the first workflow as active
    let active_name = loaded_workflows
        .get(0)
        .map(|wf| wf.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let mut app = App::new(tx_cmd.clone(), rx_evt, workflows, active_name);

    tokio::spawn(async move {
        while let Some(cmd) = rx_cmd.recv().await {
            run_workflow(cmd, tx_evt.clone()).await;
        }
    });

    loop {
        terminal.draw(|f| app.render(f))?;

        app.tick_spinner();

        let timeout = Duration::from_millis(80);
        if crossterm::event::poll(timeout)? {
            let ev = event::read()?;
            let quit = app.on_event(ev);
            if quit {
                break;
            }
        }

        app.poll_async().await;
    }

    app.persist_on_exit();

    restore_terminal(terminal)?;
    Ok(())
}