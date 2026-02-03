use std::time::Duration;
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput, BatchSize};
use rocksdb::{DB, Options, WriteOptions};

const KEY_SIZE: usize = 16;
const VALUE_SIZE: usize = 100;
const MEMTABLE_SIZE_THRESHOLD: usize = 64 * 1024 * 1024; // 64 MB

fn generate_key(i: u64) -> Vec<u8> {
    format!("{:016}", i).into_bytes()
}

fn generate_value(size: usize) -> Vec<u8> {
    vec![b'x'; size]
}

fn setup_rocksdb(path: &str) -> DB {
    let mut opts = Options::default();
    opts.create_if_missing(true);

    // MemTable設定
    opts.set_write_buffer_size(MEMTABLE_SIZE_THRESHOLD);
    opts.set_max_write_buffer_number(3);
    opts.set_min_write_buffer_number_to_merge(1);

    // Bulkload最適化設定（benchmark.shに合わせる）
    opts.set_disable_auto_compactions(true);  // 自動コンパクション無効化
    opts.set_allow_concurrent_memtable_write(false);  // 並行書き込み無効化

    // Level 0コンパクショントリガーを極端に高く設定
    opts.set_level_zero_file_num_compaction_trigger(1000000);
    opts.set_level_zero_slowdown_writes_trigger(1000000);
    opts.set_level_zero_stop_writes_trigger(1000000);

    // ディレクトリが存在する場合は削除
    let _ = std::fs::remove_dir_all(path);

    DB::open(&opts, path).expect("Failed to open RocksDB")
}

fn benchmark_bulkload_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulkload_random");

    // ベンチマーク時間を短縮
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

    // RocksDB風のベンチマーク: ランダム順序での大量キー挿入
    // 1エントリ = 116 bytes, 64MB = 約578,524 keys
    // 100k keys: 約11.1 MB (flush無し)
    // 600k keys: 約69.6 MB (flush 1回)
    // 3M keys: 約348 MB (flush 5回)
    for num_keys in [100_000, 600_000, 3_000_000].iter() {
        // スループットを bytes/sec で測定
        let bytes_per_op = (KEY_SIZE + VALUE_SIZE) as u64;
        group.throughput(Throughput::Bytes(*num_keys * bytes_per_op));

        group.bench_with_input(
            BenchmarkId::new("keys", num_keys),
            num_keys,
            |b, &num_keys| {
                let mut iteration = 0;
                b.iter_batched(
                    || {
                        // ランダムな順序でキーを生成
                        let mut keys: Vec<u64> = (0..num_keys).collect();

                        // シンプルなシャッフル（Fisher-Yates）
                        for i in (1..keys.len()).rev() {
                            let j = (i as u64 * 48271 % 2147483647) as usize % (i + 1);
                            keys.swap(i, j);
                        }

                        iteration += 1;
                        let db_path = format!("/tmp/rocksdb_bench_random_{}", iteration);
                        let db = setup_rocksdb(&db_path);
                        (db, db_path, keys, generate_value(VALUE_SIZE))
                    },
                    |(db, db_path, keys, value)| {
                        // WriteOptions: WAL無効化、fsync無効化
                        let mut write_opts = WriteOptions::default();
                        write_opts.disable_wal(true);
                        write_opts.set_sync(false);

                        // 単純なPut操作（batch_size=1相当）
                        for key_num in keys {
                            let key = generate_key(key_num);
                            db.put_opt(&key, &value, &write_opts).expect("Failed to put");
                        }

                        drop(db);
                        let _ = std::fs::remove_dir_all(db_path);
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

fn benchmark_bulkload_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulkload_sequential");

    // ベンチマーク時間を短縮
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

    // 比較用: 連続した順序での挿入
    for num_keys in [100_000, 600_000, 3_000_000].iter() {
        let bytes_per_op = (KEY_SIZE + VALUE_SIZE) as u64;
        group.throughput(Throughput::Bytes(*num_keys * bytes_per_op));

        group.bench_with_input(
            BenchmarkId::new("keys", num_keys),
            num_keys,
            |b, &num_keys| {
                let mut iteration = 0;
                b.iter_batched(
                    || {
                        iteration += 1;
                        let db_path = format!("/tmp/rocksdb_bench_sequential_{}", iteration);
                        let db = setup_rocksdb(&db_path);
                        (db, db_path, generate_value(VALUE_SIZE))
                    },
                    |(db, db_path, value)| {
                        // WriteOptions: WAL無効化、fsync無効化
                        let mut write_opts = WriteOptions::default();
                        write_opts.disable_wal(true);
                        write_opts.set_sync(false);

                        // 単純なPut操作（batch_size=1相当）
                        for i in 0..num_keys {
                            let key = generate_key(i);
                            db.put_opt(&key, &value, &write_opts).expect("Failed to put");
                        }

                        drop(db);
                        let _ = std::fs::remove_dir_all(db_path);
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_bulkload_random,
    benchmark_bulkload_sequential
);
criterion_main!(benches);
