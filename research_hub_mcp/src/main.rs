#![allow(clippy::field_reassign_with_default)]

use anyhow::Result;
use clap::{Parser, Subcommand};
use rust_research_mcp::{Config, ConfigOverrides, DaemonConfig, DaemonService, HttpServer, PidFile, Server};
use std::sync::Arc;
use tracing::{debug, error, info};

#[derive(Parser)]
#[command(name = "rust-research-mcp")]
#[command(about = "A MCP server for academic research and knowledge accumulation")]
#[command(version)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Configuration file path
    #[arg(short, long, global = true)]
    config: Option<std::path::PathBuf>,

    /// Override log level (trace, debug, info, warn, error)
    #[arg(long, global = true)]
    log_level: Option<String>,

    /// Set environment profile (development, production)
    #[arg(long, global = true)]
    profile: Option<String>,

    /// Override download directory path
    #[arg(long, global = true)]
    download_dir: Option<std::path::PathBuf>,

    /// Generate JSON schema for configuration
    #[arg(long)]
    generate_schema: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as MCP server (default mode, stdio transport)
    Mcp {
        /// Run as daemon
        #[arg(short, long)]
        daemon: bool,

        /// PID file path (for daemon mode)
        #[arg(long)]
        pid_file: Option<std::path::PathBuf>,

        /// Health check port
        #[arg(long, default_value = "8090")]
        health_port: u16,

        /// Override server port
        #[arg(long)]
        port: Option<u16>,

        /// Override server host
        #[arg(long)]
        host: Option<String>,
    },
    /// Run as HTTP REST API server
    Http {
        /// HTTP server port
        #[arg(short, long, default_value = "8091")]
        port: u16,

        /// HTTP server host
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .with_writer(std::io::stderr)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting rust-research-mcp");

    // Handle schema generation request
    if cli.generate_schema {
        let schema = Config::generate_schema();
        println!("{}", serde_json::to_string_pretty(&schema)?);
        return Ok(());
    }

    // Build base configuration overrides
    let base_overrides = ConfigOverrides {
        server_port: None,
        server_host: None,
        log_level: cli.log_level.or_else(|| {
            if cli.verbose {
                Some("debug".to_string())
            } else {
                None
            }
        }),
        profile: cli.profile,
        download_directory: cli.download_dir,
    };

    // Load configuration with proper precedence
    let config = if let Some(config_path) = cli.config.as_ref() {
        info!("Loading config from: {}", config_path.display());
        Config::load_with_overrides(Some(config_path), &base_overrides)?
    } else {
        Config::load_with_overrides(None, &base_overrides)?
    };

    // Log configuration (safely)
    let safe_config = config.safe_for_logging();
    info!(
        "Loaded configuration: profile={}, schema_version={}",
        safe_config.profile, safe_config.schema_version
    );
    debug!("Configuration details: {:#?}", safe_config);

    // Handle subcommands
    match cli.command {
        Some(Commands::Http { port, host }) => {
            info!("Starting HTTP REST API server mode");
            let server = HttpServer::new(config, port, host);
            match server.run().await {
                Ok(()) => {
                    info!("HTTP server shutdown completed successfully");
                    Ok(())
                }
                Err(e) => {
                    error!("HTTP server error: {}", e);
                    Err(e.into())
                }
            }
        }
        Some(Commands::Mcp {
            daemon,
            pid_file,
            health_port,
            port,
            host,
        }) => {
            // Apply MCP-specific overrides
            let mcp_overrides = ConfigOverrides {
                server_port: port,
                server_host: host,
                log_level: base_overrides.log_level.clone(),
                profile: base_overrides.profile.clone(),
                download_directory: base_overrides.download_directory.clone(),
            };

            let config = if let Some(config_path) = cli.config.as_ref() {
                Config::load_with_overrides(Some(config_path), &mcp_overrides)?
            } else {
                Config::load_with_overrides(None, &mcp_overrides)?
            };

            if daemon {
                info!("Starting in daemon mode");

                let mut daemon_config = DaemonConfig::default();
                daemon_config.daemon = true;
                daemon_config.health_port = health_port;
                daemon_config.pid_file = pid_file.or_else(|| Some(PidFile::standard_path()));

                let config = Arc::new(config);
                let mut daemon = DaemonService::new(config, daemon_config)?;

                match daemon.start().await {
                    Ok(()) => {
                        info!("Daemon service stopped");
                        Ok(())
                    }
                    Err(e) => {
                        error!("Daemon service error: {}", e);
                        Err(e.into())
                    }
                }
            } else {
                let server = Server::new(config);
                match server.run().await {
                    Ok(()) => {
                        info!("MCP server shutdown completed successfully");
                        Ok(())
                    }
                    Err(e) => {
                        error!("MCP server error: {}", e);
                        Err(e.into())
                    }
                }
            }
        }
        None => {
            // Default: run MCP server in foreground
            info!("Starting MCP server (default mode)");
            let server = Server::new(config);
            match server.run().await {
                Ok(()) => {
                    info!("Server shutdown completed successfully");
                    Ok(())
                }
                Err(e) => {
                    error!("Server error: {}", e);
                    Err(e.into())
                }
            }
        }
    }
}
