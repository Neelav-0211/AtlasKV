# Future Optimizations

This document tracks potential optimizations that should be evaluated after benchmarking.

---

## WAL Entry Optimizations

### 1. Remove Timestamp Field
**Status:** 🔖 TBD (To Be Decided)

**Current State:**
- `WalEntry` includes a `timestamp: u64` field (8 bytes per entry)
- Timestamp is added automatically in `WalEntry::new()`

**Proposal:**
- Remove timestamp field entirely
- LSN provides sufficient ordering for recovery

**Benefits:**
- Reduces entry size by 8 bytes
- Eliminates one `SystemTime` call per write
- Simpler serialization/deserialization

**Trade-offs:**
- Lose ability for point-in-time recovery
- Lose debugging/monitoring timestamps
- No replication lag visibility

**Decision Criteria:**
- Benchmark write throughput with/without timestamp
- Evaluate if PITR is needed for V1
- Check if monitoring needs timestamp data

**Estimated Impact:**
- ~5-10% reduction in WAL size
- Negligible performance improvement (SystemTime call is cheap)

---

### 2. Single-Allocation CRC Construction
**Status:** 🔖 TBD (To Be Decided)

**Current State:**
```rust
// Two allocations
crc_buffer = [LSN + Len + Data]  // First allocation for CRC
output = [LSN + CRC + Len + Data]  // Second allocation for output
```

**Proposal:**
```rust
// One allocation with placeholder
output = [LSN + 0x00_00_00_00 + Len + Data]  // Placeholder for CRC
// Compute CRC over bytes[0..8] and bytes[12..end]
// Write CRC into bytes[8..12]
```

**Benefits:**
- Single allocation instead of two
- No intermediate buffer copying
- Reduced memory pressure

**Complexity:**
- Need to hash non-contiguous ranges OR
- Use stateful hasher (e.g., `crc32fast::Hasher`)

**Implementation:**
```rust
let mut hasher = crc32fast::Hasher::new();
hasher.update(&output[0..8]);    // LSN
hasher.update(&output[12..]);    // Len + Data
let crc = hasher.finalize();
output[8..12].copy_from_slice(&crc.to_le_bytes());
```

**Decision Criteria:**
- Benchmark write throughput (current vs. optimized)
- Measure allocation overhead in profiler
- Check if complexity trade-off is worth it

**Estimated Impact:**
- 5-15% reduction in allocation overhead
- Most benefit seen under high write loads
- Minimal impact if writes are I/O bound

---

## Evaluation Process

1. **Implement benchmarks** for WAL write operations
2. **Profile** hot paths to identify bottlenecks
3. **Measure** baseline performance
4. **Implement** optimizations in feature branch
5. **Compare** benchmarks before/after
6. **Decide** based on:
   - Performance gain vs. complexity increase
   - Feature trade-offs (e.g., losing timestamps)
   - V1 scope and priorities
