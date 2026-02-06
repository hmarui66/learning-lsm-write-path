use std::time::Duration;
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput, BatchSize};
use rocksdb::{DB, Options, WriteOptions};

const KEY_SIZE: usize = 16;
const VALUE_SIZE: usize = 100;
const MEMTABLE_SIZE_THRESHOLD: usize = 64 * 1024 * 1024; // 64 MB
const TEST_KEYS: u64 = 100_000; // 小さめのデータセットで素早くテスト

fn generate_key(i: u64) -> Vec<u8> {
    format!("{:016}", i).into_bytes()
}

fn generate_value(size: usize) -> Vec<u8> {
    vec![b'x'; size]
}

fn setup_rocksdb_with_params(
    path: &str,
    _disable_wal: bool,
    _sync: bool,
    disable_auto_compactions: bool,
    allow_concurrent_memtable_write: bool,
) -> DB {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.set_write_buffer_size(MEMTABLE_SIZE_THRESHOLD);
    opts.set_max_write_buffer_number(3);
    opts.set_min_write_buffer_number_to_merge(1);
    opts.set_disable_auto_compactions(disable_auto_compactions);
    opts.set_allow_concurrent_memtable_write(allow_concurrent_memtable_write);
    opts.set_level_zero_file_num_compaction_trigger(1000000);
    opts.set_level_zero_slowdown_writes_trigger(1000000);
    opts.set_level_zero_stop_writes_trigger(1000000);

    let _ = std::fs::remove_dir_all(path);
    DB::open(&opts, path).expect("Failed to open RocksDB")
}

fn setup_rocksdb_manual_wal_flush(
    path: &str,
    manual_wal_flush: bool,
) -> DB {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.set_write_buffer_size(MEMTABLE_SIZE_THRESHOLD);
    opts.set_max_write_buffer_number(3);
    opts.set_min_write_buffer_number_to_merge(1);
    opts.set_disable_auto_compactions(true);
    opts.set_manual_wal_flush(manual_wal_flush);
    opts.set_allow_concurrent_memtable_write(false);
    opts.set_level_zero_file_num_compaction_trigger(1000000);
    opts.set_level_zero_slowdown_writes_trigger(1000000);
    opts.set_level_zero_stop_writes_trigger(1000000);

    let _ = std::fs::remove_dir_all(path);
    DB::open(&opts, path).expect("Failed to open RocksDB")
}

