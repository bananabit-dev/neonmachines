use warp::ws::{Message, WebSocket};
use futures_util::stream::StreamExt;
use futures_util::sink::SinkExt;
use tokio::sync::{mpsc, Mutex};
use crate::app::App;
use crate::runner::{AppEvent};
use crate::nm_config::{load_all_nm, preset_workflows};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn handle_websocket_connection(ws: WebSocket) {
    let (mut tx, mut rx) = ws.split();

    // Create App instance
    let loaded_workflows = load_all_nm().unwrap_or_else(|_| preset_workflows());
    let mut workflows = HashMap::new();
    for wf in loaded_workflows {
        workflows.insert(wf.name.clone(), wf.clone());
    }
    let active_name = workflows.keys().next().map(|name| name.clone()).unwrap_or_else(|| "default".to_string());
    let (tx_cmd, mut rx_cmd) = mpsc::unbounded_channel();
    let (tx_evt, mut rx_evt) = mpsc::unbounded_channel();
    let metrics_collector = Arc::new(tokio::sync::Mutex::new(crate::metrics::metrics_collector::MetricsCollector::new()));
    let app = Arc::new(Mutex::new(App::new(tx_cmd, rx_evt, workflows, active_name, Some(metrics_collector.clone()))));

    // Task to handle runner commands
    tokio::spawn(async move {
        while let Some(cmd) = rx_cmd.recv().await {
            crate::runner::run_workflow(cmd, tx_evt.clone(), Some(metrics_collector.clone())).await;
        }
    });

    // Task to send events to the client
    let (ws_tx, mut ws_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(message) = ws_rx.recv().await {
            if tx.send(message).await.is_err() {
                break;
            }
        }
    });

    // Task to handle incoming messages
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(result) = rx.next().await {
            if let Ok(msg) = result {
                if msg.is_text() {
                    let text = msg.to_str().unwrap();
                    let mut app = app_clone.lock().await;
                    app.input = text.to_string();
                    app.submit();
                }
            }
        }
    });

    // Main event loop for this connection
    loop {
        let mut app = app.lock().await;
        tokio::select! {
            Some(event) = app.rx.recv() => {
                let msg = match event {
                    AppEvent::Log(line) => Message::text(format!("[LOG] {}", line)),
                    AppEvent::RunStart(name) => Message::text(format!("[RUN_START] {}", name)),
                    AppEvent::RunResult(line) => Message::text(format!("[RUN_RESULT] {}", line)),
                    AppEvent::RunEnd(name) => Message::text(format!("[RUN_END] {}", name)),
                    AppEvent::Error(line) => Message::text(format!("[ERROR] {}", line)),
                };
                if ws_tx.send(msg).is_err() {
                    break;
                }
            }
        }
    }
}
