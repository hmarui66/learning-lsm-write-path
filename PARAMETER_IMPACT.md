# RocksDB Parameter Impact Analysis

## Test Configuration

- **Dataset**: 100,000 keys (small dataset for quick testing)
- **Key Size**: 16 bytes
- **Value Size**: 100 bytes
- **Total Data**: ~11.1 MB

## Results Summary

### 1. WAL (Write Ahead Log) - **CRITICAL IMPACT**

| Setting | Time | Throughput | Impact |
|---------|------|------------|--------|
| WAL enabled (disable_wal=false) | 352.67 ms | 31.37 MiB/s | Baseline |
| WAL disabled (disable_wal=true) | 80.02 ms | 138.25 MiB/s | **4.4x faster** |

**Conclusion**: WAL has the **most significant impact** on write performance. Disabling it provides a 4.4x speedup but sacrifices durability.

### 2. Sync (fsync) - **EXTREME IMPACT**

| Setting | Estimated Time | Impact |
|---------|----------------|--------|
| sync=false | ~80 ms | Baseline |
| sync=true | ~411 seconds | **~5000x slower** |

**Conclusion**: Enabling fsync has an **extreme negative impact** on performance. This is expected as each write requires disk synchronization.

**Note**: sync=true requires WAL to be enabled, so the impact combines both WAL overhead and fsync overhead.

### 3. Auto Compaction - **MINIMAL IMPACT**

| Setting | Time | Throughput | Impact |
|---------|------|------------|--------|
| Auto compaction enabled (false) | 80.46 ms | 137.49 MiB/s | Baseline |
| Auto compaction disabled (true) | 82.55 ms | 134.02 MiB/s | **No significant difference** |

**Conclusion**: Auto compaction has **minimal impact** on write performance for small datasets. This is because compaction is triggered asynchronously and doesn't directly affect write path.

### 4. Concurrent MemTable Write - **MINIMAL IMPACT**

| Setting | Time | Throughput | Impact |
|---------|------|------------|--------|
| Concurrent write disabled (false) | 80.47 ms | 137.48 MiB/s | Baseline |
| Concurrent write enabled (true) | 81.09 ms | 136.42 MiB/s | **No significant difference** |

**Conclusion**: Concurrent memtable writes have **minimal impact** on single-threaded workloads. The benefit would be more apparent in multi-threaded scenarios.

## Overall Analysis

### Critical Parameters (High Impact)

1. **disable_wal=true**: 4.4x speedup - **Essential for bulk load**
2. **sync=false**: Avoids 5000x slowdown - **Essential for bulk load**

### Minor Parameters (Low Impact)

3. **disable_auto_compactions**: ~2% difference - Minimal impact
4. **allow_concurrent_memtable_write**: ~1% difference - Minimal impact

## Recommendations

### For Bulk Load (Maximum Performance)
```rust
opts.set_disable_auto_compactions(true);  // Prevents background compaction
opts.set_allow_concurrent_memtable_write(false);  // Simpler for single-threaded

write_opts.disable_wal(true);  // CRITICAL: 4.4x speedup
write_opts.set_sync(false);    // CRITICAL: Avoids extreme slowdown
```

### For Production (Durability)
```rust
opts.set_disable_auto_compactions(false);  // Enable compaction
opts.set_allow_concurrent_memtable_write(true);  // Better for multi-threaded

write_opts.disable_wal(false);  // Enable WAL for durability
write_opts.set_sync(false);     // Still avoid fsync for performance
```

### For Maximum Durability (Crash Recovery)
```rust
opts.set_disable_auto_compactions(false);
opts.set_allow_concurrent_memtable_write(true);

write_opts.disable_wal(false);  // Enable WAL
write_opts.set_sync(true);      // Enable fsync (5000x slower!)
```

## Key Insights

1. **WAL and fsync dominate performance**: These two parameters have orders of magnitude more impact than any other settings.

2. **Auto compaction and concurrent writes are red herrings**: Despite being mentioned in benchmark.sh, they have minimal impact on write performance for single-threaded workloads.

3. **The real optimization is I/O avoidance**: Disabling WAL and fsync means avoiding disk writes and synchronization, which are the true bottlenecks.

4. **Trade-off is durability vs performance**: The 4.4x speedup from disabling WAL comes at the cost of losing crash recovery capabilities.
