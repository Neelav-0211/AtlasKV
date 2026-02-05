# AtlasKV Design Tradeoffs

This document captures the key design decisions planned for AtlasKV V1.

---

## 1. Concurrency Model

**Decision**: Single-Writer / Multi-Reader with RwLock

**Why**: Prioritizes correctness over maximum throughput. Simple, debuggable, eliminates race conditions in WAL sequencing.

**Alternatives not chosen**: Lock-free SkipList (complex), Sharded locks (cross-shard issues), MVCC (memory overhead), Actor model (async complexity).

---

## 2. WAL Sync Strategy

**Decision**: Configurable fsync with batched interval default (100ms)

**Why**: Balances durability and performance. Most apps tolerate ~100ms potential data loss. Users can choose stricter guarantees.

**Options**:
- `EveryWrite` - safest, slowest
- `Interval(ms)` - balanced default
- `OsDefault` - fastest, least safe

---

## 3. MemTable Data Structure

**Decision**: BTreeMap with RwLock

**Why**: Ordered iteration required for SSTable flush. Simple, correct. Can swap to SkipList later if benchmarks justify.

**Alternatives not chosen**: HashMap (unordered), SkipList (complex for V1).

---

## 4. SSTable Format

**Decision**: Simple linear format (no index block for V1)

**Why**: Correctness first. Most reads hit MemTable. Index block is straightforward V2 addition.

---

## 5. Serialization

**Decision**: bincode for internal storage, custom binary for wire protocol

**Why**: bincode is fast/compact for Rust internals. Custom binary protocol is simple and language-agnostic for clients.

---

## 6. Network Model

**Decision**: Blocking I/O with thread pool

**Why**: Simple, sufficient for ~1000 concurrent connections. Async adds complexity without clear benefit for V1 scale.

---

## 7. Partial Write Detection

**Decision**: CRC32 checksum per WAL entry

**Why**: Detects corruption, isolates damage to single entry. Fast (hardware accelerated). 4 bytes overhead is negligible.

---

## 8. Distribution

**Decision**: Single-node only for V1

**Why**: Solid single-node foundation first. Distribution adds massive complexity. Design doesn't preclude future replication.

---

*Decisions will be revisited when benchmarks show bottlenecks or real use cases demand changes.*
