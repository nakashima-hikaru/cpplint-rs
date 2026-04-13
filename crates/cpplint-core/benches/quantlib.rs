use criterion::{criterion_group, criterion_main, Criterion};
use cpplint_core::runner::{run_lint, RunnerConfig};
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
        eprintln!("Warning: QuantLib directory not found at {:?}. Skipping benchmark.", quantlib_path);
        return;
    }

    let config = RunnerConfig {
        recursive: true,
        quiet: true,
        num_threads: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
        ..RunnerConfig::default()
    };

    let mut group = c.benchmark_group("macro");
    
    // QuantLibは巨大なので、サンプル数と計測時間を調整します
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    
    group.bench_function("quantlib", |b| {
        b.iter(|| {
            let _ = run_lint(&[quantlib_path.clone()], &config).unwrap();
        })
    });
    
    group.finish();
}

criterion_group!(benches, bench_quantlib);
criterion_main!(benches);
