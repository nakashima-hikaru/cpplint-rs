use cpplint_core::runner::{RunnerConfig, run_lint};
use criterion::{Criterion, criterion_group, criterion_main};
use std::path::PathBuf;
use std::time::Duration;

fn bench_quantlib(c: &mut Criterion) {
    // ワークスぺースルートにある bench_data/QuantLib をターゲットにします
    // crates/cpplint-core から見て2階層上
    let mut quantlib_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    quantlib_path.pop();
    quantlib_path.pop();
    quantlib_path.push("bench_data");
    quantlib_path.push("QuantLib");

    if !quantlib_path.exists() {
        panic!(
            "QuantLib benchmark directory not found at {:?}. Run `just setup-bench-data` or clone QuantLib to run benchmarks.",
            quantlib_path
        );
    }

    let file_count = ignore::WalkBuilder::new(&quantlib_path)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()))
        .filter(|e| {
            let ext = e.path().extension().and_then(|s| s.to_str()).unwrap_or("");
            matches!(ext, "cc" | "cpp" | "cxx" | "h" | "hpp" | "hxx")
        })
        .count();

    assert!(
        file_count >= 100,
        "QuantLib benchmark corpus is missing or incomplete at {:?}. Found only {} C++ source files (expected >= 100).",
        quantlib_path,
        file_count
    );

    let config = RunnerConfig {
        recursive: true,
        quiet: true,
        num_threads: std::thread::available_parallelism().unwrap_or(std::num::NonZeroUsize::MIN),
        ..RunnerConfig::default()
    };

    let mut group = c.benchmark_group("macro");

    // QuantLibは巨大なので、サンプル数と計測時間を調整します
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));

    group.bench_function("quantlib", |b| {
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

criterion_group!(benches, bench_quantlib);
criterion_main!(benches);
