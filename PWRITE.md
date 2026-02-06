# pwrite Baseline Benchmark Results

## Overview

This benchmark measures the raw performance of sequential file writes using `pwrite` system call, providing a baseline for comparison with RocksDB's write performance.

## Test Configuration

- **Key Size**: 16 bytes
- **Value Size**: 100 bytes
- **Entry Size**: 116 bytes
- **Write Pattern**: Sequential writes to a single file

## Benchmark Results

### 1. pwrite_sequential (Standard writes, no sync)

| Keys | Time | Throughput | Operations/sec |
|------|------|------------|----------------|
| 100,000 | 327.82 ms | 33.75 MiB/s | 305,078 ops/s |
| 600,000 | 2.03 s | 32.73 MiB/s | 295,813 ops/s |
| 3,000,000 | 10.75 s | 30.88 MiB/s | 279,070 ops/s |

**Characteristics**:
- Writes are buffered by OS page cache
- No explicit synchronization
- Baseline for pure file I/O performance

### 2. pwrite_with_sync (O_SYNC flag)

| Keys | Time | Throughput | Operations/sec |
|------|------|------------|----------------|
| 100,000 | 5.14 s | 2.15 MiB/s | 19,455 ops/s |

**Characteristics**:
- Each write is synchronized to disk immediately
- **~15.7x slower** than standard pwrite
- Provides durability guarantees per write

### 3. pwrite_with_fsync (Single fsync at end)

| Keys | Time | Throughput | Operations/sec |
|------|------|------------|----------------|
| 100,000 | 363.84 ms | 30.41 MiB/s | 274,830 ops/s |

**Characteristics**:
- Buffered writes + single fsync() at completion
- Only **~10% slower** than standard pwrite
- Efficient way to ensure durability

## Comparison with RocksDB

### Random Order Writes (100,000 keys)

| Implementation | Throughput | Operations/sec | vs pwrite baseline |
|----------------|------------|----------------|-------------------|
| **pwrite baseline** | 33.75 MiB/s | 305,078 ops/s | 1.0x (baseline) |
| RocksDB (WAL enabled) | 24.35 MiB/s | 220,126 ops/s | 0.72x (28% slower) |
| RocksDB (WAL disabled) | 79.06 MiB/s | 714,655 ops/s | **2.3x faster** |

### Sequential Order Writes (100,000 keys)

| Implementation | Throughput | Operations/sec | vs pwrite baseline |
|----------------|------------|----------------|-------------------|
| **pwrite baseline** | 33.75 MiB/s | 305,078 ops/s | 1.0x (baseline) |
| RocksDB (WAL enabled) | 29.38 MiB/s | 265,564 ops/s | 0.87x (13% slower) |
| RocksDB (WAL disabled) | 127.26 MiB/s | 1,150,287 ops/s | **3.8x faster** |

### Sync/Durability Comparison (100,000 keys)

| Implementation | Throughput | Operations/sec | Impact |
|----------------|------------|----------------|--------|
| pwrite (no sync) | 33.75 MiB/s | 305,078 ops/s | Baseline |
| pwrite (O_SYNC) | 2.15 MiB/s | 19,455 ops/s | **15.7x slower** |
| pwrite (fsync at end) | 30.41 MiB/s | 274,830 ops/s | 1.1x slower |
| RocksDB (WAL enabled, manual_wal_flush=false) | 43.20 MiB/s | 390,517 ops/s | **1.3x faster** |
| RocksDB (WAL enabled, manual_wal_flush=true) | 162.80 MiB/s | 1,471,724 ops/s | **4.8x faster** |

## Analysis

### Why is RocksDB with WAL slower than pwrite?

RocksDB with default WAL settings is 13-28% slower than raw pwrite because:
1. **Dual writes**: Data is written to both WAL and MemTable
2. **Format overhead**: WAL has additional metadata and checksums
3. **Locking**: Thread-safe operations add overhead
4. **Auto WAL flush**: Each write triggers WAL flush to disk

### Why is RocksDB without WAL faster than pwrite?

RocksDB with WAL disabled is 2.3-3.8x faster than raw pwrite because:
1. **Memory-only writes**: Data goes directly to MemTable (in-memory structure)
2. **Batching**: Multiple writes can be batched efficiently
3. **No I/O in critical path**: Disk writes happen asynchronously during flush
4. **Optimized data structures**: MemTable (skiplist/AVL tree) provides efficient insertion

### The Power of manual_wal_flush

RocksDB with `manual_wal_flush=true` achieves 4.8x better performance than raw pwrite while maintaining WAL for durability:
1. **Buffered WAL writes**: WAL writes are batched in memory
2. **Reduced fsync calls**: Flush happens on MemTable flush, not per-write
3. **Best of both worlds**: Durability + performance

## Key Takeaways

1. **Raw pwrite provides ~30-33 MiB/s** for sequential file writes on this hardware

2. **O_SYNC is catastrophic**: 15.7x slowdown makes per-write sync impractical

3. **RocksDB overhead is minimal**: Only 13-28% slower with full durability (WAL enabled)

4. **Memory-first design wins**: RocksDB without WAL is 2.3-3.8x faster than raw file writes

5. **Smart buffering is key**: `manual_wal_flush=true` provides near-optimal performance (4.8x faster than pwrite) while maintaining crash recovery

6. **The write amplification trade-off**:
   - pwrite: 1x write (direct to file)
   - RocksDB (WAL enabled): 2x write (WAL + eventual SSTable)
   - RocksDB (WAL disabled): Deferred write (memory first, disk later)

## Hardware Context

- **Platform**: Apple Silicon (aarch64-apple-darwin)
- **Storage**: Integrated SSD
- **OS Buffer Cache**: macOS page cache

Results will vary on different hardware, especially with enterprise NVMe SSDs or HDDs.
