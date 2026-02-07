# LSM-Tree Write Path Performance Study

LSM-Treeの書き込みパスにおける性能検証プロジェクト。

## 概要

LSM-Treeの書き込み処理における各種実装方式の性能を測定・比較する。RocksDBと比較して、シンプルなログ出力実装の性能特性を検証する。

## ベンチマーク結果

### 1. Naive Logging Implementation

[NAIVE_LOGGING.md](./NAIVE_LOGGING.md) - Vecベースのログ出力実装

メモリバッファ + バックグラウンドフラッシュによる性能検証。

- 書き込み性能: 3.1-4.1M ops/s（345-451 MiB/s）
- RocksDBとの比較: ランダム書き込みで5-6倍高速
- 実装: Vec-based MemTable + BufWriter（fsyncなし）

実装の特徴:
- メモリバッファにログエントリを追加（O(1)）
- バッファが64MBに達したらimmutable化
- バックグラウンドスレッドでファイルに書き出し
- RocksDB風のwrite stall機構（max_write_buffer_number）

最適化の効果:
- fsync削除 + BufWriter追加で20-27倍の高速化

### 2. SkipList-based Implementation

[SKIPLIST.md](./SKIPLIST.md) - crossbeam-skiplistを使用した実装

LSM-Tree標準のSkipList-based MemTableによる性能検証。

- 書き込み性能（Random）: 480-862K ops/s（53-95 MiB/s）
- 書き込み性能（Sequential）: 1.6-2.0M ops/s（170-221 MiB/s）
- Vec版との比較: 5-6倍遅い（Random）、1.6-2倍遅い（Sequential）

純粋なデータ構造の性能:
- Vec append: 483-584 MiB/s（順序に関係なく一定）
- SkipMap insert（Random）: 42-143 MiB/s
- SkipMap insert（Sequential）: 265-322 MiB/s

ボトルネック分析:
- Vec版: Mutex + I/Oが20-30%のオーバーヘッド
- SkipList版: SkipMap自体が80-90%のコスト

利点と欠点:
- ソート済み出力（SSTable作成が効率的）
- 範囲クエリ対応
- スレッドセーフ
- 書き込み性能は5-6倍遅い（Random）、1.6-2倍遅い（Sequential）
- メモリ局所性が悪い

### 3. RocksDB Performance Baseline

[rocksdb ブランチ](https://github.com/hmarui66/learning-lsm-write-path/tree/rocksdb) - RocksDB 10.7.5の性能検証

RocksDBの性能をベースラインとして測定。

- ランダム書き込み: 約635,000 ops/s（70 MiB/s）
- シーケンシャル書き込み: 約1,880,000 ops/s（208 MiB/s）

詳細は[rocksdbブランチのROCKSDB.md](https://github.com/hmarui66/learning-lsm-write-path/blob/rocksdb/ROCKSDB.md)を参照。

## 性能比較

| 実装 | ランダム書き込み | シーケンシャル書き込み | 特徴 |
|------|-----------------|---------------------|------|
| Naive Logging (Vec) | 4.1M ops/s | 4.1M ops/s | 最速、シンプル |
| SkipList | 0.6M ops/s | 2.0M ops/s | ソート済み、範囲クエリ対応 |
| RocksDB | 0.6M ops/s | 1.9M ops/s | プロダクション品質 |

注: SkipList実装では書き込み順序により性能が変動する（ランダム: 0.5-0.9M ops/s、シーケンシャル: 1.6-2.0M ops/s）。

## アーキテクチャ

### Naive Logging (Vec-based)

```
put(key, value)
    ↓
[Mutable MemTable (Vec)] ← O(1) append
    ↓ (size >= 64MB)
freeze_memtable()
    ↓
[Immutable MemTable] → Bounded Channel
    ↓
Background Thread
    ↓
BufWriter (8KB)
    ↓
SSTable File (.sst)
```

### SkipList-based

```
put(key, value)
    ↓
[Mutable MemTable (SkipMap)] ← O(log n) insert, sorted
    ↓ (size >= 64MB)
freeze_memtable()
    ↓
[Immutable SkipMap] → Bounded Channel
    ↓
Background Thread
    ↓
Iterate in sorted order
    ↓
BufWriter (8KB)
    ↓
SSTable File (.sst)
```

## 環境

- Rust: 1.93.0
- Platform: macOS (aarch64-apple-darwin)
- Benchmark Tool: Criterion 0.8.2
- SkipList Library: crossbeam-skiplist 0.1.3

## ベンチマーク設定

- Key Size: 16 bytes
- Value Size: 100 bytes
- MemTable Threshold: 64 MB
- max_write_buffer_number: 2（RocksDB互換）
- BufWriter: 8KB
- fsync: なし（最大スループット優先）

## ベンチマーク実行方法

```bash
# Vec-based implementation
cargo bench --bench write_path

# SkipList-based implementation
cargo bench --bench write_path_skiplist

# 純粋なデータ構造比較
cargo bench --bench skiplist_raw

# すべてのベンチマーク
cargo bench
```

## プロジェクト構成

```
.
├── src/
│   ├── lib.rs                     # ライブラリエントリポイント
│   ├── write_path.rs              # Vec-based実装
│   └── write_path_skiplist.rs     # SkipList-based実装
├── benches/
│   ├── write_path.rs              # Vec-based ベンチマーク
│   ├── write_path_skiplist.rs     # SkipList-based ベンチマーク
│   └── skiplist_raw.rs            # 純粋なデータ構造ベンチマーク
├── NAIVE_LOGGING.md               # Vec-based 詳細結果
├── SKIPLIST.md                    # SkipList-based 詳細結果
└── README.md                      # このファイル
```

## 参考資料

- [RocksDB公式ドキュメント](https://github.com/facebook/rocksdb/wiki)
- [crossbeam-skiplist](https://docs.rs/crossbeam-skiplist/)
