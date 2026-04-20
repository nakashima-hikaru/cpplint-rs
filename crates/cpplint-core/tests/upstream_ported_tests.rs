use cpplint_core::categories::Category;
use cpplint_core::file_linter::FileLinter;
use cpplint_core::options::Options;
use cpplint_core::state::CppLintState;
use std::path::{Path, PathBuf};

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

fn run_lint_with_verbose(filename: &str, lines: Vec<String>, verbose: i32) -> CppLintState {
    let state = CppLintState::new();
    state.set_verbose_level(verbose);
    let options = Options::new();
    let mut linter = FileLinter::new(PathBuf::from(filename), &state, options);

    linter.process_file_data(lines);
    state
}

fn path_split_to_list(path: &str) -> Vec<String> {
    if path.is_empty() {
        return vec![String::new()];
    }

    Path::new(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn test_spacing_before_braces() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (foo){".to_string(),
        "blah{32}".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(Category::WhitespaceBraces));

    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "for {".to_string(),
        "EXPECT_DEBUG_DEATH({".to_string(),
        "std::is_convertible<A, B>{}".to_string(),
        "int8_t{3}".to_string(),
        "MoveOnly(int i1, int i2) : ip1{new int{i1}}, ip2{new int{i2}} {}".to_string(),
        "".to_string(),
    ]);
    assert!(!pass_state.has_error(Category::WhitespaceBraces));
}

#[test]
fn test_brace_initializer_list() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "MyStruct p = {1, 2};".to_string(),
        "MyStruct p{1, 2};".to_string(),
        "map_of_pairs[{1, 2}] = 3;".to_string(),
        "return {1, 2};".to_string(),
        "int p[2] = {1, 2};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(state.error_count(), 0);
}

#[test]
fn test_spacing_around_else() {
    let fail_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (foo) {".to_string(),
        "}else {".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert!(fail_state.has_error(Category::WhitespaceBraces));

    let fail_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (foo) {".to_string(),
        "} else{".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert!(fail_state.has_error(Category::WhitespaceBraces));

    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (foo) {".to_string(),
        "} else {".to_string(),
        "} else if (bar) {".to_string(),
        "".to_string(),
    ]);
    assert!(!pass_state.has_error(Category::WhitespaceBraces));
}

#[test]
fn test_two_spaces_between_code_and_comments() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "} // namespace foo".to_string(),
        "}// namespace foo".to_string(),
        "printf(\"foo\"); // Outside quotes.".to_string(),
        "int i = 0;  // Having two spaces is fine.".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(Category::WhitespaceComments));
}

#[test]
fn test_space_after_comment_marker() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "//x".to_string(),
        "////x".to_string(),
        "//!<x".to_string(),
        "///<x".to_string(),
        "// x".to_string(),
        "///".to_string(),
        "//!".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(Category::WhitespaceComments));
}

#[test]
fn test_line_preceded_by_empty_or_comment_lines() {
    let state = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "".to_string(),
            "".to_string(),
            "// hello".to_string(),
            "using namespace foo;".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(state.error_count(), 1);
    assert!(state.has_error(Category::BuildNamespaces));
}

#[test]
fn test_blank_line_before_section_keyword() {
    let mut lines = vec!["// Copyright 2026".to_string(), "class A {".to_string()];
    for i in 0..30 {
        lines.push(format!("  int value{};", i));
    }
    lines.push(" private:".to_string());
    lines.push("  int tail;".to_string());
    lines.push("};".to_string());
    lines.push("".to_string());

    let state = run_lint(lines);
    assert!(state.has_error(Category::WhitespaceBlankLine));
}

#[test]
fn test_allow_blank_lines_in_raw_strings() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "static const char *kData[] = {R\"(".to_string(),
        "".to_string(),
        ")\"};".to_string(),
        "".to_string(),
    ]);
    assert!(!state.has_error(Category::WhitespaceBlankLine));
}

#[test]
fn test_else_on_same_line_as_closing_braces() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (hoge) {".to_string(),
        "}".to_string(),
        "else if (piyo) {".to_string(),
        "}".to_string(),
        " else {".to_string(),
        "".to_string(),
        "}".to_string(),
    ]);
    assert!(state.has_error(Category::WhitespaceNewline));
}

#[test]
fn test_multiple_statements_on_same_line() {
    let state = run_lint_with_verbose(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "sum += MathUtil::SafeIntRound(x); x += 0.1;".to_string(),
            "".to_string(),
        ],
        0,
    );
    assert!(state.has_error(Category::WhitespaceNewline));
}

