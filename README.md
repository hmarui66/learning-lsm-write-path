# LSM-Tree Write Path 性能学習プロジェクト

LSM-TreeのWrite Path性能を測定するベンチマークプロジェクト。RocksDBを使用して各種パラメータが書き込み性能に与える影響を調査する。

## 概要

- 目的: LSM-Treeの書き込み性能特性の理解
- ベースライン: RocksDB 10.7.5_1 (Rustクレート: rocksdb 0.24.0)
- ベンチマークツール: Criterion 0.8.1
- プラットフォーム: Apple Silicon (aarch64-apple-darwin)

## セットアップ

### 依存ライブラリのインストール

macOS:
```bash
brew install rocksdb thrift snappy gflags lz4 zstd
```

Ubuntu/Debian:
```bash
sudo apt-get install librocksdb-dev libsnappy-dev libgflags-dev liblz4-dev libzstd-dev
```

Fedora/RHEL:
```bash
sudo dnf install rocksdb-devel snappy-devel gflags-devel lz4-devel libzstd-devel
```

### ビルド

```bash
cargo build --release
cargo bench --no-run
```

## ベンチマーク結果

### [RocksDBベンチマーク結果](ROCKSDB.md)

RocksDB公式ベンチマーク(benchmark.sh bulkload)を参考にした性能測定。

主要な結果:
- ランダム順序書き込み: ~635,000 ops/s (70 MiB/s)
- 連続順序書き込み: ~1,880,000 ops/s (208 MiB/s)
- 最適化による改善: デフォルト設定比で3-12倍

設定:
- WAL無効化
- fsync無効化
- 自動コンパクション無効化
- Level 0トリガーを極端に高く設定

### [パラメータ影響分析](PARAMETER_IMPACT.md)

各種パラメータの書き込み性能への影響を定量測定。

性能への影響（重要度順）:

1. sync (fsync): ~5000倍
   - sync=false: ~80 ms
   - sync=true: ~411秒

2. disable_wal (WAL): 4.4倍
   - WAL有効: 352.67 ms (31.37 MiB/s)
   - WAL無効: 80.02 ms (138.25 MiB/s)

3. manual_wal_flush: 3.8倍
   - 自動フラッシュ: 256.09 ms (43.20 MiB/s)
   - 手動フラッシュ: 67.95 ms (162.80 MiB/s)

4. disable_auto_compactions: ~2%
5. allow_concurrent_memtable_write: ~1%

結論:
- 性能の99%はWALとfsyncで決まる
- 最適化の本質はI/O回避
- manual_wal_flush=trueは耐久性と性能のバランスが良い

### [pwriteベースライン測定](PWRITE.md)

pwriteシステムコールの性能測定とRocksDBとの比較。

pwrite性能:
- 通常のpwrite: 33.75 MiB/s (305,078 ops/s)
- O_SYNC付き: 2.15 MiB/s (15.7倍遅い)
- fsync 1回(最後のみ): 30.41 MiB/s

RocksDB vs pwrite:

| 実装 | スループット | pwrite比 |
|------|------------|---------|
| pwrite | 33.75 MiB/s | 1.0x |
| RocksDB (WAL有効) | 24-29 MiB/s | 0.7-0.9x |
| RocksDB (WAL無効) | 79-127 MiB/s | 2.3-3.8x |
| RocksDB (manual_wal_flush=true) | 162.80 MiB/s | 4.8x |

## ベンチマーク構成

### データセット

- キーサイズ: 16 bytes
- 値サイズ: 100 bytes
- エントリサイズ: 116 bytes
- MemTableサイズ閾値: 64 MB

### テストケース

| キー数 | 総データサイズ | MemTable flush回数 |
|--------|---------------|-------------------|
| 100,000 | ~11.1 MB | 0 |
| 600,000 | ~69.6 MB | 1 |
| 3,000,000 | ~348 MB | ~5 |

### ベンチマーク種類

- bulkload_random: ランダム順序一括書き込み
- bulkload_sequential: 連続順序一括書き込み
- param_test: パラメータ影響分析
- pwrite_baseline: pwriteベースライン測定

## 使用方法

```bash
# 全ベンチマーク実行
cargo bench

# 特定のベンチマーク実行
cargo bench --bench write_path
cargo bench --bench param_test
cargo bench --bench pwrite_baseline

# HTMLレポート確認
open target/criterion/report/index.html
```

## 分析結果

### I/O操作の影響

書き込み性能はディスクI/O操作で決定される:

- sync有効: 5000倍の減速
- WAL有効: 4.4倍の減速
- manual_wal_flush無効: 3.8倍の減速

### メモリファースト設計

RocksDBのMemTableへの書き込みは単純なファイル書き込みより2-4倍高速。理由:

- メモリ内データ構造(skiplist/AVL tree)
- 非同期ディスク書き込み
- バッチ処理

### 耐久性と性能のトレードオフ

| 設定 | 性能 | 耐久性 |
|------|------|--------|
| disable_wal=true | 4.4倍高速 | クラッシュ時データ損失 |
| manual_wal_flush=true | 3.8倍高速 | クラッシュリカバリ可能 |
| sync=true | 5000倍遅い | 各書き込み永続化 |

### Bulk Load最適化

benchmark.sh bulkloadの最適化:

- WAL無効化: ディスク書き込み削減
- fsync無効化: ディスク同期回避
- auto compaction無効化: バックグラウンド処理回避

他のパラメータ(concurrent memtable write等)の影響は誤差レベル。

## 参考資料

- [RocksDB Performance Benchmarks](https://github.com/facebook/rocksdb/wiki/Performance-Benchmarks)
- [RocksDB benchmark.sh](https://github.com/facebook/rocksdb/blob/main/tools/benchmark.sh)
- [LSM-Tree論文](https://www.cs.umb.edu/~poneil/lsmtree.pdf)
