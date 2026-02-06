use std::time::Duration;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput, BatchSize};

const KEY_SIZE: usize = 16;
const VALUE_SIZE: usize = 100;

fn generate_key(i: u64) -> Vec<u8> {
    format!("{:016}", i).into_bytes()
}

fn generate_value(size: usize) -> Vec<u8> {
    vec![b'x'; size]
}

fn pwrite_sequential(file: &File, key: &[u8], value: &[u8], offset: i64) -> std::io::Result<()> {
    use std::os::unix::prelude::FileExt;

    // キーを書き込み
    file.write_all_at(key, offset as u64)?;
    // 値を書き込み
    file.write_all_at(value, (offset + KEY_SIZE as i64) as u64)?;

    Ok(())
}

fn benchmark_pwrite_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("pwrite_sequential");
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
                let mut iteration = 0;
                b.iter_batched(
                    || {
                        iteration += 1;
                        let file_path = format!("/tmp/pwrite_test_{}.dat", iteration);
                        let file = OpenOptions::new()
                            .create(true)
                            .write(true)
                            .truncate(true)
                            .open(&file_path)
                            .expect("Failed to create file");
                        (file, file_path, generate_value(VALUE_SIZE))
                    },
                    |(file, file_path, value)| {
                        let entry_size = (KEY_SIZE + VALUE_SIZE) as i64;

                        for i in 0..num_keys {
                            let key = generate_key(i);
                            let offset = i as i64 * entry_size;
                            pwrite_sequential(&file, &key, &value, offset)
                                .expect("Failed to pwrite");
                        }

                        drop(file);
                        let _ = std::fs::remove_file(file_path);
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }
    group.finish();
}

fn benchmark_pwrite_with_sync(c: &mut Criterion) {
    let mut group = c.benchmark_group("pwrite_with_sync");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

    let num_keys = 100_000u64;
    let bytes_per_op = (KEY_SIZE + VALUE_SIZE) as u64;
    group.throughput(Throughput::Bytes(num_keys * bytes_per_op));

    group.bench_function("keys/100000", |b| {
        let mut iteration = 0;
        b.iter_batched(
            || {
                iteration += 1;
                let file_path = format!("/tmp/pwrite_sync_test_{}.dat", iteration);
                let file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .custom_flags(libc::O_SYNC)
                    .open(&file_path)
                    .expect("Failed to create file");
                (file, file_path, generate_value(VALUE_SIZE))
            },
            |(file, file_path, value)| {
                let entry_size = (KEY_SIZE + VALUE_SIZE) as i64;

                for i in 0..num_keys {
                    let key = generate_key(i);
                    let offset = i as i64 * entry_size;
                    pwrite_sequential(&file, &key, &value, offset)
                        .expect("Failed to pwrite");
                }

                drop(file);
                let _ = std::fs::remove_file(file_path);
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn benchmark_pwrite_with_fsync(c: &mut Criterion) {
    let mut group = c.benchmark_group("pwrite_with_fsync");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(10);

    let num_keys = 100_000u64;
    let bytes_per_op = (KEY_SIZE + VALUE_SIZE) as u64;
    group.throughput(Throughput::Bytes(num_keys * bytes_per_op));

    group.bench_function("keys/100000", |b| {
        let mut iteration = 0;
        b.iter_batched(
            || {
                iteration += 1;
                let file_path = format!("/tmp/pwrite_fsync_test_{}.dat", iteration);
                let file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&file_path)
                    .expect("Failed to create file");
                (file, file_path, generate_value(VALUE_SIZE))
            },
            |(file, file_path, value)| {
                let entry_size = (KEY_SIZE + VALUE_SIZE) as i64;

                for i in 0..num_keys {
                    let key = generate_key(i);
                    let offset = i as i64 * entry_size;
                    pwrite_sequential(&file, &key, &value, offset)
                        .expect("Failed to pwrite");
                }

                // 最後にfsyncを1回だけ実行
                file.sync_all().expect("Failed to fsync");

                drop(file);
                let _ = std::fs::remove_file(file_path);
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_pwrite_sequential,
    benchmark_pwrite_with_sync,
    benchmark_pwrite_with_fsync
);
criterion_main!(benches);