#[test]
fn test_lambdas_on_same_line() {
    let state = run_lint_with_verbose(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "const auto lambda = [](const int i) { return i; };".to_string(),
            "const auto result = std::any_of(vector.begin(), vector.end(), [](const int i) { return i > 0; });".to_string(),
            "return mutex::Lock<void>([this]() { this->ReadLock(); }, [this]() { this->ReadUnlock(); });".to_string(),
            "".to_string(),
        ],
        0,
    );
    assert!(!state.has_error(Category::WhitespaceNewline));
}

#[test]
fn test_end_of_namespace_comments() {
    let state = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "namespace expected {".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "}".to_string(),
            "namespace outer { namespace nested {".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "}".to_string(),
            "}".to_string(),
            "namespace named_ok {".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "}  // namespace named_ok".to_string(),
            "namespace {".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "}  // anonymous namespace".to_string(),
        ],
    );
    assert!(state.has_error(Category::ReadabilityNamespace));
    assert!(state.error_count() >= 3);
}

#[test]
fn test_using_namespaces_granular() {
    let header_state = run_lint_with_filename(
        "foo.h",
        vec![
            "// Copyright 2026".to_string(),
            "using namespace std;".to_string(),
            "using namespace std::literals;".to_string(),
            "".to_string(),
        ],
    );
    assert!(header_state.has_error(Category::BuildNamespacesHeaders));
    assert!(header_state.has_error(Category::BuildNamespacesLiterals));

    let source_state = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "using namespace std;".to_string(),
            "using namespace std::chrono::literals;".to_string(),
            "".to_string(),
        ],
    );
    assert!(source_state.has_error(Category::BuildNamespaces));
    assert!(source_state.has_error(Category::BuildNamespacesLiterals));
}

#[test]
fn test_comma() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "a = f(1,2);".to_string(),
        "int tmp=a,a=b,b=tmp;".to_string(),
        "f(a, /* name */ b);".to_string(),
        "operator,(a,b)".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(Category::WhitespaceComma));
    assert!(state.has_error(Category::WhitespaceOperators));
}

#[test]
fn test_equals_operator_spacing() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int tmp= a;".to_string(),
        "int tmp =a;".to_string(),
        "int tmp=a;".to_string(),
        "bool result = a>=42;".to_string(),
        "auto result = a!=42;".to_string(),
        "a&=42;".to_string(),
        "a<<=5;".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(Category::WhitespaceOperators));
}

#[test]
fn test_shift_operator_spacing() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "a<<b".to_string(),
        "a>>b".to_string(),
        "1<<20".to_string(),
        "1024>>10".to_string(),
        "Kernel<<<1, 2>>()".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(Category::WhitespaceOperators));
}

#[test]
fn test_indent() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        " int one_space_indent;".to_string(),
        "   int three_space_indent;".to_string(),
        "  int two_space_indent;".to_string(),
        "    int four_space_indent;".to_string(),
        " public:".to_string(),
        "  protected:".to_string(),
        "   private:".to_string(),
        " protected: \\".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(Category::WhitespaceIndent));
}

#[test]
fn test_section_indent() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "class A {".to_string(),
        " public:".to_string(),
        "   private:".to_string(),
        "  int a;".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(Category::WhitespaceIndent));
}

#[test]
fn test_conditionals() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (foo)".to_string(),
        "  goto fail;".to_string(),
        "  goto fail;".to_string(),
        "if (foo)".to_string(),
        "  if (bar)".to_string(),
        "    baz;".to_string(),
        "  else".to_string(),
        "    qux;".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(Category::ReadabilityBraces));
}

#[test]
fn test_control_clause_with_parens_newline() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (condition) [[unlikely]] { do_something(); }".to_string(),
        "while (condition) { do_something(); }".to_string(),
        "for (int i = 0; i < 1; ++i) { do_something(); }".to_string(),
        "switch (value) { do_something(); }".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(Category::WhitespaceNewline));
}

#[test]
fn test_control_clause_without_parens_newline() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "else { do_something(); }".to_string(),
        "do { do_something(); }".to_string(),
        "try { do_something(); }".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(Category::WhitespaceNewline));
}

#[test]
fn test_control_clause_newline_name_false_positives() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "  else if_condition_do_something();".to_string(),
        "  } else if (blah) {".to_string(),
        "  variable_ends_in_else = true;".to_string(),
        "".to_string(),
    ]);
    assert!(!state.has_error(Category::WhitespaceNewline));
}

#[test]
fn test_tab() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "\tint a;".to_string(),
        "int a = 5;\t\t// set a to 5".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(Category::WhitespaceTab));
}

#[test]
fn test_path_split_to_list() {
    assert_eq!(path_split_to_list(""), vec![""]);
    assert_eq!(path_split_to_list("."), vec!["."]);
    assert_eq!(path_split_to_list(".."), vec![".."]);
    assert_eq!(path_split_to_list("../a/b"), vec!["..", "a", "b"]);
    assert_eq!(path_split_to_list("a/b/c/d"), vec!["a", "b", "c", "d"]);
}
