use cpplint_core::runner::{RunnerConfig, run_lint};
use criterion::{Criterion, criterion_group, criterion_main};
use std::path::PathBuf;
use std::time::Duration;

fn bench_synthetic_corpus(c: &mut Criterion) {
    let temp_dir = std::env::temp_dir().join("cpplint_bench_synthetic");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let sample_cpp = r#"// Copyright 2026 Test Project Authors. All rights reserved.
#ifndef BENCH_SYNTHETIC_HEADER_H_
#define BENCH_SYNTHETIC_HEADER_H_

#include <vector>
#include <string>
#include <iostream>

namespace bench::synthetic {

class SyntheticClass {
 public:
    SyntheticClass() : count_(0) {}
    explicit SyntheticClass(int val) : count_(val) {}
    ~SyntheticClass() = default;

    void DoWork(const std::vector<std::string>& inputs) {
        for (const auto& item : inputs) {
            std::cout << item << std::endl;
        }
    }

    int GetCount() const { return count_; }

 private:
    int count_;
};

}  // namespace bench::synthetic

#endif  // BENCH_SYNTHETIC_HEADER_H_
"#;

    let mut file_paths = Vec::new();
    for i in 0..100 {
        let file_path = temp_dir.join(format!("synthetic_{}.cc", i));
        std::fs::write(&file_path, sample_cpp).unwrap();
        file_paths.push(file_path);
    }

    let mut group = c.benchmark_group("synthetic");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(5));

    let thread_counts = [1, 2, 4, 8];
    for &threads in &thread_counts {
        let num_threads = std::num::NonZeroUsize::new(threads).unwrap();
        let config = RunnerConfig {
            recursive: false,
            quiet: true,
            num_threads,
            ..RunnerConfig::default()
        };

        // Cold benchmark (creates new thread pool each iteration)
        group.bench_function(format!("synthetic_cold_{}_threads", threads), |b| {
            b.iter(|| {
                let _ = run_lint(
                    &file_paths,
                    &config,
                    std::io::sink(),
                    std::io::sink(),
                )
                .unwrap();
            })
        });

        // Reusable Runner benchmark (reuses thread pool across iterations)
        if let Ok(runner) = cpplint_core::runner::Runner::new(num_threads) {
            group.bench_function(format!("synthetic_reusable_{}_threads", threads), |b| {
                b.iter(|| {
                    let _ = runner
                        .lint(&file_paths, &config, std::io::sink(), std::io::sink())
                        .unwrap();
                })
            });
        }
    }

    group.finish();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn bench_quantlib(c: &mut Criterion) {
    let mut quantlib_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    quantlib_path.pop();
    quantlib_path.pop();
    quantlib_path.push("bench_data");
    quantlib_path.push("QuantLib");

    if !quantlib_path.exists() {
        return;
    }

    let file_count = ignore::WalkBuilder::new(&quantlib_path)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
        .filter(|e| {
            let ext = e.path().extension().and_then(|s| s.to_str()).unwrap_or("");
            matches!(ext, "cc" | "cpp" | "cxx" | "h" | "hpp" | "hxx")
        })
        .count();

    if file_count < 100 {
        return;
    }

    let config = RunnerConfig {
        recursive: true,
        quiet: true,
        num_threads: std::thread::available_parallelism().unwrap_or(std::num::NonZeroUsize::MIN),
        ..RunnerConfig::default()
    };

    let mut group = c.benchmark_group("quantlib");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    group.bench_function("quantlib_full", |b| {
        b.iter(|| {
            let _ = run_lint(
                &[quantlib_path.clone()],
                &config,
                std::io::sink(),
                std::io::sink(),
            )
            .unwrap();
        })
    });

    group.finish();
}

criterion_group!(benches, bench_synthetic_corpus, bench_quantlib);
criterion_main!(benches);
