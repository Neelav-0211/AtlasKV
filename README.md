# AtlasKV

A high-performance, single-node key-value store written in Rust, inspired by the LSM-tree architecture used in databases like LevelDB and RocksDB.

## Features

- **LSM-Tree Architecture** — MemTable (in-memory `BTreeMap`) + SSTable (on-disk sorted files) for high write throughput
- **Write-Ahead Log (WAL)** — Append-only log with CRC32 checksums and configurable sync strategies (`EveryWrite` or batched `EveryNEntries`)
- **Crash Recovery** — Automatic WAL replay on startup with CRC validation, partial write detection, and truncation of corrupted entries
- **SSTable Persistence** — Custom binary format with header, data block, index block, and footer; supports tombstones for deletes
- **Single-Writer / Multi-Reader (SWMR)** — Write serialization via `Mutex`, concurrent reads via `parking_lot::RwLock`
- **TCP Server** — Blocking I/O with a thread pool (crossbeam bounded channels), non-blocking accept loop, configurable connection limits and timeouts
- **Custom Binary Protocol** — Compact wire format (1-byte command/status + 4-byte length + payload), 16 MB max payload
- **CLI Client** — One-shot command-line client (`get`, `set`, `del`, `ping`) with single-stream TCP pattern
- **Configuration Builder** — Fluent API for data directory, WAL strategy, memtable size limit, listen address, max connections, and timeouts

## Architecture

```
Client (CLI)                    Server
    │                              │
    │── TCP connect ──────────────►│
    │── encode(command) ─────────►│
    │                              ├─► Command Router
    │                              │     │
    │                              │     ├── GET  ─► MemTable ─► SSTables
    │                              │     ├── PUT  ─► write_lock ─► WAL ─► MemTable ─► (flush?)
    │                              │     ├── DEL  ─► write_lock ─► WAL ─► MemTable
    │                              │     └── PING ─► PONG
    │                              │
    │◄── encode(response) ────────┤
    │── shutdown(Write) ─────────►│  (half-close after response received)
    │                              │
```

### Component Layout

```
┌──────────────────────────────────────────────────────┐
│                    TCP Server                        │
│          (Thread Pool + Accept Loop)                 │
└──────────────────┬───────────────────────────────────┘
                   │
┌──────────────────▼───────────────────────────────────┐
│                    Engine                             │
│         (Coordinates all components)                 │
│                                                      │
│   write_lock: Mutex<()>     current_lsn: AtomicU64   │
└──────┬──────────────┬────────────────┬───────────────┘
       │              │                │
       ▼              ▼                ▼
 ┌───────────┐  ┌───────────┐   ┌────────────┐
 │    WAL    │  │ MemTable  │   │  Storage   │
 │ (append,  │  │ (RwLock   │   │  Manager   │
 │  fsync)   │  │  BTreeMap)│   │ (SSTables) │
 └───────────┘  └─────┬─────┘   └──────┬─────┘
                      │                │
                      └── flush ──────►│
```

### Data Flow

**Write Path** (`PUT` / `DEL`):
1. Acquire `write_lock` (ensures single-writer)
2. Increment LSN (atomic)
3. Append entry to WAL (with CRC32 checksum)
4. Insert into MemTable
5. If MemTable exceeds size limit → flush to new SSTable, clear MemTable, truncate WAL

**Read Path** (`GET`):
1. Check MemTable first (shared read lock)
2. If not found → search SSTables newest-to-oldest
3. Range filter: skip SSTables where key is outside `[min_key, max_key]`
4. Tombstone = key was deleted → return `NotFound`

**Crash Recovery**:
1. Discover existing SSTables on disk
2. Open WAL, read entries sequentially
3. Validate CRC for each entry; skip corrupted entries
4. Truncate WAL at last valid entry
5. Replay valid entries into MemTable

## On-Disk Formats

### WAL Entry

```
┌───────────┬───────────┬───────────┬──────────────────┐
│  LSN (8B) │ CRC32 (4B)│  Len (4B) │  Data (variable) │
└───────────┴───────────┴───────────┴──────────────────┘
```

### SSTable File

