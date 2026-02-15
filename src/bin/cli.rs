//! AtlasKV CLI Client
//!
//! Command-line interface for interacting with AtlasKV.
//!
//! ## Connection Handling
//!
//! Uses a single TCP stream for sequential write-then-read, following the same
//! pattern as Redis clients (redis-cli, mini-redis). This avoids the pitfalls
//! of cloning the socket into separate reader/writer handles, which causes
//! connection abort errors (OS error 10053) on Windows due to the OS-level
//! socket shutdown affecting all cloned handles.

use std::io::{BufReader, Write};
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

use clap::{Parser, Subcommand};
use atlaskv::protocol::{
    Command, Response, Status,
    encode_command, read_response,
};

/// AtlasKV CLI
#[derive(Parser, Debug)]
#[command(name = "atlaskv-cli")]
#[command(about = "CLI for AtlasKV key-value store")]
#[command(version)]
struct Args {
    /// Server address (host:port)
    #[arg(short, long, default_value = "127.0.0.1:6379")]
    server: String,

    /// Connection timeout in milliseconds
    #[arg(short, long, default_value = "5000")]
    timeout: u64,

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
    let args = Args::parse();

    // Convert CLI command to protocol command
    let command = match &args.command {
        Commands::Get { key } => Command::Get {
            key: key.as_bytes().to_vec(),
        },
        Commands::Set { key, value } => Command::Put {
            key: key.as_bytes().to_vec(),
            value: value.as_bytes().to_vec(),
        },
        Commands::Del { key } => Command::Delete {
            key: key.as_bytes().to_vec(),
        },
        Commands::Ping => Command::Ping,
    };

    // Connect to server
    let mut stream = match TcpStream::connect_timeout(
        &args.server.parse().expect("Invalid server address"),
        Duration::from_millis(args.timeout),
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to connect to {}: {}", args.server, e);
            std::process::exit(1);
        }
    };

    // Set timeouts
    let _ = stream.set_read_timeout(Some(Duration::from_millis(args.timeout)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(args.timeout)));
    
    // Disable Nagle's algorithm for immediate sends (avoid buffering delays)
    let _ = stream.set_nodelay(true);

    // === Single-stream sequential write-then-read ===
    //
    // We avoid cloning the TcpStream into separate reader/writer handles.
    // On Windows, cloned socket handles share the same underlying OS socket,
    // and shutdown() on one handle affects all of them — causing spurious
    // "connection aborted" (OS error 10053) errors when the server takes time
    // to respond (e.g., during memtable flush).
    //
    // Instead, we encode the command to bytes, write directly, then wrap the
    // stream in a BufReader only for reading the response. This is the same
    // pattern used by Redis clients (redis-cli, mini-redis).

    // Step 1: Write command bytes directly to the stream
    let cmd_bytes = encode_command(&command);
    if let Err(e) = stream.write_all(&cmd_bytes) {
        eprintln!("Failed to send command: {}", e);
        std::process::exit(1);
    }
    if let Err(e) = stream.flush() {
        eprintln!("Failed to flush command: {}", e);
        std::process::exit(1);
    }

    // Step 2: Read response from the same stream
    let mut reader = BufReader::new(&stream);
    let response = match read_response(&mut reader) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to read response: {}", e);
            std::process::exit(1);
        }
    };

    // Step 3: Half-close write side so the server's read loop sees EOF
    // immediately instead of waiting for a read timeout. This is safe now
    // because we've already received the response.
    let _ = stream.shutdown(Shutdown::Write);
    drop(reader);
    drop(stream);

    // Handle response based on command
    handle_response(&args.command, response);
}

fn handle_response(cmd: &Commands, response: Response) {
    match response.status {
        Status::Ok => {
            match cmd {
                Commands::Get { .. } => {
                    if let Some(value) = response.payload {
                        // Try to print as UTF-8, fall back to hex
                        match String::from_utf8(value.clone()) {
                            Ok(s) => println!("{}", s),
                            Err(_) => println!("{:?}", value),
                        }
                    } else {
                        println!("(nil)");
                    }
                }
                Commands::Set { .. } => {
                    println!("OK");
                }
                Commands::Del { .. } => {
                    println!("OK");
                }
                Commands::Ping => {
                    if let Some(value) = response.payload {
                        match String::from_utf8(value) {
                            Ok(s) => println!("{}", s),
                            Err(_) => println!("PONG"),
                        }
                    } else {
                        println!("PONG");
                    }
                }
            }
        }
        Status::NotFound => {
            println!("(nil)");
        }
        Status::Error => {
            if let Some(payload) = response.payload {
                match String::from_utf8(payload) {
                    Ok(msg) => eprintln!("ERROR: {}", msg),
                    Err(_) => eprintln!("ERROR: (unknown error)"),
                }
            } else {
                eprintln!("ERROR: (unknown error)");
            }
            std::process::exit(1);
        }
    }
}
