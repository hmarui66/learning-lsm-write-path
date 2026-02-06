# SkipList-based MemTable Benchmark Results

## Environment

- **Implementation**: Custom Rust logging system with SkipList-based MemTable
- **SkipList Library**: crossbeam-skiplist 0.1 (lock-free concurrent skiplist)
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
- **MemTable**: SkipMap (sorted, lock-free concurrent data structure)

## Test Parameters

| Keys | Total Size | Expected Flushes |
|------|------------|------------------|
| 100,000 | ~11.1 MB | 0 |
| 600,000 | ~69.6 MB | 1 |
| 3,000,000 | ~348 MB | ~5 |

## Benchmark Results

### Bulk Load - Random Order

Random order insertion with SkipList maintaining sorted order.

| Keys | Time | Throughput | Operations/sec |
|------|------|------------|----------------|
| 100,000 | 111.74 ms | 99 MiB/s | 895,000 ops/s |
| 600,000 | 1.24 s | 53 MiB/s | 484,000 ops/s |
| 3,000,000 | 4.88 s | 68 MiB/s | 615,000 ops/s |

### Bulk Load - Sequential Order

Sequential order insertion (best case for SkipList).

| Keys | Time | Throughput | Operations/sec |
|------|------|------------|----------------|
| 100,000 | 62.96 ms | 176 MiB/s | 1,588,000 ops/s |
| 600,000 | 400.27 ms | 166 MiB/s | 1,499,000 ops/s |
| 3,000,000 | 1.53 s | 217 MiB/s | 1,961,000 ops/s |

## Analysis

### Performance Comparison

- **Sequential vs Random**: Sequential writes are **1.8-3.2x faster** than random writes
- **Random Write Throughput**: 53-99 MiB/s (484-895K ops/s)
- **Sequential Write Throughput**: 166-217 MiB/s (1.5-2.0M ops/s)

### Comparison with Vec-based Implementation

Vec-based (same hardware):
- Random order: ~342 MiB/s (3.1M ops/s)
- Sequential order: ~350 MiB/s (3.2M ops/s)

SkipList-based:
- Random order: ~68 MiB/s (615K ops/s)
- Sequential order: ~166 MiB/s (1.5M ops/s)

**Performance vs Vec-based:**
- Random order: **5.0x slower** (20% of Vec performance)
- Sequential order: **2.1x slower** (47% of Vec performance)

### Comparison with Naive Logging (Vec-based)

Naive Logging (Vec):
- Random order: 3.1-4.1M ops/s (345-451 MiB/s)
- Sequential order: 3.2-4.1M ops/s (351-452 MiB/s)

SkipList Implementation:
- Random order: 0.5-0.9M ops/s (53-99 MiB/s)
- Sequential order: 1.5-2.0M ops/s (166-217 MiB/s)

**Degradation from Naive Logging:**
- Random order: **6.6x slower** (15% of Vec throughput)
- Sequential order: **2.1x slower** (48% of Vec throughput)

### Scalability

- **Random Order**: Poor scalability with decreasing per-operation throughput as dataset grows
- **Sequential Order**: Good scalability with consistent throughput
- **Large datasets benefit sequential writes** due to reduced tree rebalancing

### MemTable Flush Impact

- **100K keys (no flush)**: SkipList overhead most visible in small datasets
- **600K keys (1 flush)**: Random writes suffer most (~53 MiB/s)
- **3M keys (5 flushes)**: Sequential writes show best relative performance

Background flushing has minimal additional impact beyond SkipList insertion overhead.

## Architecture

### Write Path

```
put(key, value)
    ↓
[SkipMap MemTable] ← O(log n) insertion, maintains sorted order
    ↓ (size >= threshold)
freeze_memtable()
    ↓
[Immutable SkipMap] → Bounded Channel (capacity: max_write_buffer_number - 1)
    ↓
Background Thread
    ↓
Iterate SkipMap in sorted order
    ↓
BufWriter (8KB buffer)
    ↓
SSTable File (.sst) ← Already sorted!
    ↓
flush() (no fsync)
```

