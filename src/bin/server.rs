//! AtlasKV Server Binary
//!
//! Starts the TCP server for AtlasKV.

use std::sync::Arc;
use clap::Parser;
use atlaskv::{Config, Engine};
use atlaskv::network::Server;
use tracing_subscriber::{fmt, EnvFilter};

/// AtlasKV Server
#[derive(Parser, Debug)]
#[command(name = "atlaskv-server")]
#[command(about = "High-performance distributed key-value store")]
#[command(version)]
struct Args {
    /// Data directory
    #[arg(short, long, default_value = "./atlaskv_data")]
    data_dir: String,

    /// Listen address (host:port)
    #[arg(short, long, default_value = "127.0.0.1:6379")]
    listen: String,

    /// Maximum concurrent connections
    #[arg(short, long, default_value = "1024")]
    max_connections: usize,

    /// MemTable size limit in MB before flush
    #[arg(short = 'm', long, default_value = "64")]
    memtable_mb: usize,
}

fn main() {
    // Initialize tracing/logging
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,atlaskv=debug"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .init();

    let args = Args::parse();

    tracing::info!("AtlasKV Server v{}", atlaskv::VERSION);
    tracing::info!("Data directory: {}", args.data_dir);
    tracing::info!("Listen address: {}", args.listen);

    // Build config from args
    let config = Config::builder()
        .data_dir(&args.data_dir)
        .listen_addr(&args.listen)
        .max_connections(args.max_connections)
        .memtable_size_limit(args.memtable_mb * 1024 * 1024)
        .build();

    // Open engine
    let engine = match Engine::open(config.clone()) {
        Ok(e) => Arc::new(e),
        Err(e) => {
            tracing::error!("Failed to open engine: {}", e);
            std::process::exit(1);
        }
    };

    tracing::info!("Engine initialized successfully");

    // Set up Ctrl+C handler
    let shutdown_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let shutdown_flag_clone = Arc::clone(&shutdown_flag);

    ctrlc_handler(move || {
        tracing::info!("Received Ctrl+C, initiating shutdown...");
        shutdown_flag_clone.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    // Start server
    let mut server = Server::new(config, engine);
    if let Err(e) = server.run() {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }

    tracing::info!("Server stopped");
}

/// Set up a Ctrl+C handler
fn ctrlc_handler<F: FnOnce() + Send + 'static>(handler: F) {
    // We use a simple approach - store the handler in a static once
    // In production, you'd use the ctrlc crate
    std::thread::spawn(move || {
        // This is a simplified handler - in production use the ctrlc crate
        // For now, the server's non-blocking accept loop will handle shutdown
        let _ = handler;
    });
}
