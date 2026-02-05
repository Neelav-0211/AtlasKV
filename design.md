# AtlasKV V1 Design Document

## Overview

AtlasKV V1 is a single-node, high-performance key-value store with durability guarantees. This document outlines the architecture, data flow, and implementation plan.

## Goals

### V1 Scope
- [x] Single-node operation (no distribution)
- [ ] Write-Ahead Logging with crash recovery
- [ ] LSM-tree inspired architecture (MemTable + SSTable)
- [ ] Single-writer/multi-reader concurrency
- [ ] TCP-based client protocol
- [ ] Configurable durability (fsync strategies)

### Non-Goals for V1
- Distributed consensus (Raft/Paxos)
- Multi-node replication
- Transactions (multi-key atomicity)
- Range queries / iteration API
- Compression
- Bloom filters
- Compaction (basic implementation only)

---

## Architecture

### Component Diagram

```
                           ┌──────────────────┐
                           │   TCP Server     │
                           │  (Thread Pool)   │
                           └────────┬─────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    │               │               │
                    ▼               ▼               ▼
              ┌──────────┐   ┌──────────┐   ┌──────────┐
              │ Client 1 │   │ Client 2 │   │ Client N │
              └────┬─────┘   └────┬─────┘   └────┬─────┘
                   │              │              │
                   └──────────────┼──────────────┘
                                  │
                                  ▼
                         ┌────────────────┐
                         │     Engine     │
                         │ (Coordinator)  │
                         └───────┬────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              │                  │                  │
              ▼                  ▼                  ▼
       ┌────────────┐    ┌────────────┐    ┌────────────┐
       │    WAL     │    │  MemTable  │    │  Storage   │
       │  (Durability)   │ (In-Memory)│    │ (On-Disk)  │
       └────────────┘    └────────────┘    └────────────┘
```

### Data Flow

#### Write Path (PUT/DELETE)

```
1. Client sends PUT(key, value)
              │
              ▼
2. Engine acquires write lock
              │
              ▼
3. WAL: Append entry with LSN
   ┌─────────────────────┐
   │ LSN | CRC | Len | Data │
   └─────────────────────┘
              │
              ▼
4. MemTable: Insert key-value
   (or tombstone for DELETE)
              │
              ▼
5. Release write lock
              │
              ▼
6. Return success to client
              │
              ▼
7. (Async) If MemTable > size_limit:
   └──► Flush to new SSTable
   └──► Clear MemTable
   └──► Truncate WAL
```

#### Read Path (GET)

```
1. Client sends GET(key)
              │
              ▼
2. Engine acquires read lock
              │
              ▼
3. Check MemTable first
   ├── Found value ──► Return value
   ├── Found tombstone ──► Return NOT_FOUND
   └── Not found ──► Continue
              │
              ▼
4. Check SSTables (newest to oldest)
   ├── Found value ──► Return value
   ├── Found tombstone ──► Return NOT_FOUND
   └── Not found ──► Return NOT_FOUND
              │
              ▼
5. Release read lock
```

---

## Component Details

### 1. Write-Ahead Log (WAL)

**Purpose**: Ensure durability before acknowledging writes.

**Entry Format**:
```
┌───────────┬───────────┬───────────┬──────────────────┐
│  LSN (8)  │  CRC (4)  │  Len (4)  │    Data (var)    │
└───────────┴───────────┴───────────┴──────────────────┘
     │           │           │              │
     │           │           │              └── Serialized operation
     │           │           └── Length of data section
     │           └── CRC32 of (LSN + Len + Data)
     └── Log Sequence Number (monotonic)
```

**Operations**:
- `Put { key: Vec<u8>, value: Vec<u8> }`
- `Delete { key: Vec<u8> }`

**Sync Strategies**:
| Strategy | Durability | Performance |
|----------|------------|-------------|
| `EveryWrite` | Highest | Lowest |
| `Interval(ms)` | Configurable | Balanced |
| `OsDefault` | Lowest | Highest |

**Recovery**:
1. Open WAL file
2. Read entries sequentially
3. Validate CRC for each entry
4. Skip corrupted entries (log warning)
5. Truncate file at last valid entry
6. Replay valid entries to MemTable

### 2. MemTable

**Purpose**: Fast in-memory storage for recent writes.

**Data Structure**: `RwLock<BTreeMap<Vec<u8>, MemTableEntry>>`

**Why BTreeMap?**
- Ordered iteration (required for SSTable flush)
- Simple, correct implementation
- Good enough for V1; can optimize later with SkipList

**Entry Types**:
```rust
enum MemTableEntry {
    Value(Vec<u8>),   // Live value
    Tombstone,        // Deleted key
}
```

**Concurrency Model**:
- `RwLock` allows multiple readers OR single writer
- Write operations acquire exclusive lock
- Read operations acquire shared lock

**Size Tracking**:
- Track approximate size (sum of key + value sizes)
- Trigger flush when size > `memtable_size_limit`

### 3. Storage (SSTable)

**Purpose**: Persistent, sorted, immutable key-value storage.

