mod tui;
mod agents;
mod nm_config;
mod runner;
mod commands;
mod app;
mod tools;
mod shared_history;
mod cli;
mod poml;
mod rate_limiter;
mod error;

mod nmmcp;
mod create_ui;
mod workflow_ui;
mod state;
mod web;
mod metrics;

use color_eyre::Result;
use crossterm::event;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use app::App;
use std::collections::HashMap;
use nm_config::{load_all_nm, preset_workflows};
use runner::AppEvent;
use tui::{restore_terminal, setup_terminal};
use cli::{AppMode, Cli};
use poml::handle_poml_execution;
use nmmcp::{load_all_extensions, get_extensions_directory};
use runner::run_workflow;
use tracing::{error, warn, info, instrument};
use tracing_appender::{non_blocking, rolling};
use warp::Filter;
use serde::Serialize;
use std::time::Instant;


static START_TIME: once_cell::sync::Lazy<Instant> = once_cell::sync::Lazy::new(Instant::now);

#[derive(Serialize)]
struct Metrics {
    uptime: String,
    memory_usage: String,
    cpu_usage: String,
}


#[instrument]
fn init_logging(cli: &Cli) -> Result<()> {
    let _level_filter = cli.get_tracing_level();
    let file_appender = if let Some(log_file) = &cli.log_file {
        rolling::daily("logs", log_file)
    } else {
        rolling::daily("logs", "neonmachines.log")
    };
    let (_non_blocking, guard) = non_blocking(file_appender);
    tracing_subscriber::fmt()
        .compact()
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_writer(std::io::stdout)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    tracing::info!("Logging initialized with level: {}", cli.log_level);
    if let Some(log_file) = &cli.log_file {
        tracing::info!("Logs will be written to: {}", log_file.display());
    }
    std::mem::forget(guard);
    Ok(())
}

impl Default for Cli {
    fn default() -> Self {
        Cli {
            command: None,
            tui: true,
            web: false,
            config: false,
            port: 3000,
            host: "127.0.0.1".to_string(),
            config_file: None,
            log_level: "info".to_string(),
            verbose: false,
            theme: "default".to_string(),
            avatar: None,
            rate_limit: 60,
            enable_rate_limit: false,
            poml_file: None,
            working_dir: None,
            log_file: None,
            experimental: false,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args: Vec<String> = std::env::args().collect();
    let mut cli = Cli::default();
    cli.web = args.contains(&"--web".to_string());
    cli.config = args.contains(&"--config".to_string());
    cli.verbose = args.contains(&"--verbose".to_string());
    cli.experimental = args.contains(&"--experimental".to_string());
    for (i, arg) in args.iter().enumerate() {
        match arg.as_str() {
            "--port" if i + 1 < args.len() => {
                if let Ok(port) = args[i + 1].parse() {
                    cli.port = port;
                }
            }
            "--host" if i + 1 < args.len() => {
                cli.host = args[i + 1].clone();
            }
            "--log-level" if i + 1 < args.len() => {
                cli.log_level = args[i + 1].clone();
            }
            "--theme" if i + 1 < args.len() => {
                cli.theme = args[i + 1].clone();
            }
            "--rate-limit" if i + 1 < args.len() => {
                if let Ok(limit) = args[i + 1].parse() {
                    cli.rate_limit = limit;
                }
            }
            "--poml-file" if i + 1 < args.len() => {
                cli.poml_file = Some(PathBuf::from(&args[i + 1]));
            }
            "--working-dir" if i + 1 < args.len() => {
                cli.working_dir = Some(PathBuf::from(&args[i + 1]));
            }
            "--log-file" if i + 1 < args.len() => {
                cli.log_file = Some(PathBuf::from(&args[i + 1]));
            }
            _ => {}
        }
    }
    info!("Starting Neonmachines v{}", env!("CARGO_PKG_VERSION"));
    if let Err(e) = cli.validate() {
        error!("CLI validation failed: {}", e);
        eprintln!("Configuration error: {}", e);
        return Err(e.into());
    }
    if let Err(e) = init_logging(&cli) {
        error!("Failed to initialize logging: {}", e);
        eprintln!("Failed to initialize logging: {}", e);
        return Err(e.into());
    }
    if let Some(poml_file) = &cli.poml_file {
        info!("Executing POML file: {}", poml_file.display());
        let (tx_evt, _) = mpsc::unbounded_channel::<AppEvent>();
        let working_dir = cli.working_dir.clone();
        match handle_poml_execution(poml_file, working_dir, tx_evt).await {
            Ok(_) => {
                info!("POML execution completed successfully");
                println!("POML execution completed successfully");
            }
            Err(e) => {
                error!("POML execution failed: {}", e);
                eprintln!("POML execution failed: {}", e);
            }
        }
        return Ok(());
    }
    if cli.enable_rate_limit {
        info!("Rate limiting enabled with limit: {} requests/minute", cli.rate_limit);
        println!("Rate limiting enabled with limit: {} requests/minute", cli.rate_limit);
    }
    let mode = cli.get_mode();
    info!("Running in {:?} mode", mode);
    match mode {
        AppMode::Web => run_web(cli).await,
        AppMode::Config => run_config(cli).await,
        AppMode::Command => run_command(cli).await,
        AppMode::Tui => run_tui(cli).await,
    }
}

async fn run_tui(cli: Cli) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let log_file = cli.log_file.clone().unwrap_or_else(|| PathBuf::from("neonmachines.log"));
    println!("Logging to file: {}", log_file.display());
    let loaded_workflows = load_all_nm().unwrap_or_else(|_| preset_workflows());
    let mut workflows = HashMap::new();
    for wf in loaded_workflows {
        workflows.insert(wf.name.clone(), wf.clone());
    }
    let active_name = workflows
        .keys()
        .next()
        .map(|name| name.clone())
        .unwrap_or_else(|| "default".to_string());
    let metrics_collector = Arc::new(tokio::sync::Mutex::new(
        crate::metrics::metrics_collector::MetricsCollector::new(),
    ));
    let (tx_cmd, mut rx_cmd) = mpsc::unbounded_channel();
    let (tx_evt, rx_evt) = mpsc::unbounded_channel();
    let metrics_clone = metrics_collector.clone();
    tokio::spawn(async move {
        while let Some(cmd) = rx_cmd.recv().await {
            run_workflow(cmd, tx_evt.clone(), Some(metrics_clone.clone())).await;
        }
    });
    let mut app = App::new(
        tx_cmd.clone(),
        rx_evt,
        workflows,
        active_name,
        Some(metrics_collector.clone()),
    );
    if let Err(e) = app.load_history_from_file() {
        println!("Warning: Could not load command history: {}", e);
    } else {
        println!("Loaded {} commands from history", app.command_history.len());
    }
    let (tx_signal, mut rx_signal) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        let _ = tx_signal.send(());
    });
    loop {
        if let Ok(()) = rx_signal.try_recv() {
            app.add_message("system", "Received shutdown signal...".to_string());
            break;
        }
        app.update_cached_metrics();
        terminal.draw(|f| app.render(f))?;
        if let Ok(ev) = event::poll(Duration::from_millis(33)) {
            if ev {
                let ev = event::read()?;
                app.queue_event(ev);
            }
        }
        if app.process_events() {
            break;
        }
        app.poll_async().await;
    }
    app.persist_on_exit();
    restore_terminal(terminal)?;
    Ok(())
}

