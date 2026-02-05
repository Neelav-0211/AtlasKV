# AtlasKV Design Tradeoffs

This document captures the key design decisions made for AtlasKV V1, the alternatives considered, and the rationale behind each choice.

---

## 1. Concurrency Model

### Decision: Single-Writer / Multi-Reader with RwLock

**Chosen Approach**:
- One writer at a time (Mutex-guarded)
- Multiple concurrent readers (RwLock shared access)
- Simple, correct, predictable

**Alternatives Considered**:

| Approach | Pros | Cons |
|----------|------|------|
| **Lock-free SkipList** | Higher write throughput, no writer blocking | Complex, harder to debug, memory ordering bugs |
| **Sharded locks** | Better parallelism for writes | Complexity, cross-shard operations tricky |
| **MVCC (Multi-Version)** | Reads never block writes | Memory overhead, garbage collection needed |
| **Actor model** | Clear ownership, no locks | Message passing overhead, Rust async complexity |

**Rationale**:
- V1 prioritizes correctness over maximum throughput
- RwLock is well-understood and debuggable
- Single-writer eliminates race conditions in WAL sequencing
- Can upgrade to lock-free structures in V2 with benchmarks to justify

**Implications**:
- Write operations are serialized (one at a time)
- Read operations can proceed in parallel
- Writes block reads during the critical section (WAL + MemTable update)

---

## 2. WAL Sync Strategy

### Decision: Configurable with Interval Default (100ms)

**Chosen Approach**:
```rust
enum WalSyncStrategy {
    EveryWrite,           // fsync after each write
    Interval { ms: u64 }, // fsync periodically (default: 100ms)
    OsDefault,            // no explicit fsync
}
```

**Tradeoff Analysis**:

| Strategy | Durability Window | Writes/sec (est.) | Use Case |
|----------|-------------------|-------------------|----------|
| `EveryWrite` | 0 (immediate) | ~1,000-5,000 | Financial, critical data |
| `Interval(100ms)` | 0-100ms | ~50,000-100,000 | Balanced default |
| `OsDefault` | OS-dependent (30s+) | ~200,000+ | Caching, non-critical |

**Rationale**:
- 100ms interval balances durability and performance
- Most applications can tolerate ~100ms of potential data loss
- Users can configure stricter guarantees if needed
- Power loss in 100ms window is rare; crash recovery handles process crashes

**Implications**:
- Default: Up to 100ms of writes may be lost on power failure
- Users must explicitly choose `EveryWrite` for zero data loss
- `OsDefault` should only be used for caching scenarios

---

## 3. MemTable Data Structure

### Decision: BTreeMap with RwLock

**Chosen Approach**:
```rust
struct MemTable {
    data: RwLock<BTreeMap<Vec<u8>, MemTableEntry>>,
    size: AtomicUsize,
}
```

**Alternatives Considered**:

| Structure | Pros | Cons |
|-----------|------|------|
| **BTreeMap + RwLock** | Ordered, simple, correct | Lock contention under high write load |
| **SkipList (crossbeam)** | Lock-free reads, concurrent writes | More complex, memory overhead |
| **HashMap + RwLock** | O(1) lookup | Unordered (bad for SSTable flush) |
| **LSM MemTable (leveled)** | Better for range queries | Overkill for V1 |

**Rationale**:
- BTreeMap provides ordered iteration (required for SSTable generation)
- RwLock is simple and well-tested
- V1 isn't targeting extreme write throughput
- Can swap to SkipList in V2 without API changes

**Implications**:
- All writes go through a single lock
- Iteration for flush is already sorted (no extra sort step)
- Memory layout may not be cache-optimal

---

## 4. SSTable Format

### Decision: Simple Linear Format (No Index Block)

**Chosen Approach**:
```
┌────────────────┐
│ Header (16B)   │
├────────────────┤
│ Data Entries   │  ◄── Linear scan for lookups
│ (key-value)    │
├────────────────┤
│ Footer (16B)   │
└────────────────┘
```

**Alternatives Considered**:

| Format | Pros | Cons |
|--------|------|------|
| **Linear (chosen)** | Simple, easy to implement | O(n) lookups |
| **With index block** | O(log n) lookups | More complex, V1 scope creep |
| **Block-based (LevelDB)** | Compression, efficient | Significant complexity |
| **B-tree pages** | Range queries, updates | SSTables are immutable anyway |

**Rationale**:
- V1 focuses on correctness, not read performance
- Most reads hit MemTable (recent data)
- SSTables should be small initially (flushed often)
- Index block is straightforward V2 addition

**Implications**:
- Large SSTables will have slow point lookups
- Range scans are efficient (already sequential)
- Should tune MemTable size to keep SSTables reasonable

---

## 5. Serialization Format

### Decision: bincode for Internal, Custom Binary for Protocol

**Internal Storage (WAL, SSTable)**:
- bincode: Compact, fast, Rust-native
- Handles versioning through struct evolution

**Wire Protocol**:
- Custom binary: Simple, language-agnostic
- Easy to implement clients in any language

**Alternatives Considered**:

| Format | Pros | Cons |
|--------|------|------|
| **bincode** | Fast, compact, derive macros | Rust-specific |
| **JSON** | Human-readable, debuggable | Verbose, slow |
| **MessagePack** | Compact, cross-language | Extra dependency |
| **Protocol Buffers** | Schema evolution, cross-lang | Complexity, code gen |
| **FlatBuffers** | Zero-copy | Complex, overkill |

**Rationale**:
- bincode for internal = maximum performance, Rust ecosystem fit
- Custom binary protocol = simple enough to implement in any language
- JSON can be added as optional protocol later

**Implications**:
- WAL files are not human-readable
- Protocol is simple but needs documentation
- Clients in other languages need to implement binary parsing

