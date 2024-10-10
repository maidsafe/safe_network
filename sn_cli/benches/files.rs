// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::{thread_rng, Rng};
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::{exit, Command},
    time::Duration,
};
use tempfile::tempdir;

const SAMPLE_SIZE: usize = 20;

// This procedure includes the client startup, which will be measured by criterion as well.
// As normal user won't care much about initial client startup,
// but be more alerted on communication speed during transmission.
// It will be better to execute bench test with `local`,
// to make the measurement results reflect speed improvement or regression more accurately.
fn safe_files_upload(dir: &str) {
    let output = Command::new("./target/release/safe")
        .arg("files")
        .arg("upload")
        .arg(dir)
        .arg("--retry-strategy") // no retries
        .arg("quick")
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        let err = output.stderr;
        let err_string = String::from_utf8(err).expect("Failed to parse error string");
        panic!("Upload command executed with failing error code: {err_string:?}");
    }
}

fn safe_files_download() {
    let output = Command::new("./target/release/safe")
        .arg("files")
        .arg("download")
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        let err = output.stderr;
        let err_string = String::from_utf8(err).expect("Failed to parse error string");
        panic!("Download command executed with failing error code: {err_string:?}");
    }
}

fn generate_file(path: &PathBuf, file_size_mb: usize) {
    let mut file = File::create(path).expect("Failed to create file");
    let mut rng = thread_rng();

    // can create [u8; 32] max at time. Thus each mb has 1024*32 such small chunks
    let n_small_chunks = file_size_mb * 1024 * 32;
    for _ in 0..n_small_chunks {
        let random_data: [u8; 32] = rng.gen();
        file.write_all(&random_data)
            .expect("Failed to write to file");
    }
    let size = file.metadata().expect("Failed to get metadata").len() as f64 / (1024 * 1024) as f64;
    assert_eq!(file_size_mb as f64, size);
}

fn fund_cli_wallet() {
    let _ = Command::new("./target/release/safe")
        .arg("wallet")
        .arg("get-faucet")
        .arg("127.0.0.1:8000")
        .output()
        .expect("Failed to execute 'safe wallet get-faucet' command");
}

fn criterion_benchmark(c: &mut Criterion) {
    // Check if the binary exists
    if !Path::new("./target/release/safe").exists() {
        eprintln!("Error: Binary ./target/release/safe does not exist. Please make sure to compile your project first");
        exit(1);
    }

    let sizes: [u64; 2] = [1, 10]; // File sizes in MB. Add more sizes as needed

    for size in sizes.iter() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let temp_dir_path = temp_dir.into_path();
        let temp_dir_path_str = temp_dir_path.to_str().expect("Invalid unicode encountered");

        // create 23 random files. This is to keep the benchmark results consistent with prior runs. The change to make
        // use of ChunkManager means that we don't upload the same file twice and the `uploaded_files` file is now read
        // as a set and we don't download the same file twice. Hence create 23 files as counted from the logs
        // pre ChunkManager change.
        (0..23).into_par_iter().for_each(|idx| {
            let path = temp_dir_path.join(format!("random_file_{size}_mb_{idx}"));
            generate_file(&path, *size as usize);
        });
        fund_cli_wallet();

        // Wait little bit for the fund to be settled.
        std::thread::sleep(Duration::from_secs(10));

        let mut group = c.benchmark_group(format!("Upload Benchmark {size}MB"));
        group.sampling_mode(criterion::SamplingMode::Flat);
        // One sample may compose of multiple iterations, and this is decided by `measurement_time`.
        // Set this to a lower value to ensure each sample only contains one iteration.
        // To ensure the download throughput calculation is correct.
        group.measurement_time(Duration::from_secs(5));
        group.warm_up_time(Duration::from_secs(5));
        group.sample_size(SAMPLE_SIZE);

        // Set the throughput to be reported in terms of bytes
        group.throughput(Throughput::Bytes(size * 1024 * 1024));
        let bench_id = format!("safe files upload {size}mb");
        group.bench_function(bench_id, |b| {
            b.iter(|| safe_files_upload(temp_dir_path_str))
        });
        group.finish();
    }

    let mut group = c.benchmark_group("Download Benchmark".to_string());
    group.sampling_mode(criterion::SamplingMode::Flat);
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(5));

    // The download will download all uploaded files during bench.
    // If the previous bench executed with the default 100 sample size,
    // there will then be around 1.1GB in total, and may take around 40s for each iteratioin.
    // Hence we have to reduce the number of iterations from the default 100 to 10,
    // To avoid the benchmark test taking over one hour to complete.
    //
    // During `measurement_time` and `warm_up_time`, there will be one upload run for each.
    // Which means two additional `uploaded_files` created and for downloading.
    let total_size: u64 = sizes
        .iter()
        .map(|size| (SAMPLE_SIZE as u64 + 2) * size)
        .sum();
    group.sample_size(SAMPLE_SIZE / 2);

    // Set the throughput to be reported in terms of bytes
    group.throughput(Throughput::Bytes(total_size * 1024 * 1024));
    let bench_id = "safe files download".to_string();
    group.bench_function(bench_id, |b| b.iter(safe_files_download));
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