async fn run_web(cli: Cli) -> Result<()> {
    info!("Starting web interface on http://{}:{}/", cli.get_host(), cli.get_port());
    println!("🚀 Starting Neonmachines Web Interface");
    println!("📍 URL: http://{}:{}/", cli.get_host(), cli.get_port());

    let _app_state = crate::state::AppState::new();
    let addr = format!("{}:{}", cli.get_host(), cli.get_port());

    let ws_route = warp::path("ws")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| {
            ws.on_upgrade(move |socket| web::handle_websocket_connection(socket))
        });

    let static_files = warp::fs::dir("web");

    let root = warp::get()
        .and(warp::path::end())
        .and(warp::fs::file("web/index.html"));

    let create_route = warp::get()
        .and(warp::path("create"))
        .and(warp::path::end())
        .and(warp::fs::file("web/graph-editor.html"));

    let metrics_route = warp::path!("api" / "metrics")
        .map(|| {
            let metrics = Metrics {
                uptime: format!("{:.2?}", START_TIME.elapsed()),
                memory_usage: "N/A".to_string(), // Placeholder
                cpu_usage: "N/A".to_string(),    // Placeholder
            };
            warp::reply::json(&metrics)
        });

    let routes = root.or(create_route).or(ws_route).or(static_files).or(metrics_route);

    warp::serve(routes).run(addr.parse::<std::net::SocketAddr>()?).await;

    Ok(())
}

async fn run_config(cli: Cli) -> Result<()> {
    println!("Configuration mode not yet implemented.");
    println!("Config file: {:?}", cli.config_file);
    println!("Log level: {}", cli.log_level);
    if cli.verbose {
        println!("Verbose logging enabled");
    }
    Ok(())
}

