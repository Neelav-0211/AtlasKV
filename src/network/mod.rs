//! Network Module
//!
//! TCP server and client handling.
//!
//! ## Architecture
//! - Single acceptor thread
//! - Worker thread pool for connections
//! - Commands routed through Engine

mod server;
mod connection;

pub use server::Server;
pub use connection::Connection;
