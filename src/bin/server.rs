//! AtlasKV Server Binary
//!
//! Starts the TCP server for AtlasKV.

use std::sync::Arc;
use clap::Parser;
use atlaskv::{Config, Engine};
use atlaskv::network::Server;

/// AtlasKV Server
#[derive(Parser, Debug)]
#[command(name = "atlaskv-server")]
#[command(about = "High-performance distributed key-value store")]
struct Args {
    /// Data directory
    #[arg(short, long, default_value = "./atlaskv_data")]
    data_dir: String,

    /// Listen address
    #[arg(short, long, default_value = "127.0.0.1:6379")]
    listen: String,
}

fn main() {
    let _args = Args::parse();

    // TODO: Initialize tracing/logging

    // TODO: Build config from args
    let _config = Config::default();

    // TODO: Open engine
    // let engine = Arc::new(Engine::open(config.clone()).expect("Failed to open engine"));

    // TODO: Start server
    // let mut server = Server::new(config, engine);
    // server.run().expect("Server error");

    println!("AtlasKV Server - Not yet implemented");
}