```
┌──────────────────────────────────────────────────────┐
│ Header (14B)                                         │
│   Magic: "ATKV" (4) │ Version: u16 (2) │ Count (8)  │
├──────────────────────────────────────────────────────┤
│ Data Block                                           │
│   [KeyLen: u32][ValLen: u32][Key][Value] × N         │
│   (ValLen = u32::MAX → tombstone, no value bytes)    │
├──────────────────────────────────────────────────────┤
│ Index Block                                          │
│   [KeyLen: u32][Offset: u64][Key] × N                │
├──────────────────────────────────────────────────────┤
│ Footer (16B)                                         │
│   IndexOffset: u64 (8) │ DataCRC: u32 (4) │ Pad (4) │
└──────────────────────────────────────────────────────┘
```

## Quick Start

### Build

```bash
cargo build --release
```

### Run the Server

```bash
# Default settings (data in ./atlaskv_data, listen on 127.0.0.1:6379)
./target/release/atlaskv-server

# Custom configuration
./target/release/atlaskv-server \
    --data-dir /path/to/data \
    --listen "127.0.0.1:6969" \
    --max-connections 4 \
    --memtable-mb 5
```

### Use the CLI

```bash
# Ping the server
./target/release/atlaskv-cli ping

# Set a key
./target/release/atlaskv-cli set mykey "hello world"

# Get a key
./target/release/atlaskv-cli get mykey

# Delete a key
./target/release/atlaskv-cli del mykey

# Connect to a specific server
./target/release/atlaskv-cli --server 127.0.0.1:6969 ping
```

## Configuration

| Parameter | Default | Description |
|---|---|---|
| `data_dir` | `./atlaskv_data` | Root directory for WAL and SSTable files |
| `wal_sync_strategy` | `EveryNEntries(100)` | WAL fsync frequency |
| `memtable_size_limit` | 64 MB | Flush threshold for the in-memory table |
| `listen_addr` | `127.0.0.1:6379` | TCP listen address |
| `max_connections` | 1024 | Maximum concurrent client connections |
| `read_timeout_ms` | 30000 | Per-connection read timeout (ms) |
| `write_timeout_ms` | 30000 | Per-connection write timeout (ms) |

## Project Structure

```
src/
├── lib.rs              # Public API re-exports
├── engine.rs           # Core engine (coordinates WAL, MemTable, Storage)
├── config.rs           # Configuration with builder pattern
├── error.rs            # Error types (thiserror)
├── bin/
│   ├── server.rs       # Server binary entry point
│   └── cli.rs          # CLI client binary
├── wal/
│   ├── entry.rs        # WAL entry format & serialization
│   ├── writer.rs       # Append-only WAL writer with fsync
│   ├── reader.rs       # Sequential WAL reader with CRC validation
│   └── recovery.rs     # Crash recovery: replay, truncation
├── memtable/
│   └── table.rs        # BTreeMap-backed MemTable with RwLock
├── storage/
│   ├── manager.rs      # Multi-SSTable query coordinator
│   └── sstable/
│       ├── builder.rs  # SSTable writer (flush from MemTable)
│       ├── reader.rs   # SSTable reader with in-memory index
│       └── iterator.rs # SSTable entry iterator
├── protocol/
│   ├── command.rs      # Command enum (Get, Put, Delete, Ping)
│   ├── response.rs     # Response struct (Status + optional payload)
│   └── codec.rs        # Binary encode/decode for wire protocol
└── network/
    ├── server.rs       # TCP server with thread pool
    └── connection.rs   # Per-connection command loop
```

## Design Documents

- [design.md](design.md) — V1 architecture, data flow, component details, and implementation phases
- [tradeoffs.md](tradeoffs.md) — Key design decisions and alternatives considered
- [future_optimizations.md](future_optimizations.md) — Potential optimizations to evaluate after benchmarking

## Roadmap (V2+)

- [ ] Compaction — merge multiple SSTables to reclaim space and remove stale tombstones
- [ ] Bloom Filters — probabilistic filter to speed up negative lookups
- [ ] Compression — LZ4/Snappy for SSTable data blocks
- [ ] Range Queries — scan/iterate API
- [ ] Distribution — Raft consensus for replication (long-term)

## License

MIT
