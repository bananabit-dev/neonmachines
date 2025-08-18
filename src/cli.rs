use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "neonmachines",
    about = "A graph-based AI Orchestration framework with professional UI",
    version = "0.2.0",
    author = "Neonmachines Team"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Run in TUI mode (default)
    #[arg(long, conflicts_with_all = ["web", "config"])]
    pub tui: bool,

    /// Run in web mode
    #[arg(long, conflicts_with_all = ["tui", "config"])]
    pub web: bool,

    /// Run in configuration mode
    #[arg(long, conflicts_with_all = ["tui", "web"])]
    pub config: bool,

    /// Port for web server
    #[arg(long, default_value = "3000")]
    pub port: u16,

    /// Host for web server
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Configuration file path
    #[arg(long, short = 'c')]
    pub config_file: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Enable verbose logging
    #[arg(long)]
    pub verbose: bool,

    /// Theme name
    #[arg(long, default_value = "default")]
    pub theme: String,

    /// Custom avatar for web UI
    #[arg(long)]
    pub avatar: Option<PathBuf>,

    /// Rate limit requests per minute
    #[arg(long, default_value = "60")]
    pub rate_limit: u32,

    /// Enable rate limiting
    #[arg(long)]
    pub enable_rate_limit: bool,

    /// POML file to execute
    #[arg(long)]
    pub poml_file: Option<PathBuf>,

    /// Working directory
    #[arg(long)]
    pub working_dir: Option<PathBuf>,

    /// Output file for logs
    #[arg(long)]
    pub log_file: Option<PathBuf>,

    /// Enable experimental features
    #[arg(long)]
    pub experimental: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run the TUI interface
    Tui {
        /// Configuration file path
        #[arg(long)]
        config: Option<PathBuf>,

        /// Theme name
        #[arg(long, default_value = "default")]
        theme: String,

        /// Rate limit requests per minute
        #[arg(long, default_value = "60")]
        rate_limit: u32,

        /// Enable rate limiting
        #[arg(long)]
        enable_rate_limit: bool,
    },

    /// Run the web interface
    Web {
        /// Port for web server
        #[arg(long, default_value = "3000")]
        port: u16,

        /// Host for web server
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Configuration file path
        #[arg(long)]
        config: Option<PathBuf>,

        /// Custom avatar for web UI
        #[arg(long)]
        avatar: Option<PathBuf>,

        /// Theme name
        #[arg(long, default_value = "default")]
        theme: String,

        /// Rate limit requests per minute
        #[arg(long, default_value = "60")]
        rate_limit: u32,

        /// Enable rate limiting
        #[arg(long)]
        enable_rate_limit: bool,
    },

    /// Configuration management
    Config {
        /// List available themes
        #[arg(long)]
        list_themes: bool,

        /// List available providers
        #[arg(long)]
        list_providers: bool,

        /// Show current configuration
        #[arg(long)]
        show: bool,

        /// Edit configuration file
        #[arg(long)]
        edit: bool,

        /// Validate configuration
        #[arg(long)]
        validate: bool,

        /// Theme to configure
        #[arg(long)]
        theme: Option<String>,

        /// Provider to configure
        #[arg(long)]
        provider: Option<String>,
    },

    /// Execute a POML file
    Poml {
        /// POML file path
        #[arg(required = true)]
        file: PathBuf,

        /// Working directory
        #[arg(long)]
        working_dir: Option<PathBuf>,

        /// Output file
        #[arg(long)]
        output: Option<PathBuf>,

        /// Provider to use
        #[arg(long)]
        provider: Option<String>,

        /// Temperature
        #[arg(long, default_value = "0.7")]
        temperature: f32,

        /// Maximum tokens
        #[arg(long, default_value = "2000")]
        max_tokens: u32,

        /// Log level
        #[arg(long, default_value = "info")]
        log_level: String,

        /// Save results
        #[arg(long)]
        save: bool,
    },

    /// Manage extensions
    Extension {
        /// List extensions
        #[arg(long)]
        list: bool,

        /// Install extension
        #[arg(long)]
        install: Option<PathBuf>,

        /// Uninstall extension
        #[arg(long)]
        uninstall: Option<String>,

        /// Update extensions
        #[arg(long)]
        update: bool,

        /// Extension type: tool or mcp
        #[arg(long, default_value = "tool")]
        extension_type: String,
    },

    /// Show system information
    Info {
        /// Show detailed information
        #[arg(long)]
        detailed: bool,

        /// Show installed extensions
        #[arg(long)]
        extensions: bool,

        /// Show available themes
        #[arg(long)]
        themes: bool,
    },

    /// Test configuration
    Test {
        /// Test provider connection
        #[arg(long)]
        provider: bool,

        /// Test extensions
        #[arg(long)]
        extensions: bool,

        /// Run a quick test
        #[arg(long)]
        quick: bool,
    },
}

impl Cli {
    pub fn get_mode(&self) -> AppMode {
        if self.web {
            AppMode::Web
        } else if self.config {
            AppMode::Config
        } else if self.command.is_some() {
            AppMode::Command
        } else {
            AppMode::Tui
        }
    }

    pub fn get_port(&self) -> u16 {
        self.command
            .as_ref()
            .and_then(|cmd| match cmd {
                Commands::Web { port, .. } => Some(*port),
                _ => None,
            })
            .unwrap_or(self.port)
    }

    pub fn get_host(&self) -> String {
        self.command
            .as_ref()
            .and_then(|cmd| match cmd {
                Commands::Web { host, .. } => Some(host.clone()),
                _ => None,
            })
            .unwrap_or(self.host.clone())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Tui,
    Web,
    Config,
    Command,
}