fn benchmark_wal_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("param_wal");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

    let bytes_per_op = (KEY_SIZE + VALUE_SIZE) as u64;
    group.throughput(Throughput::Bytes(TEST_KEYS * bytes_per_op));

    for &disable_wal in [false, true].iter() {
        group.bench_with_input(
            BenchmarkId::new("disable_wal", disable_wal),
            &disable_wal,
            |b, &disable_wal| {
                let mut iteration = 0;
                b.iter_batched(
                    || {
                        iteration += 1;
                        let db_path = format!("/tmp/rocksdb_wal_test_{}", iteration);
                        let db = setup_rocksdb_with_params(&db_path, disable_wal, false, true, false);
                        (db, db_path, generate_value(VALUE_SIZE))
                    },
                    |(db, db_path, value)| {
                        let mut write_opts = WriteOptions::default();
                        write_opts.disable_wal(disable_wal);
                        write_opts.set_sync(false);

                        for i in 0..TEST_KEYS {
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

fn benchmark_sync_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("param_sync");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

    let bytes_per_op = (KEY_SIZE + VALUE_SIZE) as u64;
    group.throughput(Throughput::Bytes(TEST_KEYS * bytes_per_op));

    for &sync in [false, true].iter() {
        group.bench_with_input(
            BenchmarkId::new("sync", sync),
            &sync,
            |b, &sync| {
                let mut iteration = 0;
                b.iter_batched(
                    || {
                        iteration += 1;
                        let db_path = format!("/tmp/rocksdb_sync_test_{}", iteration);
                        // syncを有効にする場合はWALも有効にする必要がある
                        let db = setup_rocksdb_with_params(&db_path, !sync, sync, true, false);
                        (db, db_path, generate_value(VALUE_SIZE))
                    },
                    |(db, db_path, value)| {
                        let mut write_opts = WriteOptions::default();
                        write_opts.disable_wal(!sync);
                        write_opts.set_sync(sync);

                        for i in 0..TEST_KEYS {
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

fn benchmark_auto_compaction_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("param_auto_compaction");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

    let bytes_per_op = (KEY_SIZE + VALUE_SIZE) as u64;
    group.throughput(Throughput::Bytes(TEST_KEYS * bytes_per_op));

    for &disable_auto_compactions in [false, true].iter() {
        group.bench_with_input(
            BenchmarkId::new("disable_auto_compactions", disable_auto_compactions),
            &disable_auto_compactions,
            |b, &disable_auto_compactions| {
                let mut iteration = 0;
                b.iter_batched(
                    || {
                        iteration += 1;
                        let db_path = format!("/tmp/rocksdb_compaction_test_{}", iteration);
                        let db = setup_rocksdb_with_params(&db_path, true, false, disable_auto_compactions, false);
                        (db, db_path, generate_value(VALUE_SIZE))
                    },
                    |(db, db_path, value)| {
                        let mut write_opts = WriteOptions::default();
                        write_opts.disable_wal(true);
                        write_opts.set_sync(false);

                        for i in 0..TEST_KEYS {
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

fn benchmark_concurrent_memtable_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("param_concurrent_memtable");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

    let bytes_per_op = (KEY_SIZE + VALUE_SIZE) as u64;
    group.throughput(Throughput::Bytes(TEST_KEYS * bytes_per_op));

    for &allow_concurrent in [false, true].iter() {
        group.bench_with_input(
            BenchmarkId::new("allow_concurrent_memtable_write", allow_concurrent),
            &allow_concurrent,
            |b, &allow_concurrent| {
                let mut iteration = 0;
                b.iter_batched(
                    || {
                        iteration += 1;
                        let db_path = format!("/tmp/rocksdb_concurrent_test_{}", iteration);
                        let db = setup_rocksdb_with_params(&db_path, true, false, true, allow_concurrent);
                        (db, db_path, generate_value(VALUE_SIZE))
                    },
                    |(db, db_path, value)| {
                        let mut write_opts = WriteOptions::default();
                        write_opts.disable_wal(true);
                        write_opts.set_sync(false);

                        for i in 0..TEST_KEYS {
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

fn benchmark_manual_wal_flush_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("param_manual_wal_flush");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

    let bytes_per_op = (KEY_SIZE + VALUE_SIZE) as u64;
    group.throughput(Throughput::Bytes(TEST_KEYS * bytes_per_op));

    for &manual_wal_flush in [false, true].iter() {
        group.bench_with_input(
            BenchmarkId::new("manual_wal_flush", manual_wal_flush),
            &manual_wal_flush,
            |b, &manual_wal_flush| {
                let mut iteration = 0;
                b.iter_batched(
                    || {
                        iteration += 1;
                        let db_path = format!("/tmp/rocksdb_manual_wal_flush_test_{}", iteration);
                        let db = setup_rocksdb_manual_wal_flush(&db_path, manual_wal_flush);
                        (db, db_path, generate_value(VALUE_SIZE))
                    },
                    |(db, db_path, value)| {
                        let mut write_opts = WriteOptions::default();
                        write_opts.disable_wal(false);  // WAL有効にして影響を確認
                        write_opts.set_sync(false);

                        for i in 0..TEST_KEYS {
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
    benchmark_wal_impact,
    benchmark_auto_compaction_impact,
    benchmark_concurrent_memtable_impact,
    benchmark_manual_wal_flush_impact
);
criterion_main!(benches);
