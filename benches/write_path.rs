use std::hint::black_box;
use std::time::Duration;
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput, BatchSize};
use learning_lsm_write_path::WritePath;

const KEY_SIZE: usize = 16;
const VALUE_SIZE: usize = 100;
const MEMTABLE_SIZE_THRESHOLD: usize = 64 * 1024 * 1024; // 64 MB

fn generate_key(i: u64) -> Vec<u8> {
    format!("{:016}", i).into_bytes()
}

fn generate_value(size: usize) -> Vec<u8> {
    vec![b'x'; size]
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
                b.iter_batched(
                    || {
                        // ランダムな順序でキーを生成
                        let mut keys: Vec<u64> = (0..num_keys).collect();

                        // シンプルなシャッフル（Fisher-Yates）
                        for i in (1..keys.len()).rev() {
                            let j = (i as u64 * 48271 % 2147483647) as usize % (i + 1);
                            keys.swap(i, j);
                        }

                        (keys, generate_value(VALUE_SIZE))
                    },
                    |(keys, value)| {
                        let temp_dir = tempfile::tempdir().unwrap();
                        let write_path = WritePath::new(temp_dir.path(), MEMTABLE_SIZE_THRESHOLD).unwrap();

                        for key_num in keys {
                            let key = generate_key(key_num);
                            write_path.put(key, value.clone()).unwrap();
                        }

                        // 最後にフラッシュして全データをディスクに書き出す
                        write_path.flush().unwrap();
                        black_box(write_path);
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
                b.iter_batched(
                    || generate_value(VALUE_SIZE),
                    |value| {
                        let temp_dir = tempfile::tempdir().unwrap();
                        let write_path = WritePath::new(temp_dir.path(), MEMTABLE_SIZE_THRESHOLD).unwrap();

                        for i in 0..num_keys {
                            let key = generate_key(i);
                            write_path.put(key, value.clone()).unwrap();
                        }

                        // 最後にフラッシュして全データをディスクに書き出す
                        write_path.flush().unwrap();
                        black_box(write_path);
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
