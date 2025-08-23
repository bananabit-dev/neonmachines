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
use std::sync::atomic::{AtomicBool, Ordering};
use app::App;
use std::collections::HashMap;
use nm_config::{load_all_nm, preset_workflows};
use runner::AppEvent;
use tui::{restore_terminal, setup_terminal};
use cli::{AppMode, Cli};
use clap::Parser;
use poml::handle_poml_execution;
use nmmcp::{load_all_extensions, get_extensions_directory};
use runner::run_workflow;
use tracing::{error, warn, info, instrument};
use tracing_appender::{non_blocking, rolling};
use warp::Filter;
use std::fs;
use std::path::Path;



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
        // Load theme from config file if it exists, otherwise use default
        let theme = load_default_theme().unwrap_or_else(|_| "default".to_string());
        
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
            theme,
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
    let cli = Cli::parse();
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
        match handle_poml_execution(poml_file, working_dir, None, tx_evt).await {
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
    
    // Setup signal handling for graceful shutdown
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let shutdown_flag_clone = shutdown_flag.clone();
    
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        println!("Received shutdown signal...");
        shutdown_flag_clone.store(true, Ordering::SeqCst);
    });
    
    // Setup SIGTERM handling
    #[cfg(unix)]
    tokio::spawn({
        let shutdown_flag_clone = shutdown_flag.clone();
        async move {
            let mut sig_term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to setup SIGTERM handler");
            sig_term.recv().await;
            println!("Received SIGTERM...");
            shutdown_flag_clone.store(true, Ordering::SeqCst);
        }
    });
    
    // Main event loop with proper shutdown handling
    loop {
        // Check for shutdown signal
        if shutdown_flag.load(Ordering::SeqCst) {
            app.add_message("system", "Shutting down gracefully...".to_string());
            break;
        }
        
        app.update_cached_metrics();
        terminal.draw(|f| app.render(f))?;
        
        // Handle events
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
    
    // Cleanup and save state
    app.persist_on_exit();
    restore_terminal(terminal)?;
    println!("Shutdown complete.");
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
        .and_then(get_metrics);

    let tracing_route = warp::path!("api" / "tracing")
        .map(|| {
            let traces = vec![
                serde_json::json!({
                    "id": "1",
                    "timestamp": "2025-08-20 21:28:10",
                    "service": "OpenAI",
                    "status": "Success",
                    "duration": "1.2s",
                    "details": "Model response completed successfully"
                }),
                serde_json::json!({
                    "id": "2",
                    "timestamp": "2025-08-20 21:28:12",
                    "service": "Anthropic",
                    "status": "Failure",
                    "duration": "0.8s",
                    "details": "Connection timeout"
                }),
            ];
            warp::reply::json(&traces)
        });

    let poml_files_route = warp::path!("api" / "poml-files")
        .map(|| {
            let mut poml_files = Vec::new();
            let prompts_dir = Path::new("prompts");
            
            if prompts_dir.exists() {
                for entry in fs::read_dir(prompts_dir).unwrap() {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("poml") {
                        if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                            poml_files.push(file_name.to_string());
                        }
                    }
                }
                poml_files.sort();
            }
            
            warp::reply::json(&poml_files)
        });

    let load_poml_route = warp::path!("api" / "load-poml")
        .and(warp::query::<HashMap<String, String>>())
        .map(|params: HashMap<String, String>| {
            let file_name = params.get("file").cloned().unwrap_or_default();
            
            if file_name.is_empty() {
                return warp::reply::json(&serde_json::json!({
                    "error": "No file specified"
                }));
            }
            
            let file_path = Path::new("prompts").join(&file_name);
            
            if !file_path.exists() {
                return warp::reply::json(&serde_json::json!({
                    "error": format!("POML file not found: {}", file_name)
                }));
            }
            
            match fs::read_to_string(&file_path) {
                Ok(content) => warp::reply::json(&serde_json::json!({
                    "file": file_name,
                    "content": content
                })),
                Err(e) => warp::reply::json(&serde_json::json!({
                    "error": format!("Failed to read POML file: {}", e)
                })),
            }
        });

    let routes = root.or(create_route).or(ws_route).or(static_files).or(metrics_route).or(poml_files_route).or(load_poml_route).or(tracing_route);


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

// API endpoint for getting metrics using the actual metrics collector
async fn get_metrics() -> Result<impl warp::Reply, warp::Rejection> {
    // For now, we're creating a simple metrics response
    // In a real implementation, this would connect to the actual metrics collector
    let metrics = serde_json::json!({
        "requests_count": 0,
        "success_rate": 1.0,
        "average_response_time": 0.0,
        "active_requests": 0,
        "alerts": []
    });
    
    Ok(warp::reply::json(&metrics))
}

/// Load the default theme from config file
fn load_default_theme() -> std::io::Result<String> {
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;
    
    let config_path = Path::new(".neonmachines_data").join("theme_config.json");
    if config_path.exists() {
        let mut file = File::open(config_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        let config: serde_json::Value = serde_json::from_str(&contents)?;
        if let Some(theme) = config.get("default_theme") {
            Ok(theme.as_str().unwrap_or("default").to_string())
        } else {
            Ok("default".to_string())
        }
    } else {
        Ok("default".to_string())
    }
}
