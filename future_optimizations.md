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

### 3. Time-Based + Background Worker Sync Strategy
**Status:** 🔖 TBD (To Be Decided)

**Current State:**
- V1 uses count-based batching: `EveryNEntries { count: usize }`
- Sync happens synchronously in `append()` call
- Simple, predictable, no threading complexity

**Proposal:**
Add optional time-based sync with background worker thread:

```rust
pub enum WalSyncStrategy {
    EveryWrite,
    EveryNEntries { count: usize },
    
    // New options:
    TimeBasedBackground { interval_ms: u64 },  // Background thread syncs every N ms
    Hybrid { count: usize, max_delay_ms: u64 }, // Sync on count OR timeout (whichever first)
}
```

**Implementation Approaches:**

**Option A: Dedicated Sync Thread (PostgreSQL Style)**
```rust
pub struct WalWriter {
    file: Arc<Mutex<BufWriter<File>>>,
    sync_thread: Option<JoinHandle<()>>,
}

// Background thread
thread::spawn(move || {
    loop {
        thread::sleep(Duration::from_millis(interval));
        writer_lock.lock().unwrap().sync().ok();
    }
});
```

**Option B: Channel-Based Signaling**
```rust
pub struct WalWriter {
    file: BufWriter<File>,
    sync_signal: Receiver<()>,
}

// Background thread sends wake-up signals
// Main thread checks signal in append(), syncs if triggered
```

**Option C: Tokio Async (Advanced)**
```rust
// Async WAL writer with background flush task
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        interval.tick().await;
        writer.sync().await?;
    }
});
```

**Benefits:**
- Writes never block on fsync (better latency)
- More predictable sync timing
- Can achieve PostgreSQL-like behavior
- Matches MongoDB WiredTiger's group commit pattern

**Trade-offs:**
- Threading complexity (mutex/channels)
- Potential data loss window if crash before background sync
- Harder to test and debug
- Mutex overhead on every write (Option A)

**Decision Criteria:**
- Benchmark latency improvement (blocking vs. background sync)
- Evaluate complexity vs. benefit
- Consider if users want this control
- Check if Engine layer should handle this instead

**Estimated Impact:**
- 10-30% reduction in write latency (no blocking on fsync)
- Slightly worse durability (delayed sync)
- Good for write-heavy workloads with acceptable data loss window

**Recommendation:**
- V1: Keep simple count-based sync
- V2: Add time-based background sync as opt-in feature
- Let Engine layer handle periodic sync for maximum flexibility

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
