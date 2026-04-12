use cpplint_core::file_linter::FileLinter;
use cpplint_core::state::CppLintState;
use cpplint_core::options::Options;
use std::path::PathBuf;

fn run_lint(lines: Vec<String>) -> CppLintState {
    run_lint_with_filename("test.cpp", lines)
}

fn run_lint_with_filename(filename: &str, lines: Vec<String>) -> CppLintState {
    let state = CppLintState::new();
    let options = Options::new();
    let mut linter = FileLinter::new(PathBuf::from(filename), &state, options);

    linter.process_file_data(lines);
    state
}

#[test]
fn test_blank_line_at_eof() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int main() {}".to_string(),
    ]);
    assert!(state.error_count() >= 1);
    assert!(state.has_error("whitespace/ending_newline"));
}

#[test]
fn test_tab_check() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "	int x = 0;".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error("whitespace/tab"));
}

#[test]
fn test_c_style_cast() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int x = (int)1.0;".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error("readability/casting"));
}

#[test]
fn test_brace_placement() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "void Function()".to_string(),
        "{".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error("whitespace/braces"));
}

#[test]
fn test_extra_space_before_bracket() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int x [10];".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error("whitespace/braces"));
}

#[test]
fn test_range_for_spacing() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "for (auto x:collection) {}".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error("whitespace/forcolon"));
}

#[test]
fn test_operator_spacing() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int x=0;".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error("whitespace/operators"));
}

#[test]
fn test_header_guard_nolint_is_respected() {
    let state = run_lint_with_filename(
        "test.h",
        vec![
            "// Copyright 2026".to_string(),
            "// NOLINT(build/header_guard)".to_string(),
        ],
    );
    assert!(!state.has_error("build/header_guard"));
}

#[test]
fn test_header_guard_uses_parent_directories() {
    let state = run_lint_with_filename(
        "foo/bar/baz.h",
        vec![
            "#ifndef FOO_BAR_BAZ_H_".to_string(),
            "#define FOO_BAR_BAZ_H_".to_string(),
            "#endif  // FOO_BAR_BAZ_H_".to_string(),
        ],
    );
    assert!(!state.has_error("build/header_guard"));
}

#[test]
fn test_operator_assignment_overload_is_not_flagged() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "T& operator=(const T& t);".to_string(),
        "".to_string(),
    ]);
    assert!(!state.has_error("whitespace/operators"));
}

#[test]
fn test_header_guard_requires_endif_comment() {
    let state = run_lint_with_filename(
        "test.h",
        vec![
            "#ifndef TEST_H_".to_string(),
            "#define TEST_H_".to_string(),
            "#endif".to_string(),
        ],
    );
    assert!(state.has_error("build/header_guard"));
}

#[test]
fn test_header_guard_checks_hxx_files() {
    let state = run_lint_with_filename(
        "test.hxx",
        vec![
            "#ifndef TEST_HXX_".to_string(),
            "#define TEST_HXX_".to_string(),
            "#endif  // TEST_HXX_".to_string(),
        ],
    );
    assert!(!state.has_error("build/header_guard"));
}