**File Format (V1 - Simple)**:
```
┌────────────────────────────────────────┐
│ Header (16 bytes)                      │
│ ┌────────┬─────────┬─────────────────┐ │
│ │Magic(4)│Version(2)│ Entry Count(8) │ │
│ │"ATKV"  │  0x0001  │                │ │
│ └────────┴─────────┴─────────────────┘ │
├────────────────────────────────────────┤
│ Data Block (variable)                  │
│ ┌────────┬────────┬─────┬───────────┐ │
│ │KeyLen(4)│ValLen(4)│ Key │  Value   │ │
│ └────────┴────────┴─────┴───────────┘ │
│ (ValLen = 0 for tombstones)           │
│ ... repeated for each entry ...       │
├────────────────────────────────────────┤
│ Footer (16 bytes)                      │
│ ┌───────────────────┬────────────────┐ │
│ │ Min Key Offset(8) │   CRC32(4)     │ │
│ └───────────────────┴────────────────┘ │
└────────────────────────────────────────┘
```

**Lookup Strategy (V1)**:
- Linear scan (simple, correct)
- Future: Add index block for binary search

**File Naming**: `sstable_{id}.dat` where id is monotonic

### 4. Engine

**Purpose**: Coordinate all components, handle concurrency.

**State**:
```rust
struct Engine {
    config: Config,
    wal: Mutex<WalWriter>,
    memtable: MemTable,            // Has internal RwLock
    storage: RwLock<StorageManager>,
    write_lock: Mutex<()>,         // Serializes writes
    current_lsn: AtomicU64,
}
```

**Write Serialization**:
```rust
fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
    // 1. Acquire write lock (ensures single writer)
    let _guard = self.write_lock.lock();
    
    // 2. Write to WAL first
    let lsn = self.current_lsn.fetch_add(1, Ordering::SeqCst);
    let entry = WalEntry::new(lsn, Operation::Put { key, value });
    self.wal.lock().append(&entry)?;
    
    // 3. Write to MemTable
    self.memtable.put(key.to_vec(), value.to_vec())?;
    
    // 4. Check if flush needed
    if self.memtable.should_flush(self.config.memtable_size_limit) {
        self.flush_memtable()?;
    }
    
    Ok(())
}
```

### 5. Network

**Protocol**: Simple binary (see README.md)

**Server Architecture**:
```
┌─────────────────────────────────────────┐
│              Main Thread                │
│                                         │
│  loop {                                 │
│      conn = listener.accept()           │
│      pool.execute(|| handle(conn))      │
│  }                                      │
└─────────────────────────────────────────┘
                    │
       ┌────────────┼────────────┐
       ▼            ▼            ▼
┌──────────┐  ┌──────────┐  ┌──────────┐
│ Worker 1 │  │ Worker 2 │  │ Worker N │
│          │  │          │  │          │
│ loop {   │  │ loop {   │  │ loop {   │
│  read()  │  │  read()  │  │  read()  │
│  execute()│  │  execute()│  │  execute()│
│  write() │  │  write() │  │  write() │
│ }        │  │ }        │  │ }        │
└──────────┘  └──────────┘  └──────────┘
```

---

## Implementation Phases

### Phase 1: Core Storage
1. WAL entry format and serialization
2. WAL writer (append, sync)
3. MemTable (get, put, delete)
4. Basic Engine (coordinates WAL + MemTable)

**Deliverable**: In-memory KV store with WAL durability

### Phase 2: Persistence
1. SSTable format and writer
2. SSTable reader
3. StorageManager (multi-SSTable queries)
4. MemTable flush to SSTable
5. WAL truncation after flush

**Deliverable**: Persistent KV store with restart support

### Phase 3: Crash Recovery
1. WAL reader
2. CRC validation
3. Partial write detection
4. Recovery replay
5. SSTable discovery on startup

**Deliverable**: Crash-safe KV store

### Phase 4: Networking
1. Protocol codec
2. Connection handler
3. TCP server with thread pool
4. CLI client

**Deliverable**: Networked KV store with client

### Phase 5: Polish
1. Configuration builder
2. Logging/tracing
3. Metrics collection
4. Integration tests
5. Benchmarks

**Deliverable**: Production-ready V1

---

## Testing Strategy

### Unit Tests
- WAL entry serialization/deserialization
- CRC calculation and validation
- MemTable operations
- SSTable build and read
- Protocol codec

### Integration Tests
- Engine: basic CRUD operations
- Engine: flush triggers
- Engine: crash recovery (kill and restart)
- Server: concurrent clients
- Server: connection limits

### Stress Tests
- High write throughput
- High read throughput
- Mixed workload
- Large values
- Many keys

---

## Future Work (V2+)

- [ ] **Compaction**: Merge multiple SSTables
- [ ] **Bloom Filters**: Speed up negative lookups
- [ ] **Block Index**: Binary search within SSTables
- [ ] **Compression**: LZ4/Snappy for data blocks
- [ ] **Range Queries**: Scan/iterate API
- [ ] **Distribution**: Raft consensus for replication
- [ ] **Transactions**: Multi-key atomic operations
