use cpplint_core::runner::{RunnerConfig, run_lint};
use cpplint_core::state::OutputFormat;
use ignore::WalkBuilder;
use std::ffi::OsStr;
use std::hint::black_box;
use std::path::{Path, PathBuf};

fn is_profile_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("c" | "cc" | "cpp" | "cxx" | "h" | "hpp" | "hxx")
    )
}

fn collect_profile_files(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return root
            .is_file()
            .then(|| root.to_path_buf())
            .filter(|path| is_profile_file(path))
            .into_iter()
            .collect();
    }

    let mut walk = WalkBuilder::new(root);
    walk.hidden(false)
        .git_ignore(false)
        .git_exclude(false)
        .parents(false)
        .ignore(false);

    let mut files = Vec::new();
    for entry in walk.build().flatten() {
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        let path = entry.into_path();
        if is_profile_file(&path) {
            files.push(path);
        }
    }
    files
}

fn sample_repo_files() -> Vec<PathBuf> {
    if let Some(target) = std::env::var_os("CPPLINT_HOTPATH_TARGET") {
        let mut files = collect_profile_files(Path::new(&target));
        files.sort();
        return files;
    }

    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    let mut files = Vec::new();

    for directory in [repo_root.join("src"), repo_root.join("include")] {
        if !directory.exists() {
            continue;
        }
        files.extend(collect_profile_files(&directory));
    }

    files.sort();
    files
}

fn profile_iterations() -> usize {
    std::env::var("CPPLINT_HOTPATH_ITERS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(20)
}

fn profile_threads() -> usize {
    std::env::var("CPPLINT_HOTPATH_THREADS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1)
        .max(1)
}

fn profile_config(num_threads: usize) -> RunnerConfig {
    let mut config = RunnerConfig {
        output_format: OutputFormat::Emacs,
        quiet: true,
        num_threads,
        ..RunnerConfig::default()
    };
    config.options.add_filter("-legal/copyright");
    config
}

#[cfg_attr(feature = "hotpath", hotpath::main)]
fn main() {
    let files = sample_repo_files();
    let config = profile_config(profile_threads());
    let iterations = profile_iterations();

    for _ in 0..iterations {
        let result = run_lint(
            black_box(&files),
            black_box(&config),
            std::io::sink(),
            std::io::sink(),
        )
        .unwrap();
        black_box((result.stdout.len(), result.stderr.len(), result.error_count));
    }
}