### Key Design Decisions

1. **crossbeam-skiplist**: Lock-free concurrent skiplist for thread-safety
2. **No fsync**: Prioritizes throughput over durability (data loss on crash)
3. **Bounded Channel**: Prevents unbounded memory growth
4. **Single Background Thread**: Sufficient for current workload
5. **Sorted Output**: SSTable files are naturally sorted without additional work
6. **Simple Format**: `[key_len: u32][key][value_len: u32][value]`

## Performance Analysis

### Why SkipList is Slower

**Insertion Complexity:**
- Vec: O(1) append operation
- SkipList: O(log n) search + insertion

**Memory Access Pattern:**
- Vec: Sequential, cache-friendly
- SkipList: Pointer chasing, cache-unfriendly

**Allocation Overhead:**
- Vec: Amortized allocations (grows by 2x)
- SkipList: Per-node allocation

**Lock-Free Overhead:**
- Vec: Simple memory write
- SkipList: Atomic operations for thread-safety

### Why Sequential is Faster than Random

Sequential insertion in SkipList:
- New keys always go to the end of the skiplist
- Fewer pointer updates during traversal
- Better cache locality for consecutive inserts
- Reduced tree rebalancing operations

Random insertion:
- Keys scattered throughout the structure
- More pointer traversal
- Worse cache behavior
- More tree rebalancing

### Detailed Performance Breakdown (3M keys)

| Metric | Vec (Random) | SkipList (Random) | Vec (Seq) | SkipList (Seq) |
|--------|--------------|-------------------|-----------|----------------|
| Time | 735 ms | 4.88 s | 743 ms | 1.53 s |
| Throughput | 451 MiB/s | 68 MiB/s | 447 MiB/s | 217 MiB/s |
| Ops/sec | 4.08M | 615K | 4.04M | 1.96M |
| Relative | 1.0x | 0.15x | 1.0x | 0.49x |

## Trade-offs

### Advantages
- **Sorted output**: SSTable files are already sorted
- **No sorting overhead**: Eliminates post-processing step
- **Range queries**: Supports efficient range scans (if implemented)
- **Concurrent reads**: Lock-free structure allows concurrent access
- **Natural LSM-tree fit**: Matches typical LSM-tree MemTable design

### Disadvantages
- **5-7x slower writes**: Compared to Vec for random inserts
- **2x slower writes**: Compared to Vec for sequential inserts
- **Higher memory overhead**: Pointer overhead per entry
- **Complex implementation**: More code complexity than Vec
- **Cache unfriendly**: Poor memory locality vs sequential Vec
- **Atomic overhead**: Lock-free operations have cost even in single-threaded use

## Use Cases

### When to Use SkipList MemTable

**Good for:**
- LSM-tree databases with read workloads
- Systems requiring range queries
- Multi-threaded concurrent writes
- Applications where sorted SSTable output is critical
- Production LSM-tree implementations (RocksDB, LevelDB style)

**Not suitable for:**
- Write-only workloads (use Vec)
- Single-threaded append-only logs (use Vec)
- Maximum write throughput scenarios
- Simple learning/benchmarking projects
- Applications without read requirements

### When to Use Vec MemTable

**Good for:**
- Pure write/logging workloads
- Maximum write throughput
- Simple implementations
- Single-threaded scenarios
- Learning LSM-tree write path fundamentals
- Write-optimized benchmarks

**Trade-off:** Requires sorting before SSTable flush

## Notes

### Benchmark Settings

- Warm-up time: 1 second
- Measurement time: 3 seconds
- Sample size: 10 iterations
- Each benchmark run uses a fresh temporary directory
- SSTable files are cleaned up after each iteration

### Implementation Details

