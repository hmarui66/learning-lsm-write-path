use std::hint::black_box;
use std::time::Duration;
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput, BatchSize};
use crossbeam_skiplist::SkipMap;

const KEY_SIZE: usize = 16;
const VALUE_SIZE: usize = 100;

fn generate_key(i: u64) -> Vec<u8> {
    format!("{:016}", i).into_bytes()
}

fn generate_value(size: usize) -> Vec<u8> {
    vec![b'x'; size]
}

fn benchmark_skipmap_insert_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("skipmap_raw_insert_random");

    // ベンチマーク時間を短縮
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

    // 純粋なSkipMap挿入性能を測定
    for num_keys in [100_000, 600_000, 3_000_000].iter() {
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
                        let skipmap = SkipMap::new();

                        for key_num in keys {
                            let key = generate_key(key_num);
                            skipmap.insert(key, value.clone());
                        }

                        black_box(skipmap);
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

fn benchmark_skipmap_insert_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("skipmap_raw_insert_sequential");

    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

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
                        let skipmap = SkipMap::new();

                        for i in 0..num_keys {
                            let key = generate_key(i);
                            skipmap.insert(key, value.clone());
                        }

                        black_box(skipmap);
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

fn benchmark_vec_append_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("vec_raw_append_random");

    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

    // 比較用: Vecへの単純なappend
    for num_keys in [100_000, 600_000, 3_000_000].iter() {
        let bytes_per_op = (KEY_SIZE + VALUE_SIZE) as u64;
        group.throughput(Throughput::Bytes(*num_keys * bytes_per_op));

        group.bench_with_input(
            BenchmarkId::new("keys", num_keys),
            num_keys,
            |b, &num_keys| {
                b.iter_batched(
                    || {
                        let mut keys: Vec<u64> = (0..num_keys).collect();
                        for i in (1..keys.len()).rev() {
                            let j = (i as u64 * 48271 % 2147483647) as usize % (i + 1);
                            keys.swap(i, j);
                        }
                        (keys, generate_value(VALUE_SIZE))
                    },
                    |(keys, value)| {
                        let mut vec = Vec::new();

                        for key_num in keys {
                            let key = generate_key(key_num);
                            vec.push((key, value.clone()));
                        }

                        black_box(vec);
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

fn benchmark_vec_append_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("vec_raw_append_sequential");

    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

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
                        let mut vec = Vec::new();

                        for i in 0..num_keys {
                            let key = generate_key(i);
                            vec.push((key, value.clone()));
                        }

                        black_box(vec);
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
    benchmark_skipmap_insert_random,
    benchmark_skipmap_insert_sequential,
    benchmark_vec_append_random,
    benchmark_vec_append_sequential
);
criterion_main!(benches);
