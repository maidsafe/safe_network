use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::{exit, Command};
use std::time::Duration;
use tempfile::tempdir;

fn safe_files_upload(dir: &str) {
    let output = Command::new("./target/release/safe")
        .arg("files")
        .arg("upload")
        .arg(dir)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        panic!("Upload command executed with failing error code");
    }
}
fn safe_files_download() {
    let output = Command::new("./target/release/safe")
        .arg("files")
        .arg("download")
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        panic!("Download command executed with failing error code");
    }
}

fn create_file(size_mb: u64) -> tempfile::TempDir {
    let dir = tempdir().expect("Failed to create temporary directory");
    let file_path = dir.path().join("tempfile");

    let mut file = File::create(file_path).expect("Failed to create file");
    let data = vec![0u8; (size_mb * 1024 * 1024) as usize]; // Create a vector with size_mb MB of data
    file.write_all(&data).expect("Failed to write to file");

    dir
}

fn criterion_benchmark(c: &mut Criterion) {
    // Check if the binary exists
    if !Path::new("./target/release/safe").exists() {
        eprintln!("Error: Binary ./target/release/safe does not exist. Please make sure to compile your project first");
        exit(1);
    }

    let sizes = vec![1, 10]; // File sizes in MB. Add more sizes as needed

    for size in sizes {
        let dir = create_file(size);
        let dir_path = dir.path().to_str().unwrap();

        let mut group = c.benchmark_group(format!("Upload Benchmark {}MB", size));
        group.sample_size(50);
        group.sampling_mode(criterion::SamplingMode::Flat);
        group.measurement_time(Duration::from_secs(120));
        group.warm_up_time(Duration::from_secs(10));

        group.bench_function(BenchmarkId::new("safe files upload", size), |b| {
            b.iter(|| safe_files_upload(dir_path))
        });
        group.finish();
    }

    // and now run downloads (will download everything we just uploaded)
    let mut group = c.benchmark_group("Download Benchmark");
    group.warm_up_time(Duration::from_secs(10));
    group.sample_size(10);

    group.bench_function("safe files download", |b| b.iter(safe_files_download));
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