- Uses `crossbeam_skiplist::SkipMap` for concurrent lock-free skiplist
- Background thread receives immutable memtables via channel
- BufWriter provides userspace buffering (8KB default)
- SSTable format: simple length-prefixed key-value pairs
- File naming: sequential numbering (000000.sst, 000001.sst, ...)
- Sorted iteration via SkipMap's natural ordering

### Performance Notes

The performance characteristics are due to:
1. **O(log n) insertion**: Fundamental SkipList complexity
2. **Pointer-based structure**: Non-sequential memory access
3. **Atomic operations**: Lock-free coordination overhead
4. **Per-node allocation**: More allocator pressure than Vec
5. **Cache misses**: Poor spatial locality vs sequential Vec

The better sequential performance is because:
1. **Append-like insertion**: Sequential keys go to end of skiplist
2. **Reduced traversal**: Less pointer following for consecutive keys
3. **Better prediction**: CPU can predict access patterns
4. **Fewer updates**: Less internal structure rebalancing

### Comparison Summary

| Feature | Vec-based | SkipList-based |
|---------|-----------|----------------|
| Write Speed (Random) | 342-451 MiB/s | 53-99 MiB/s |
| Write Speed (Sequential) | 347-452 MiB/s | 166-217 MiB/s |
| Memory Overhead | Low | Medium-High |
| Code Complexity | Simple | Complex |
| Sorted Output | ❌ No | ✅ Yes |
| Range Queries | ❌ No | ✅ Yes |
| Cache Efficiency | ✅ High | ❌ Low |
| Thread Safety | ❌ No | ✅ Yes |
| Best Use Case | Write-only logs | LSM-tree databases |

## Conclusion

For pure write throughput in LSM-tree learning projects, **Vec-based MemTable is superior**:
- 5-7x faster random writes
- 2x faster sequential writes
- Simpler implementation
- Better cache performance

For production LSM-tree databases, **SkipList-based MemTable is appropriate**:
- Sorted output eliminates sorting overhead during flush
- Supports range queries and scans
- Thread-safe concurrent access
- Standard LSM-tree design pattern

The performance trade-off is acceptable in production databases because:
1. **Read performance matters**: SkipList enables efficient range scans
2. **Write amplification**: Sorting overhead eliminated
3. **Compaction**: Sorted SSTables merge more efficiently
4. **Real-world workloads**: Mixed read/write patterns benefit from sorted structure

For this write-only benchmark project, Vec-based implementation achieves the goal of understanding LSM-tree write path with maximum performance.

## Raw Data Structure Performance

To isolate the performance characteristics of the underlying data structures, we benchmarked pure SkipMap and Vec operations without any I/O, Mutex, or channel overhead.

### Pure SkipMap Performance (crossbeam-skiplist)

Direct insertion into SkipMap without MemTable wrapper.

| Keys | Random Insert | Sequential Insert |
|------|---------------|-------------------|
| 100,000 | 143 MiB/s (77ms) | 322 MiB/s (34ms) |
| 600,000 | 63 MiB/s (1.05s) | 298 MiB/s (223ms) |
| 3,000,000 | 42 MiB/s (7.8s) | 265 MiB/s (1.25s) |

### Pure Vec Performance (std::vec::Vec)

Direct append to Vec without MemTable wrapper.

| Keys | Random Append | Sequential Append |
|------|---------------|-------------------|
| 100,000 | 579 MiB/s (19ms) | 584 MiB/s (19ms) |
| 600,000 | 541 MiB/s (123ms) | 540 MiB/s (123ms) |
| 3,000,000 | 483 MiB/s (687ms) | 485 MiB/s (685ms) |

### Performance Ratio: Vec vs SkipMap

| Dataset | Random Insert | Sequential Insert |
|---------|---------------|-------------------|
| 100,000 | **4.0x faster** | **1.8x faster** |
| 600,000 | **8.6x faster** | **1.8x faster** |
| 3,000,000 | **11.5x faster** | **1.8x faster** |

## Bottleneck Analysis

### MemTable Implementation Overhead

Comparing pure data structure performance with full MemTable implementation:

