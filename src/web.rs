use warp::ws::{Message, WebSocket};
use futures_util::stream::StreamExt;
use futures_util::sink::SinkExt;
use tokio::sync::{mpsc, Mutex};
use crate::app::App;
use crate::runner::{AppEvent};
use crate::nm_config::{load_all_nm, preset_workflows};
use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct UiCommand {
    command: String,
    payload: serde_json::Value,
}

#[derive(Serialize)]
struct UiResponse {
    status: String,
    data: serde_json::Value,
}

pub async fn handle_websocket_connection(ws: WebSocket) {
    let (mut tx, mut rx) = ws.split();

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

    tokio::spawn(async move {
        while let Some(cmd) = rx_cmd.recv().await {
            crate::runner::run_workflow(cmd, tx_evt.clone(), Some(metrics_collector.clone())).await;
        }
    });

    let (ws_tx, mut ws_rx) = mpsc::unbounded_channel();

    // Task to send outgoing messages to WebSocket
    tokio::spawn(async move {
        while let Some(message) = ws_rx.recv().await {
            if tx.send(message).await.is_err() {
                // connection closed
                break;
            }
        }
    });

    let app_clone = app.clone();
    let ws_tx_clone = ws_tx.clone();

    // Task to handle app events and forward to WebSocket
    tokio::spawn(async move {
        let mut app = app_clone.lock().await;
        while let Some(event) = app.rx.recv().await {
            let msg = match event {
                AppEvent::Log(line) => Message::text(serde_json::to_string(&UiResponse { status: "log".to_string(), data: serde_json::Value::String(line) }).unwrap()),
                AppEvent::RunStart(name) => Message::text(serde_json::to_string(&UiResponse { status: "run_start".to_string(), data: serde_json::Value::String(name) }).unwrap()),
                AppEvent::RunResult(line) => Message::text(serde_json::to_string(&UiResponse { status: "run_result".to_string(), data: serde_json::Value::String(line) }).unwrap()),
                AppEvent::RunEnd(name) => Message::text(serde_json::to_string(&UiResponse { status: "run_end".to_string(), data: serde_json::Value::String(name) }).unwrap()),
                AppEvent::Error(line) => Message::text(serde_json::to_string(&UiResponse { status: "error".to_string(), data: serde_json::Value::String(line) }).unwrap()),
            };
            if ws_tx_clone.send(msg).is_err() {
                // connection closed
                break;
            }
        }
    });

    // Main loop to handle incoming WebSocket messages
    while let Some(result) = rx.next().await {
        if let Ok(msg) = result {
            if msg.is_text() {
                if let Ok(text) = msg.to_str() {
                    if let Ok(cmd) = serde_json::from_str::<UiCommand>(text) {
                        let mut app = app.lock().await;
                        match cmd.command.as_str() {
                            "submit" => {
                                if let Some(input) = cmd.payload.as_str() {
                                    app.input = input.to_string();
                                    app.submit();
                                }
                            }
                            "add_node" => {
                                // Logic to add a node based on payload
                                // This requires expanding App's functionality
                            }
                            // Handle other commands like "connect_nodes", "delete_node", etc.
                            _ => {
                                // unhandled command
                            }
                        }
                    }
                }
            }
        }
    }
}
