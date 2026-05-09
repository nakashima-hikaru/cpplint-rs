use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap()
}

fn cpplint_binary() -> &'static str {
    env!("CARGO_BIN_EXE_cpplint")
}

fn run_cli(repo_root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cpplint_binary())
        .current_dir(repo_root)
        .args(args)
        .output()
        .unwrap()
}

#[allow(dead_code)]
fn read_golden(name: &str, repo_root: &Path) -> String {
    std::fs::read_to_string(repo_root.join("cpplint-rs/tests/golden").join(name))
        .unwrap()
        .replace("{{REPO_ROOT}}", &repo_root.to_string_lossy())
}

fn temp_dir() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("cpplint-rs-cli-{}-{}", unique, counter))
}

// #[test]
// fn matches_golden_outputs_for_repository_samples() {
//     let root = repo_root();

//     let clean_source = run_cli(&root, &["src/string_utils.cpp"]);
//     assert!(clean_source.status.success());
//     assert_eq!(
//         String::from_utf8_lossy(&clean_source.stdout),
//         read_golden("string_utils.txt", &root)
//     );
//     assert!(clean_source.stderr.is_empty());

//     let clean_header = run_cli(&root, &["include/string_utils.h"]);
//     assert!(clean_header.status.success());
//     assert_eq!(
//         String::from_utf8_lossy(&clean_header.stdout),
//         read_golden("string_utils_h.txt", &root)
//     );
//     assert!(clean_header.stderr.is_empty());

//     let excluded = run_cli(&root, &["tests/test_files/invalid_utf.c"]);
//     assert!(excluded.status.success());
//     assert_eq!(
//         String::from_utf8_lossy(&excluded.stdout),
//         read_golden("invalid_utf.txt", &root)
//     );
//     assert!(excluded.stderr.is_empty());
// }

#[test]
fn supports_alternate_output_formats() {
    let root = repo_root();
    let temp = temp_dir();
    std::fs::create_dir_all(&temp).unwrap();
    let file = temp.join("bad.cc");
    std::fs::write(&file, "int x=0;\n").unwrap();

    let file_arg = file.to_string_lossy().to_string();

    let vs7 = run_cli(
        &root,
        &["--filter=-legal/copyright", "--output=vs7", &file_arg],
    );
    assert_eq!(vs7.status.code(), Some(1));
    let vs7_stderr = String::from_utf8_lossy(&vs7.stderr);
    assert!(vs7_stderr.contains("bad.cc(1): error cpplint: [whitespace/operators]"));

    let junit = run_cli(
        &root,
        &["--filter=-legal/copyright", "--output=junit", &file_arg],
    );
    assert_eq!(junit.status.code(), Some(1));
    let junit_stdout = String::from_utf8_lossy(&junit.stdout);
    assert!(junit_stdout.contains("<testsuite"));
    assert!(junit_stdout.contains("<failure"));
    assert!(junit_stdout.contains("bad.cc"));

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn explicit_check_subcommand_preserves_legacy_behavior() {
    let root = repo_root();

    let legacy = run_cli(&root, &["src/string_utils.cpp"]);
    let explicit = run_cli(&root, &["check", "src/string_utils.cpp"]);

    assert_eq!(legacy.status.code(), explicit.status.code());
    assert_eq!(legacy.stdout, explicit.stdout);
    assert_eq!(legacy.stderr, explicit.stderr);
}

#[test]
fn rule_command_lists_families_and_categories() {
    let root = repo_root();

    let overview = run_cli(&root, &["rule"]);
    assert!(overview.status.success());
    let overview_stdout = String::from_utf8_lossy(&overview.stdout);
    assert!(overview_stdout.contains("cpplint rule families"));
    assert!(overview_stdout.contains("headers"));

    let family = run_cli(&root, &["rule", "headers"]);
    assert!(family.status.success());
    let family_stdout = String::from_utf8_lossy(&family.stdout);
    assert!(family_stdout.contains("Family: headers"));
    assert!(family_stdout.contains("build/header_guard"));

    let category = run_cli(&root, &["rule", "whitespace/operators"]);
    assert!(category.status.success());
    let category_stdout = String::from_utf8_lossy(&category.stdout);
    assert!(category_stdout.contains("Rule: whitespace/operators"));
    assert!(category_stdout.contains("Family: whitespace"));
}

#[test]
fn fix_flag_rewrites_file_in_place() {
    let root = repo_root();
    let temp = temp_dir();
    std::fs::create_dir_all(&temp).unwrap();
    let file = temp.join("bad.cc");
    std::fs::write(&file, "int x=0;\nCHECK(mode == 'x');\n").unwrap();

    let file_arg = file.to_string_lossy().to_string();
    let fixed = run_cli(&root, &["--filter=-legal/copyright", "--fix", &file_arg]);
    assert_eq!(fixed.status.code(), Some(0));

    let contents = std::fs::read_to_string(&file).unwrap();
    assert_eq!(contents, "int x = 0;\nCHECK_EQ(mode, 'x');\n");

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn quiet_mode_suppresses_clean_output_but_not_error_output() {
    let root = repo_root();
    let temp = temp_dir();
    std::fs::create_dir_all(&temp).unwrap();
    let file = temp.join("sample.cc");

    std::fs::write(&file, "int clean = 0;\n").unwrap();
    let file_arg = file.to_string_lossy().to_string();

    let clean = run_cli(
        &root,
        &[
            "--quiet",
            "--filter=-legal/copyright,-whitespace/operators",
            &file_arg,
        ],
    );
    assert_eq!(clean.status.code(), Some(0));
    assert!(clean.stdout.is_empty());
    assert!(clean.stderr.is_empty());

    std::fs::write(&file, "int x=0;\n").unwrap();
    let has_error = run_cli(&root, &["--quiet", "--filter=-legal/copyright", &file_arg]);
    assert_eq!(has_error.status.code(), Some(1));
    assert!(!has_error.stderr.is_empty());
    assert!(
        String::from_utf8_lossy(&has_error.stderr).contains("[whitespace/operators]"),
        "stderr should include diagnostics in quiet mode when errors exist"
    );

    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn recursive_and_exclude_match_expected_files() {
    let root = repo_root();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp =
        std::env::temp_dir().join(format!("cpplint-rs-cli-recursive-{}-{}", unique, counter));
    let nested = temp.join("sub");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(temp.join("keep.cc"), "int keep=0;\n").unwrap();
    std::fs::write(nested.join("skip.cc"), "int skip=0;\n").unwrap();

    let dir_arg = temp.to_string_lossy().to_string();
    let output = run_cli(
        &root,
        &[
            "--recursive",
            "--exclude=**/skip.cc",
            "--filter=-legal/copyright",
            &dir_arg,
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("keep.cc"));
    assert_eq!(stderr.matches("[whitespace/operators]").count(), 1);

    std::fs::remove_dir_all(temp).unwrap();
}