**Vec-based:**
- Pure Vec: 483-584 MiB/s
- MemTable (with Mutex + I/O): 342-451 MiB/s
- **Overhead**: 20-30% (Mutex locking and file I/O)

**SkipMap-based:**
- Pure SkipMap: 42-322 MiB/s
- MemTable (with Mutex + I/O): 53-217 MiB/s
- **Overhead**: Minimal to none (SkipMap insertion dominates)

### Key Findings

1. **Vec append is extremely fast**: 480-580 MiB/s regardless of insertion order
   - Random order: No performance penalty (append is always O(1))
   - Sequential order: Same performance (order doesn't matter for append)

2. **SkipMap is inherently slower**: 42-322 MiB/s depending on access pattern
   - Random order: **11.5x slower** than Vec (worst case)
   - Sequential order: **1.8x slower** than Vec (best case)
   - Performance degrades with dataset size (O(log n) complexity)

3. **SkipMap insertion is the bottleneck**:
   - For Vec: Mutex + I/O adds 20-30% overhead
   - For SkipMap: Data structure itself is 80-90% of the cost

4. **Sequential insertion helps SkipMap**:
   - Random: 42 MiB/s (3M keys)
   - Sequential: 265 MiB/s (3M keys)
   - **6.3x faster** when keys are ordered

### Why SkipMap Sequential is Faster

Sequential key insertion optimizes SkipMap operations:
1. **Append-like behavior**: New keys always go to the end
2. **Reduced traversal**: Fewer pointer hops to find insertion point
3. **Cache locality**: Consecutive accesses improve cache hit rate
4. **Reduced rebalancing**: Less internal structure updates

### Why SkipMap Random Degrades with Size

Random insertion worst-case behavior:
1. **Full tree traversal**: Average O(log n) pointer hops
2. **Cache misses**: Random access pattern defeats CPU cache
3. **Allocation overhead**: Per-node allocations scattered in memory
4. **Atomic operations**: Lock-free coordination on every update

## Conclusion: Data Structure Selection

### For Write-Only Workloads

**Use Vec:**
- 4-12x faster than SkipMap
- Predictable O(1) performance
- Minimal memory overhead
- Simple implementation

**Cost:** Must sort before creating SSTable (one-time O(n log n))

### For Read-Write Workloads

**Use SkipMap:**
- Maintains sorted order automatically
- Supports range queries
- Thread-safe concurrent access
- Standard LSM-tree pattern

**Cost:** 2-12x slower writes depending on access pattern

### Performance Summary Table

| Metric | Vec | SkipMap (Seq) | SkipMap (Rand) |
|--------|-----|---------------|----------------|
| 3M keys throughput | 485 MiB/s | 265 MiB/s | 42 MiB/s |
| 3M keys time | 685 ms | 1.25 s | 7.8 s |
| Complexity | O(1) | O(log n) | O(log n) |
| Memory locality | Excellent | Poor | Poor |
| Cache efficiency | High | Low | Low |
| Sorting required | Yes | No | No |
| Range queries | No | Yes | Yes |

### Raw SkipMap Performance Characteristics

The benchmarks reveal fundamental SkipMap behavior:

1. **O(log n) complexity visible**: 100K→3M shows 6.3x slowdown (random)
2. **Sequential optimization**: 6.3x faster than random (265 vs 42 MiB/s)
3. **Still slower than Vec**: Even best case is 1.8x slower
4. **Concurrent data structure overhead**: Atomic operations cost even in single-threaded use

### Recommendation

For this learning project focused on LSM-tree write path:
- **Vec is the clear winner** for maximum write throughput
- SkipMap would be appropriate for a full LSM-tree implementation with reads
- The 4-12x performance difference justifies the one-time sorting cost

The raw performance data confirms that **crossbeam-skiplist itself is the bottleneck**, not the surrounding infrastructure (Mutex, channels, I/O). This validates the architectural decision to use Vec for write-optimized workloads.
