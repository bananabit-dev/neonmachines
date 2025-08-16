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
use crossterm::{event, execute, terminal};
use std::time::Duration;
use tokio::sync::mpsc;

use app::App;
use nm_config::{load_nm_or_create, preset_workflows};
use runner::{run_workflow, AppCommand, AppEvent};
use tui::{restore_terminal, setup_terminal};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let mut terminal = setup_terminal()?;

    let (tx_cmd, mut rx_cmd) = mpsc::unbounded_channel::<AppCommand>();
    let (tx_evt, rx_evt) = mpsc::unbounded_channel::<AppEvent>();

    let loaded = load_nm_or_create();
    let presets = preset_workflows();
    let mut workflows = std::collections::HashMap::new();
    for wf in &presets {
        workflows.insert(wf.name.clone(), wf.clone());
    }
    workflows.insert(loaded.name.clone(), loaded.clone());

    let mut app = App::new(tx_cmd.clone(), rx_evt, workflows, loaded.name.clone());

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