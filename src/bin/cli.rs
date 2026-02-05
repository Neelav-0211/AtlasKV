//! AtlasKV CLI Client
//!
//! Command-line interface for interacting with AtlasKV.

use clap::{Parser, Subcommand};

/// AtlasKV CLI
#[derive(Parser, Debug)]
#[command(name = "atlaskv-cli")]
#[command(about = "CLI for AtlasKV key-value store")]
struct Args {
    /// Server address
    #[arg(short, long, default_value = "127.0.0.1:6379")]
    server: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Get a value by key
    Get {
        /// The key to get
        key: String,
    },

    /// Set a key-value pair
    Set {
        /// The key to set
        key: String,

        /// The value to set
        value: String,
    },

    /// Delete a key
    Del {
        /// The key to delete
        key: String,
    },

    /// Ping the server
    Ping,
}

fn main() {
    let _args = Args::parse();

    // TODO: Connect to server
    // TODO: Execute command
    // TODO: Print result

    println!("AtlasKV CLI - Not yet implemented");
}