---

## 6. Error Handling Strategy

### Decision: Custom Error Enum with thiserror

**Chosen Approach**:
```rust
#[derive(Debug, Error)]
pub enum AtlasError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("WAL corruption: {0}")]
    WalCorruption(String),
    // ...
}
```

**Alternatives Considered**:

| Approach | Pros | Cons |
|----------|------|------|
| **Custom enum (chosen)** | Type-safe, matchable | Boilerplate |
| **anyhow::Error** | Ergonomic, less code | Not matchable, opaque |
| **Box<dyn Error>** | Flexible | No type safety |
| **Result<T, String>** | Simple | No error chaining |

**Rationale**:
- Library code should expose typed errors
- Callers can match on error variants and handle appropriately
- thiserror reduces boilerplate while keeping types
- Applications can wrap in anyhow if desired

**Implications**:
- Must define error variants for all failure modes
- Error conversion via `From` impls
- API is stable and explicit about failure modes

---

## 7. Network Architecture

### Decision: Blocking I/O with Thread Pool

**Chosen Approach**:
```
Main Thread: accept() loop
     │
     └──► Thread Pool: handle_connection()
```

**Alternatives Considered**:

| Model | Pros | Cons |
|-------|------|------|
| **Thread-per-connection** | Simple | Doesn't scale (10K+ threads) |
| **Thread pool (chosen)** | Bounded resources, simple | Connection limits |
| **async/await (tokio)** | Scales to 100K+ connections | Complexity, colored functions |
| **io_uring** | Maximum performance | Linux-only, complex |

**Rationale**:
- V1 targets hundreds to low thousands of connections
- Thread pool is simple and sufficient
- Async adds significant complexity (lifetime issues, executor choice)
- Can migrate to async in V2 if connection scale requires it

**Implications**:
- Max ~1000 concurrent connections efficiently
- Each connection has dedicated thread from pool
- Simple mental model for connection handling

---

## 8. Partial Write Detection

### Decision: CRC32 Checksum per WAL Entry

**Chosen Approach**:
```
┌───────────┬───────────┬───────────┬──────────────────┐
│  LSN (8)  │  CRC (4)  │  Len (4)  │    Data (var)    │
└───────────┴───────────┴───────────┴──────────────────┘
                 │
                 └── CRC32(LSN + Len + Data)
```

**Recovery Process**:
1. Read entry header (LSN, CRC, Len)
2. Read data of specified length
3. Compute CRC, compare with stored
4. If mismatch or incomplete: truncate here, stop

**Alternatives Considered**:

| Method | Pros | Cons |
|--------|------|------|
| **CRC per entry (chosen)** | Detects corruption, simple | 4 bytes overhead per entry |
| **CRC per block** | Less overhead | Loses more data on corruption |
| **Checksummed pages** | Common in databases | More complex |
| **No checksum** | Simpler | Silent corruption possible |

**Rationale**:
- Per-entry CRC isolates corruption to single entry
- CRC32 is fast (hardware accelerated on modern CPUs)
- 4 bytes overhead is negligible for typical entry sizes
- Partial writes always fail CRC validation

**Implications**:
- Can detect and skip corrupted entries during recovery
- Partial writes at end of file are safely truncated
- No silent data corruption in WAL

---

## 9. Distribution Model (V1)

### Decision: Single-Node Only

**Chosen Approach**: No distribution in V1. Single-node with hooks for future expansion.

**Rationale**:
- Distribution adds massive complexity (consensus, partitioning, failure handling)
- V1 needs solid single-node foundation first
- LSN ordering and WAL design support future replication
- Better to have correct single-node than broken distributed

**Future Path (V2+)**:
1. Primary-replica replication (async)
2. Raft consensus for strong consistency
3. Consistent hashing for partitioning

**Implications**:
- V1 is not highly available
- V1 cannot scale beyond single machine
- Design doesn't preclude distribution later

---

## 10. Key/Value Size Limits

### Decision: Soft Limits with Configurable Maximums

**Chosen Approach**:
```rust
const DEFAULT_MAX_KEY_SIZE: usize = 64 * 1024;     // 64 KB
const DEFAULT_MAX_VALUE_SIZE: usize = 16 * 1024 * 1024; // 16 MB
```

**Rationale**:
- Keys should be small (used for indexing)
- Values can be larger but bounded (memory pressure)
- Limits are configurable for specialized use cases
- Prevents accidental memory exhaustion

**Alternatives**:
- No limits (dangerous)
- Streaming for large values (complex)
- External value storage (scope creep)

**Implications**:
- Large value workloads may need tuning
- Keys in MemTable consume memory
- SSTable format handles any size within limits

---

## Summary Table

| Decision | Choice | Key Rationale |
|----------|--------|---------------|
| Concurrency | Single-Writer/Multi-Reader | Correctness over throughput |
| WAL Sync | Configurable (100ms default) | Balance durability/perf |
| MemTable | BTreeMap + RwLock | Simple, ordered, correct |
| SSTable | Linear format | Simple for V1, add index later |
| Serialization | bincode + custom protocol | Fast internal, simple wire |
| Errors | Custom enum + thiserror | Type-safe, matchable |
| Network | Blocking + thread pool | Simple, sufficient scale |
| Corruption | CRC32 per entry | Reliable detection |
| Distribution | Single-node | Foundation first |
| Size Limits | 64KB keys, 16MB values | Reasonable defaults |

---

## Revisiting Decisions

These decisions should be revisited when:

1. **Benchmarks show bottlenecks** - Profile before optimizing
2. **Use cases demand features** - Real users > hypothetical needs
3. **V2 planning begins** - With V1 learnings in hand

Document any changes in this file with date and rationale.
