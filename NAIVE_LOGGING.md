# Naive Logging Implementation Benchmark Results

## Environment

- **Implementation**: Custom Rust logging system with LSM-like write path
- **Rust Version**: 1.93.0
- **Platform**: macOS (aarch64-apple-darwin)
- **Benchmark Tool**: Criterion 0.8.1

## Configuration

- **Key Size**: 16 bytes
- **Value Size**: 100 bytes
- **Entry Size**: 116 bytes
- **MemTable Size Threshold**: 64 MB
- **max_write_buffer_number**: 2 (default, RocksDB compatible)

### Write Path Options

- `fsync`: false (no sync_all, buffered writes only)
- `BufWriter`: true (8KB buffer, default)
- `max_write_buffer_number`: 2 (1 mutable + 1 immutable max)
- Background flush: Single thread
- Write stall: Enabled when immutable memtable limit reached

## Test Parameters

| Keys | Total Size | Expected Flushes |
|------|------------|------------------|
| 100,000 | ~11.1 MB | 0 |
| 600,000 | ~69.6 MB | 1 |
| 3,000,000 | ~348 MB | ~5 |

## Benchmark Results

### Bulk Load - Random Order

Random order insertion simulating RocksDB's bulk load test.

| Keys | Time | Throughput | Operations/sec |
|------|------|------------|----------------|
| 100,000 | 31.11 ms | 355.59 MiB/s | 3,216,844 ops/s |
| 600,000 | 192.53 ms | 344.76 MiB/s | 3,118,997 ops/s |
| 3,000,000 | 735.48 ms | 451.24 MiB/s | 4,079,638 ops/s |

### Bulk Load - Sequential Order

Sequential order insertion for comparison.

| Keys | Time | Throughput | Operations/sec |
|------|------|------------|----------------|
| 100,000 | 31.55 ms | 350.60 MiB/s | 3,171,739 ops/s |
| 600,000 | 190.12 ms | 349.13 MiB/s | 3,157,200 ops/s |
| 3,000,000 | 735.02 ms | 451.52 MiB/s | 4,082,352 ops/s |

## Analysis

### Performance Comparison

- **Sequential vs Random**: Performance is nearly **identical** (~350-450 MiB/s)
- **Random Write Throughput**: Excellent at ~3.1-4.1M ops/s (~345-451 MiB/s)
- **Sequential Write Throughput**: Excellent at ~3.2-4.1M ops/s (~351-452 MiB/s)

### Comparison with RocksDB Benchmarks

RocksDB (same hardware):
- Random order: ~635,000 ops/sec (63-79 MiB/s)
- Sequential order: ~1,880,000 ops/sec (127-208 MiB/s)

This implementation (Naive Logging):
- Random order: ~3,100,000-4,100,000 ops/sec (345-451 MiB/s)
- Sequential order: ~3,200,000-4,100,000 ops/sec (351-452 MiB/s)

**Improvement over RocksDB:**
- Random order: **5-6x faster** (488-645% of RocksDB)
- Sequential order: **2-2.2x faster** (165-217% of RocksDB)

### Scalability

- **Random Order**: Excellent scalability with increasing throughput at larger datasets
- **Sequential Order**: Excellent scalability matching random order performance
- **Both patterns benefit from larger datasets** (100K→3M shows ~30% throughput increase)

### MemTable Flush Impact

- **100K keys (no flush)**: ~31ms, ~355 MiB/s
- **600K keys (1 flush)**: ~192ms, ~345 MiB/s (minimal impact)
- **3M keys (5 flushes)**: ~735ms, ~451 MiB/s (best throughput)

Background flushing has minimal impact on write performance due to asynchronous design.

### Write Stall Mechanism

- Implements RocksDB-style write stall using bounded channels
- `max_write_buffer_number = 2`: allows 1 immutable memtable
- Writes block when immutable memtable limit is reached
- No observable stalls in benchmarks due to efficient background flushing

## Architecture

### Write Path

```
put(key, value)
    ↓
[Mutable MemTable]
    ↓ (size >= threshold)
freeze_memtable()
    ↓
[Immutable MemTable] → Bounded Channel (capacity: max_write_buffer_number - 1)
    ↓
Background Thread
    ↓
BufWriter (8KB buffer)
    ↓
SSTable File (.sst)
    ↓
flush() (no fsync)
```

### Key Design Decisions

1. **BufWriter**: 8KB buffer reduces system calls (~20-27x improvement vs unbuffered)
2. **No fsync**: Prioritizes throughput over durability (data loss on crash)
3. **Bounded Channel**: Prevents unbounded memory growth
4. **Single Background Thread**: Sufficient for current workload
5. **Simple Format**: `[key_len: u32][key][value_len: u32][value]`

## Optimization Impact

### Before Optimization (fsync + unbuffered)
- Random order: ~17 MiB/s
- Sequential order: ~17 MiB/s

### After Optimization (no fsync + BufWriter)
- Random order: ~345-451 MiB/s (**20-27x improvement**)
- Sequential order: ~351-452 MiB/s (**20-27x improvement**)

### Key Optimizations Applied

1. **Removed sync_all()**: Eliminated fsync overhead
2. **Added BufWriter**: Reduced write() system calls
3. **Bounded Channel**: Memory-safe with RocksDB-compatible write stall
4. **Background Flushing**: Non-blocking asynchronous writes

## Trade-offs

### Advantages
- **Very high throughput**: 3-4M ops/s for pure append workloads
- **Minimal write stalls**: Efficient background flushing keeps up with writes
- **Memory bounded**: max_write_buffer_number prevents unbounded growth
- **Simple implementation**: ~200 lines of code vs RocksDB's complexity

### Disadvantages
- **No durability**: Data loss on crash (no WAL, no fsync)
- **No read support**: Write-only implementation
- **No compaction**: SSTable files accumulate without merging
- **Single-threaded flush**: May not scale to very high write rates
- **No compression**: Raw data increases storage requirements

## Notes

### Benchmark Settings

- Warm-up time: 1 second
- Measurement time: 3 seconds
- Sample size: 10 iterations
- Each benchmark run uses a fresh temporary directory
- SSTable files are cleaned up after each iteration

### Implementation Details

- Uses Rust std::sync::mpsc::sync_channel for bounded buffering
- Background thread receives immutable memtables via channel
- BufWriter provides userspace buffering (8KB default)
- SSTable format: simple length-prefixed key-value pairs
- File naming: sequential numbering (000000.sst, 000001.sst, ...)

### Performance Notes

The excellent performance characteristics are due to:
1. **No fsync overhead**: Pure buffered writes to page cache
2. **Efficient buffering**: BufWriter reduces system call overhead
3. **Asynchronous design**: Background flushing doesn't block writers
4. **Simple format**: Minimal serialization overhead
5. **Apple Silicon optimization**: Unified memory and fast integrated SSD

The similar performance for random vs sequential order is because:
1. **Append-only writes**: Both patterns write sequentially to SSTable files
2. **In-memory buffering**: MemTable absorbs random order in memory
3. **No read overhead**: Write-only benchmark doesn't stress random read paths

### Use Cases

This implementation is suitable for:
- **High-throughput logging**: Application logs, metrics, events
- **Temporary data**: Data that can be regenerated on crash
- **Staging writes**: Before final persistence with durability guarantees
- **Learning/Prototyping**: Understanding LSM-tree write path fundamentals

Not suitable for:
- **Durable storage**: Use RocksDB or similar with WAL and fsync
- **Read-heavy workloads**: No index or compaction support
- **Production databases**: Missing many critical database features