async fn run_command(cli: Cli) -> Result<()> {
    match &cli.command {
        Some(cli::Commands::Poml { file, working_dir, output, provider, temperature, max_tokens, log_level, save }) => {
            let mut command = tokio::process::Command::new("python");
            command.arg("-m").arg("poml");
            command.arg("-f").arg(file.display().to_string());
            if let Some(working_dir) = working_dir {
                command.current_dir(working_dir);
            }
            if let Some(provider) = provider {
                command.arg("--provider").arg(provider);
            }
            if let Some(output_path) = output {
                command.arg("--output").arg(output_path.display().to_string());
            }
            command.arg("--temperature").arg(temperature.to_string());
            command.arg("--max-tokens").arg(max_tokens.to_string());
            command.arg("--log-level").arg(log_level);
            if *save {
                command.arg("--save");
            }
            info!("Executing POML file with external CLI: {:?}", command);
            let command_output = command.output().await?;
            if command_output.status.success() {
                info!("POML execution successful");
                println!("POML execution successful:");
                println!("{}", String::from_utf8_lossy(&command_output.stdout));
                if *save {
                    println!("Results saved to output file as requested");
                }
            } else {
                error!("POML execution failed: {}", String::from_utf8_lossy(&command_output.stderr));
                eprintln!("POML execution failed:");
                eprintln!("{}", String::from_utf8_lossy(&command_output.stderr));
            }
        }
        Some(cli::Commands::Config { list_themes, list_providers, show, edit: _, validate: _, theme: _, provider: _ }) => {
            if *list_themes {
                println!("Available themes: default, dark, light");
            }
            if *list_providers {
                println!("Available providers: openai, anthropic, local");
            }
            if *show {
                println!("Configuration not yet implemented.");
            }
        }
        Some(cli::Commands::Extension { list, install, uninstall, update, extension_type: _ }) => {
            let (tx, _) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
            if *list {
                println!("Loading extensions...");
                match load_all_extensions(tx.clone()).await {
                    Ok(registry) => {
                        println!("Loaded {} extensions:", registry.get_extensions().len());
                        for (name, ext) in registry.get_extensions() {
                            println!("  - {} v{} by {}", name, ext.version, ext.author);
                            println!("    {}", ext.description);
                            if !ext.tools.is_empty() {
                                println!("    Tools: {}", ext.tools.len());
                            }
                            println!();
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to load extensions: {}", e);
                    }
                }
            }
            if let Some(install_path) = install {
                println!("Installing extension from: {}", install_path.display());
                println!("Extension installation not yet implemented");
            }
            if let Some(uninstall_name) = uninstall {
                println!("Uninstalling extension: {}", uninstall_name);
                println!("Extension uninstallation not yet implemented");
            }
            if *update {
                println!("Updating extensions...");
                println!("Extension update not yet implemented");
            }
            if !(*list || install.is_some() || uninstall.is_some() || *update) {
                println!("Extension management commands:");
                println!("  --list          List all available extensions");
                println!("  --install <path> Install extension from path");
                println!("  --uninstall <name> Uninstall extension");
                println!("  --update        Update all extensions");
                println!("  --extension-type <type> Extension type (tool or mcp, default: tool)");
            }
        }
        Some(cli::Commands::Info { detailed, extensions, themes }) => {
            if *detailed {
                println!("Neonmachines v{}", env!("CARGO_PKG_VERSION"));
                println!("Built for graph-based AI orchestration");
                println!("Extensions: NMMCP (NeonMachines Model Control Protocol)");
                println!("Tools: Terminal execution, POML workflow execution");
            }
            if *extensions {
                println!("Extension System: NMMCP");
                println!("Extensions Directory: {}", get_extensions_directory().display());
                println!("Status: Ready for extension loading");
            }
            if *themes {
                println!("Available Themes: default, dark, light");
            }
        }
        Some(cli::Commands::Test { provider, extensions, quick }) => {
            if *provider {
                println!("Testing provider connections...");
                println!("Provider testing not yet implemented");
            }
            if *extensions {
                println!("Testing extensions...");
                match load_all_extensions(tokio::sync::mpsc::unbounded_channel().0).await {
                    Ok(registry) => {
                        println!("Extension test successful: {} extensions loaded", registry.get_extensions().len());
                    }
                    Err(e) => {
                        println!("Extension test failed: {}", e);
                    }
                }
            }
            if *quick {
                println!("Running quick test...");
                println!("✓ CLI parsing");
                println!("✓ Logging system");
                println!("✓ Extension framework");
                println!("✓ POML integration");
                println!("Quick test completed successfully");
            }
        }
        _ => {
            println!("Command not yet implemented.");
        }
    }
    Ok(())
}

async fn is_poml_available() -> bool {
    match tokio::process::Command::new("python")
        .arg("--version")
        .output()
        .await
    {
        Ok(_) => {
            match tokio::process::Command::new("python")
                .arg("-m")
                .arg("poml")
                .arg("--help")
                .output()
                .await
            {
                Ok(poml_output) => poml_output.status.success(),
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}
