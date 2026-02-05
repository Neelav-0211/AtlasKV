# AtlasKV

A high-performance, distributed key-value store written in Rust.

## Features

- **Write-Ahead Logging (WAL)**: Durability guarantees with configurable sync strategies
- **Crash Recovery**: Automatic recovery with partial write handling
- **Single-Writer/Multi-Reader**: Optimized concurrency model
- **TCP Protocol**: Simple binary protocol for client connections
- **LSM-Tree Inspired**: MemTable + SSTable architecture for high write throughput

## Project Status

🚧 **V1 In Development** - Not ready for production use.

See [design.md](design.md) for the V1 architecture plan and [tradeoffs.md](tradeoffs.md) for design decisions.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      TCP Server                              │
│                  (Multiple Clients)                          │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│                   Command Router                             │
│            (Single Writer / Multi Reader)                    │
└─────────────────────┬───────────────────────────────────────┘
                      │
         ┌────────────┴────────────┐
         │                         │
         ▼                         ▼
  ┌─────────────┐          ┌─────────────┐
  │     WAL     │          │  MemTable   │
  │  (Append)   │          │  (RwLock)   │
  └─────────────┘          └──────┬──────┘
                                  │
                                  ▼
                          ┌─────────────┐
                          │   Storage   │
                          │  (SSTable)  │
                          └─────────────┘
```

## License

MIT
