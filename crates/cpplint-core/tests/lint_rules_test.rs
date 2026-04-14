use cpplint_core::file_linter::FileLinter;
use cpplint_core::options::Options;
use cpplint_core::state::CppLintState;
use std::path::PathBuf;

fn run_lint(lines: Vec<String>) -> CppLintState {
    run_lint_with_filename("test.cpp", lines)
}

fn run_lint_with_filter(filename: &str, lines: Vec<String>, filter: &str) -> CppLintState {
    let state = CppLintState::new();
    let mut options = Options::new();
    options.add_filter(filter);
    let mut linter = FileLinter::new(PathBuf::from(filename), &state, options);

    linter.process_file_data(lines);
    state
}

fn run_lint_with_filters(filename: &str, lines: Vec<String>, filters: &[&str]) -> CppLintState {
    let state = CppLintState::new();
    let mut options = Options::new();
    for filter in filters {
        options.add_filter(filter);
    }
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

fn run_lint_with_filters_and_verbose(
    filename: &str,
    lines: Vec<String>,
    filters: &[&str],
    verbose: i32,
) -> CppLintState {
    let state = CppLintState::new();
    state.set_verbose_level(verbose);
    let mut options = Options::new();
    for filter in filters {
        options.add_filter(filter);
    }
    let mut linter = FileLinter::new(PathBuf::from(filename), &state, options);

    linter.process_file_data(lines);
    state
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
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceEndingNewline));
}

#[test]
fn test_nul_byte_lines() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "char data[] = \"a\0b\";".to_string(),
        "const char* other = \"\0\";".to_string(),
        "".to_string(),
    ]);
    assert_eq!(state.error_count(), 2);
    assert!(state.has_error(cpplint_core::categories::Category::ReadabilityNul));
}

#[test]
fn test_tab_check() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "	int x = 0;".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceTab));
}

#[test]
fn test_c_style_cast() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int x = (int)1.0;".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::ReadabilityCasting));
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
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceBraces));
}

#[test]
fn test_extra_space_before_bracket() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int x [10];".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceBraces));
}

#[test]
fn test_range_for_spacing() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "for (auto x:collection) {}".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceForcolon));
}

#[test]
fn test_operator_spacing() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int x=0;".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceOperators));
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
    assert!(!state.has_error(cpplint_core::categories::Category::BuildHeaderGuard));
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
    assert!(!state.has_error(cpplint_core::categories::Category::BuildHeaderGuard));
}

#[test]
fn test_operator_assignment_overload_is_not_flagged() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "T& operator=(const T& t);".to_string(),
        "".to_string(),
    ]);
    assert!(!state.has_error(cpplint_core::categories::Category::WhitespaceOperators));
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
    assert!(state.has_error(cpplint_core::categories::Category::BuildHeaderGuard));
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
    assert!(!state.has_error(cpplint_core::categories::Category::BuildHeaderGuard));
}

#[test]
fn test_comparison_operator_spacing() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (foo<bar) return;".to_string(),
        "if (foo>bar) return;".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceOperators));
}

#[test]
fn test_unary_operator_spacing() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "i ++;".to_string(),
        "! flag;".to_string(),
        "~ flag;".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceOperators));
}

#[test]
fn test_control_statement_paren_spacing() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "for(;;) {}".to_string(),
        "if(true) return;".to_string(),
        "while(true) continue;".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceParens));
}

#[test]
fn test_mismatching_spaces_inside_control_parens() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (foo ) {".to_string(),
        "}".to_string(),
        "switch ( foo) {".to_string(),
        "}".to_string(),
        "for (foo; bar; baz ) {".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceParens));
}

#[test]
fn test_extra_spaces_inside_control_parens() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "while (  foo  ) {".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceParens));
}

#[test]
fn test_comma_and_semicolon_spacing() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "a = f(1,2);".to_string(),
        "for (foo;bar;baz) {".to_string(),
        "  func();a = b;".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceComma));
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceSemicolon));
}

#[test]
fn test_function_call_paren_spacing() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "foo( bar);".to_string(),
        "foo (bar);".to_string(),
        "Func(1, 3 );".to_string(),
        "     );".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceParens));
}

#[test]
fn test_spacing_before_else_and_semicolon_rules() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (foo) {".to_string(),
        "}else {".to_string(),
        "default:;".to_string(),
        "    ;".to_string(),
        "func() ;".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceBraces));
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceSemicolon));
}

#[test]
fn test_comment_spacing_rules() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int a = 0; // comment".to_string(),
        "int b = 0;  //comment".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceComments));
}

#[test]
fn test_todo_spacing_and_username_rules() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int a = 0;  //  TODO(me): test".to_string(),
        "int b = 0;  // TODO: test".to_string(),
        "int c = 0;  // TODO(me):test".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceTodo));
    assert!(state.has_error(cpplint_core::categories::Category::ReadabilityTodo));
}

#[test]
fn test_comment_spacing_pass_cases() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int a = 0;  // comment".to_string(),
        "// TODO(me): test it".to_string(),
        "foo(  // comment".to_string(),
        "    bar);".to_string(),
        "".to_string(),
    ]);
    assert!(!state.has_error(cpplint_core::categories::Category::WhitespaceComments));
    assert!(!state.has_error(cpplint_core::categories::Category::WhitespaceTodo));
    assert!(!state.has_error(cpplint_core::categories::Category::ReadabilityTodo));
}

#[test]
fn test_blank_line_pass_cases() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "namespace {".to_string(),
        "".to_string(),
        "}  // namespace".to_string(),
        "namespace".to_string(),
        "detail".to_string(),
        "{".to_string(),
        "".to_string(),
        "int value = 0;".to_string(),
        "}".to_string(),
        "extern \"C\" {".to_string(),
        "".to_string(),
        "void Func() {}".to_string(),
        "".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert!(!state.has_error(cpplint_core::categories::Category::WhitespaceBlankLine));
}

#[test]
fn test_blank_line_block_start() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (foo) {".to_string(),
        "".to_string(),
        "    func();".to_string(),
        "} else if (bar) {".to_string(),
        "".to_string(),
        "    func();".to_string(),
        "} else {".to_string(),
        "".to_string(),
        "    func();".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceBlankLine));
}

#[test]
fn test_blank_line_block_end_and_after_section() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (foo) {".to_string(),
        "    func();".to_string(),
        "".to_string(),
        "} else if (bar) {".to_string(),
        "    func();".to_string(),
        "".to_string(),
        "} else {".to_string(),
        "    func();".to_string(),
        "".to_string(),
        "}".to_string(),
        "class A {".to_string(),
        " public:".to_string(),
        "".to_string(),
        " private:".to_string(),
        "".to_string(),
        "    struct B {".to_string(),
        "     protected:".to_string(),
        "".to_string(),
        "        int foo;".to_string(),
        "    };".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceBlankLine));
}

#[test]
fn test_function_call_before_parens_pass_cases() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "foo (Foo::*bar)(".to_string(),
        ");".to_string(),
        "foo (x::y::*z)(".to_string(),
        ");".to_string(),
        "foo (*bar)(".to_string(),
        ");".to_string(),
        "sizeof (foo);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(state.error_count(), 0);
    assert!(!state.has_error(cpplint_core::categories::Category::WhitespaceParens));
}

#[test]
fn test_function_call_before_parens_fail_cases() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "foo (bar);".to_string(),
        "foo (Foo::bar)(".to_string(),
        ");".to_string(),
        "__VA_OPT__ (,).".trim_end_matches('.').to_string(),
        "".to_string(),
    ]);
    assert_eq!(state.error_count(), 3);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceParens));
}

#[test]
fn test_function_call_closing_parens_cases() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "Func(1, 3".to_string(),
        "    );".to_string(),
        "Other(Nest(1),".to_string(),
        "      Nest(3".to_string(),
        "      ));".to_string(),
        "Func(1, 3 );".to_string(),
        "Func(1,".to_string(),
        "    3 );".to_string(),
        "".to_string(),
    ]);
    assert_eq!(state.error_count(), 4);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceParens));
}

#[test]
fn test_section_spacing_pass_cases() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "class Foo {".to_string(),
        " public:".to_string(),
        " protected:".to_string(),
        " private:".to_string(),
        "    struct B {".to_string(),
        "     public:".to_string(),
        "     private:".to_string(),
        "    };".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(state.error_count(), 0);
    assert!(!state.has_error(cpplint_core::categories::Category::WhitespaceBlankLine));
}

#[test]
fn test_section_spacing_fail_cases() {
    let mut lines = vec![
        "// Copyright 2026".to_string(),
        "class Foo {".to_string(),
        " public:".to_string(),
        " protected:".to_string(),
        " private:".to_string(),
        "    struct B {".to_string(),
        "     public:".to_string(),
        "     private:".to_string(),
        "    };".to_string(),
        "};".to_string(),
        "".to_string(),
    ];
    for _ in 0..22 {
        lines.insert(7, "        int a;".to_string());
    }

    let state = run_lint(lines);
    assert_eq!(state.error_count(), 3);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceBlankLine));
}

#[test]
fn test_include_checks_representative_cases() {
    let order_pass = run_lint_with_filename(
        "test.cpp",
        vec![
            "// Copyright 2026".to_string(),
            "#include \"test.h\"".to_string(),
            "#include <stdio.h>".to_string(),
            "#include <string>".to_string(),
            "#include \"foo/public.h\"".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(order_pass.error_count(), 0);

    let order_pass_hpp = run_lint_with_filename(
        "test.cpp",
        vec![
            "// Copyright 2026".to_string(),
            "#include \"test.hpp\"".to_string(),
            "#include <stdio.h>".to_string(),
            "#include <string>".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(order_pass_hpp.error_count(), 0);

    let order_pass_with_macro = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <string>".to_string(),
        "#ifdef PLATFORM".to_string(),
        "#include <stdio.h>".to_string(),
        "#endif".to_string(),
        "".to_string(),
    ]);
    assert_eq!(order_pass_with_macro.error_count(), 0);

    let subdir_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <string>".to_string(),
        "#include \"baz.aa\"".to_string(),
        "#include \"dir/foo.h\"".to_string(),
        "#include \"lua.h\"".to_string(),
        "".to_string(),
    ]);
    assert!(!subdir_pass.has_error(cpplint_core::categories::Category::BuildIncludeSubdir));

    let subdir_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include \"bar.hh\"".to_string(),
        "#include \"foo.h\"".to_string(),
        "".to_string(),
    ]);
    assert_eq!(subdir_fail.error_count(), 2);
    assert!(subdir_fail.has_error(cpplint_core::categories::Category::BuildIncludeSubdir));

    let duplication = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <string>".to_string(),
        "#include <string>".to_string(),
        "".to_string(),
    ]);
    assert_eq!(duplication.error_count(), 1);
    assert!(duplication.has_error(cpplint_core::categories::Category::BuildInclude));

    let other_pkg_c = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include \"other/package.c\"".to_string(),
        "".to_string(),
    ]);
    assert_eq!(other_pkg_c.error_count(), 1);
    assert!(other_pkg_c.has_error(cpplint_core::categories::Category::BuildInclude));

    let order_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <string>".to_string(),
        "#include <stdio.h>".to_string(),
        "".to_string(),
    ]);
    assert_eq!(order_fail.error_count(), 1);
    assert!(order_fail.has_error(cpplint_core::categories::Category::BuildIncludeOrder));

    let order_fail_with_macro = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include \"foo/public.h\"".to_string(),
        "#ifdef PLATFORM".to_string(),
        "#include <vector>".to_string(),
        "#endif".to_string(),
        "#include <string>".to_string(),
        "".to_string(),
    ]);
    assert_eq!(order_fail_with_macro.error_count(), 0);

    let alpha_fail = run_lint_with_filter(
        "test.cpp",
        vec![
            "// Copyright 2026".to_string(),
            "#include \"foo/e.h\"".to_string(),
            "#include \"foo/b.h\"".to_string(),
            "#include \"foo/c.h\"".to_string(),
            "".to_string(),
        ],
        "+build/include_alpha",
    );
    assert_eq!(alpha_fail.error_count(), 1);
    assert!(alpha_fail.has_error(cpplint_core::categories::Category::BuildIncludeAlpha));

    let alpha_pass_inl = run_lint_with_filter(
        "test.cpp",
        vec![
            "// Copyright 2026".to_string(),
            "#include \"foo/bar.h\"".to_string(),
            "#include \"foo/bar-inl.h\"".to_string(),
            "".to_string(),
        ],
        "+build/include_alpha",
    );
    assert_eq!(alpha_pass_inl.error_count(), 0);

    let iwyu_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <string>".to_string(),
        "void f() {".to_string(),
        "  std::string name;".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(iwyu_pass.error_count(), 0);

    let iwyu_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "void f() {".to_string(),
        "  std::string name;".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(iwyu_fail.error_count(), 1);
    assert!(iwyu_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));

    let iwyu_vector_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <vector>".to_string(),
        "void f() {".to_string(),
        "  std::vector< int > values;".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(iwyu_vector_pass.error_count(), 0);

    let iwyu_vector_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "void f() {".to_string(),
        "  std::vector< int > values;".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(iwyu_vector_fail.error_count(), 1);
    assert!(iwyu_vector_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));

    let iwyu_iostream_pass = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "#include <iostream>".to_string(),
            "void Print() {".to_string(),
            "  cout << value;".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_iostream_pass.error_count(), 0);

    let iwyu_iostream_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Print() {".to_string(),
            "  cout << value;".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_iostream_fail.error_count(), 1);
    assert!(
        iwyu_iostream_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_iostream_cerr_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Print() {".to_string(),
            "  cerr << value;".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_iostream_cerr_fail.error_count(), 1);
    assert!(
        iwyu_iostream_cerr_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_iostream_cin_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Read() {".to_string(),
            "  cin >> value;".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_iostream_cin_fail.error_count(), 1);
    assert!(
        iwyu_iostream_cin_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_iostream_clog_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Log() {".to_string(),
            "  clog << value;".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_iostream_clog_fail.error_count(), 1);
    assert!(
        iwyu_iostream_clog_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_iostream_wcout_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void PrintWide() {".to_string(),
            "  wcout << value;".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_iostream_wcout_fail.error_count(), 1);
    assert!(
        iwyu_iostream_wcout_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_iostream_wcin_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void ReadWide() {".to_string(),
            "  wcin >> value;".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_iostream_wcin_fail.error_count(), 1);
    assert!(
        iwyu_iostream_wcin_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_iostream_wcerr_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void ErrorWide() {".to_string(),
            "  wcerr << value;".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_iostream_wcerr_fail.error_count(), 1);
    assert!(
        iwyu_iostream_wcerr_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_iostream_wclog_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void LogWide() {".to_string(),
            "  wclog << value;".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_iostream_wclog_fail.error_count(), 1);
    assert!(
        iwyu_iostream_wclog_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstring_memcpy_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Copy() {".to_string(),
            "  memcpy(dst, src, size);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_memcpy_fail.error_count(), 0);

    let iwyu_cstring_memcmp_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Compare() {".to_string(),
            "  memcmp(lhs, rhs, size);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_memcmp_fail.error_count(), 0);

    let iwyu_cstring_strchr_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Find() {".to_string(),
            "  strchr(text, 'x');".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strchr_fail.error_count(), 0);

    let iwyu_cstring_strlen_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Count() {".to_string(),
            "  strlen(text);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strlen_fail.error_count(), 0);

    let iwyu_cstring_memmove_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Shift() {".to_string(),
            "  memmove(dst, src, size);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_memmove_fail.error_count(), 0);

    let iwyu_cstring_memset_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Clear() {".to_string(),
            "  memset(buf, 0, size);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_memset_fail.error_count(), 0);

    let iwyu_cstring_strcmp_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void CompareText() {".to_string(),
            "  strcmp(lhs, rhs);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strcmp_fail.error_count(), 0);

    let iwyu_cstring_strncmp_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void ComparePrefix() {".to_string(),
            "  strncmp(lhs, rhs, size);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strncmp_fail.error_count(), 0);

    let iwyu_cstring_strrchr_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void FindLast() {".to_string(),
            "  strrchr(text, '/');".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strrchr_fail.error_count(), 0);

    let iwyu_cstring_strstr_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void FindSubstr() {".to_string(),
            "  strstr(text, needle);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strstr_fail.error_count(), 0);

    let iwyu_cstring_strspn_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void PrefixSpan() {".to_string(),
            "  strspn(text, charset);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strspn_fail.error_count(), 0);

    let iwyu_cstring_strcspn_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void PrefixReject() {".to_string(),
            "  strcspn(text, reject);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strcspn_fail.error_count(), 0);

    let iwyu_cstring_strpbrk_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void FindAny() {".to_string(),
            "  strpbrk(text, charset);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strpbrk_fail.error_count(), 0);

    let iwyu_cstring_strerror_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Message() {".to_string(),
            "  strerror(errnum);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strerror_fail.error_count(), 0);

    let iwyu_cstring_strcoll_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void CompareLocale() {".to_string(),
            "  strcoll(lhs, rhs);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strcoll_fail.error_count(), 0);

    let iwyu_cstring_strxfrm_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Transform() {".to_string(),
            "  strxfrm(dst, src, size);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strxfrm_fail.error_count(), 0);

    let iwyu_cstring_strcpy_fail = run_lint_with_filter(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void CopyText() {".to_string(),
            "  strcpy(dst, src);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
        "-runtime/printf",
    );
    assert_eq!(iwyu_cstring_strcpy_fail.error_count(), 0);

    let iwyu_cstring_strncpy_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void CopyPrefix() {".to_string(),
            "  strncpy(dst, src, size);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strncpy_fail.error_count(), 0);

    let iwyu_cstring_strcat_fail = run_lint_with_filter(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Append() {".to_string(),
            "  strcat(dst, src);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
        "-runtime/printf",
    );
    assert_eq!(iwyu_cstring_strcat_fail.error_count(), 0);

    let iwyu_cstring_strncat_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void AppendPrefix() {".to_string(),
            "  strncat(dst, src, size);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstring_strncat_fail.error_count(), 0);

    let iwyu_cstdio_pass = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "#include <cstdio>".to_string(),
            "FILE* fp = nullptr;".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_pass.error_count(), 0);

    let iwyu_cstdio_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "FILE* fp = nullptr;".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fail.error_count(), 1);
    assert!(iwyu_cstdio_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));

    let iwyu_cstdio_printf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Print() {".to_string(),
            "  printf(\"%d\", value);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_printf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_printf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fopen_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Open() {".to_string(),
            "  fopen(path, \"r\");".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fopen_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fopen_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fclose_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Close() {".to_string(),
            "  fclose(fp);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fclose_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fclose_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fgets_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Read() {".to_string(),
            "  fgets(buf, sizeof(buf), fp);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fgets_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fgets_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fgetc_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Read() {".to_string(),
            "  fgetc(fp);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fgetc_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fgetc_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_perror_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Report() {".to_string(),
            "  perror(\"boom\");".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_perror_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_perror_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fprintf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Report() {".to_string(),
            "  fprintf(stderr, \"%d\", value);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fprintf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fprintf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_getc_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Read() {".to_string(),
            "  getc(fp);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_getc_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_getc_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_putc_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Write() {".to_string(),
            "  putc(ch, fp);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_putc_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_putc_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_puts_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Report() {".to_string(),
            "  puts(\"hello\");".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_puts_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_puts_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fputs_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Report() {".to_string(),
            "  fputs(msg, fp);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fputs_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fputs_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_scanf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Read() {".to_string(),
            "  scanf(\"%d\", &value);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_scanf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_scanf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_tmpfile_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Temp() {".to_string(),
            "  tmpfile();".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_tmpfile_fail.error_count(), 0);

    let iwyu_cstdio_tmpnam_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void TempName() {".to_string(),
            "  tmpnam(buf);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_tmpnam_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_tmpnam_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_vfprintf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Report() {".to_string(),
            "  vfprintf(stderr, fmt, args);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_vfprintf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_vfprintf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_vprintf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Report() {".to_string(),
            "  vprintf(fmt, args);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_vprintf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_vprintf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_getchar_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void f() {".to_string(),
            "  getchar();".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_getchar_fail.error_count(), 0);

    let iwyu_cstdio_putchar_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void f() {".to_string(),
            "  putchar('x');".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_putchar_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_putchar_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_vfscanf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void f(FILE* fp, const char* fmt, va_list args) {".to_string(),
            "  vfscanf(fp, fmt, args);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_vfscanf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_vfscanf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_vscanf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void f(const char* fmt, va_list args) {".to_string(),
            "  vscanf(fmt, args);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_vscanf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_vscanf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fflush_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Flush(FILE* fp) {".to_string(),
            "  fflush(fp);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fflush_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fflush_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fputc_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Put(FILE* fp) {".to_string(),
            "  fputc('x', fp);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fputc_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fputc_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_rewind_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Reset(FILE* fp) {".to_string(),
            "  rewind(fp);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_rewind_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_rewind_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_ungetc_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void PushBack(FILE* fp) {".to_string(),
            "  ungetc('x', fp);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_ungetc_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_ungetc_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fread_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Read(char* buf) {".to_string(),
            "  fread(buf, 1, 8, stream);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fread_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fread_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fwrite_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Write(const char* buf) {".to_string(),
            "  fwrite(buf, 1, 8, stream);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fwrite_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fwrite_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_freopen_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Redirect() {".to_string(),
            "  freopen(\"out.txt\", \"w\", stream);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_freopen_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_freopen_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_ftell_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Pos() {".to_string(),
            "  int pos = ftell(stream);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_ftell_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_ftell_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fseek_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Seek() {".to_string(),
            "  fseek(stream, 0, 0);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fseek_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fseek_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_feof_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void CheckEof() {".to_string(),
            "  feof(stream);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_feof_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_feof_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_ferror_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void CheckError() {".to_string(),
            "  ferror(stream);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_ferror_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_ferror_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_remove_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Delete() {".to_string(),
            "  remove(\"out.txt\");".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_remove_fail.error_count(), 0);

    let iwyu_cstdio_rename_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Move() {".to_string(),
            "  rename(\"a.txt\", \"b.txt\");".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_rename_fail.error_count(), 0);

    let iwyu_cstdio_clearerr_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void ResetError() {".to_string(),
            "  clearerr(stream);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_clearerr_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_clearerr_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fgetpos_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void GetPos() {".to_string(),
            "  fgetpos(stream, &pos);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fgetpos_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fgetpos_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fsetpos_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void SetPos() {".to_string(),
            "  fsetpos(stream, &pos);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fsetpos_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fsetpos_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_setbuf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Buffer() {".to_string(),
            "  setbuf(stream, buf);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_setbuf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_setbuf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_setvbuf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void BufferMode() {".to_string(),
            "  setvbuf(stream, buf, 0, 1024);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_setvbuf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_setvbuf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_fscanf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Scan() {".to_string(),
            "  fscanf(stream, \"%d\", &value);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_fscanf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_fscanf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_sscanf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Parse() {".to_string(),
            "  sscanf(text, \"%d\", &value);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_sscanf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_sscanf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_snprintf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void Format() {".to_string(),
            "  snprintf(buf, sizeof(buf), \"%d\", value);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_snprintf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_snprintf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_sprintf_fail = run_lint_with_filter(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void FormatUnsafe() {".to_string(),
            "  sprintf(buf, \"%d\", value);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
        "-runtime/printf",
    );
    assert_eq!(iwyu_cstdio_sprintf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_sprintf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_vsnprintf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void FormatArgs() {".to_string(),
            "  vsnprintf(buf, sizeof(buf), fmt, args);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_vsnprintf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_vsnprintf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );

    let iwyu_cstdio_vsscanf_fail = run_lint_with_filename(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void ParseArgs() {".to_string(),
            "  vsscanf(text, fmt, args);".to_string(),
            "}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(iwyu_cstdio_vsscanf_fail.error_count(), 1);
    assert!(
        iwyu_cstdio_vsscanf_fail
            .has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse)
    );
}

#[test]
fn test_redundant_virtual_and_override_cases() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "struct Base {".to_string(),
        "    virtual void F();".to_string(),
        "};".to_string(),
        "struct Derived : Base {".to_string(),
        "    void F() override;".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(pass_state.error_count(), 0);

    let virtual_override_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "struct Derived : Base {".to_string(),
        "    virtual void F() override;".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(virtual_override_fail.error_count(), 1);
    assert!(
        virtual_override_fail.has_error(cpplint_core::categories::Category::ReadabilityInheritance)
    );

    let multiline_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "struct Derived : Base {".to_string(),
        "    virtual void F()".to_string(),
        "        final;".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(multiline_fail.error_count(), 1);
    assert!(multiline_fail.has_error(cpplint_core::categories::Category::ReadabilityInheritance));

    let override_final_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "struct Derived : Base {".to_string(),
        "    void F() override final;".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(override_final_fail.error_count(), 1);
    assert!(
        override_final_fail.has_error(cpplint_core::categories::Category::ReadabilityInheritance)
    );
}

#[test]
fn test_function_size_cases() {
    fn long_body_lines(count: usize) -> Vec<String> {
        let mut lines = Vec::with_capacity(count);
        for _ in 0..count {
            lines.push("  Work();".to_string());
        }
        lines
    }

    let mut pass_lines = vec![
        "// Copyright 2026".to_string(),
        "void ShortFn(".to_string(),
        "    int value) {".to_string(),
    ];
    pass_lines.extend(long_body_lines(250));
    pass_lines.push("}".to_string());
    pass_lines.push("".to_string());
    let pass_state =
        run_lint_with_filters_and_verbose("foo.cc", pass_lines, &["-", "+readability/fn_size"], 0);
    assert_eq!(pass_state.error_count(), 0);

    let fail_state = run_lint_with_filters_and_verbose(
        "foo.cc",
        {
            let mut lines = vec![
                "// Copyright 2026".to_string(),
                "void LongFn(".to_string(),
                "    int value) {".to_string(),
            ];
            lines.extend(long_body_lines(251));
            lines.push("}".to_string());
            lines.push("".to_string());
            lines
        },
        &["-", "+readability/fn_size"],
        0,
    );
    assert_eq!(fail_state.error_count(), 1);
    assert!(fail_state.has_error(cpplint_core::categories::Category::ReadabilityFnSize));

    let no_start_fail = run_lint_with_filters_and_verbose(
        "foo.cc",
        vec![
            "// Copyright 2026".to_string(),
            "void LongFn(".to_string(),
            "    int value)".to_string(),
            "".to_string(),
        ],
        &["-", "+readability/fn_size"],
        0,
    );
    assert_eq!(no_start_fail.error_count(), 1);
    assert!(no_start_fail.has_error(cpplint_core::categories::Category::ReadabilityFnSize));

    let test_func_pass = run_lint_with_filters_and_verbose(
        "foo.cc",
        {
            let mut lines = vec!["// Copyright 2026".to_string(), "void TEST() {".to_string()];
            lines.extend(long_body_lines(400));
            lines.push("}".to_string());
            lines.push("".to_string());
            lines
        },
        &["-", "+readability/fn_size"],
        0,
    );
    assert_eq!(test_func_pass.error_count(), 0);

    let test_func_fail = run_lint_with_filters_and_verbose(
        "foo.cc",
        {
            let mut lines = vec!["// Copyright 2026".to_string(), "void TEST() {".to_string()];
            lines.extend(long_body_lines(401));
            lines.push("}".to_string());
            lines.push("".to_string());
            lines
        },
        &["-", "+readability/fn_size"],
        0,
    );
    assert_eq!(test_func_fail.error_count(), 1);
    assert!(test_func_fail.has_error(cpplint_core::categories::Category::ReadabilityFnSize));

    let test_prefix_pass = run_lint_with_filters_and_verbose(
        "foo.cc",
        {
            let mut lines = vec![
                "// Copyright 2026".to_string(),
                "void TestHelper() {".to_string(),
            ];
            lines.extend(long_body_lines(400));
            lines.push("}".to_string());
            lines.push("".to_string());
            lines
        },
        &["-", "+readability/fn_size"],
        0,
    );
    assert_eq!(test_prefix_pass.error_count(), 0);

    let test_prefix_fail = run_lint_with_filters_and_verbose(
        "foo.cc",
        {
            let mut lines = vec![
                "// Copyright 2026".to_string(),
                "void TestHelper() {".to_string(),
            ];
            lines.extend(long_body_lines(401));
            lines.push("}".to_string());
            lines.push("".to_string());
            lines
        },
        &["-", "+readability/fn_size"],
        0,
    );
    assert_eq!(test_prefix_fail.error_count(), 1);
    assert!(test_prefix_fail.has_error(cpplint_core::categories::Category::ReadabilityFnSize));
}

#[test]
fn test_explicit_single_parameter_constructor_cases() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "class Foo {".to_string(),
        " public:".to_string(),
        "  explicit Foo(int value);".to_string(),
        "  Foo(std::initializer_list< int > values);".to_string(),
        "  Foo(const Foo& other);".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(pass_state.error_count(), 0);

    let fail_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "class Foo {".to_string(),
        " public:".to_string(),
        "  Foo(int value);".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(fail_state.error_count(), 1);
    assert!(fail_state.has_error(cpplint_core::categories::Category::RuntimeExplicit));

    let callable_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "class Foo {".to_string(),
        " public:".to_string(),
        "  Foo(int value, int extra = 0);".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(callable_fail.error_count(), 1);
    assert!(callable_fail.has_error(cpplint_core::categories::Category::RuntimeExplicit));

    let template_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <vector>".to_string(),
        "class Foo {".to_string(),
        " public:".to_string(),
        "  Foo(const std::vector< int >& values = std::vector< int >());".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(template_fail.error_count(), 1);
    assert!(template_fail.has_error(cpplint_core::categories::Category::RuntimeExplicit));
}

#[test]
fn test_invalid_increment_and_deprecated_operator_cases() {
    let invalid_increment_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "(*count)++;".to_string(),
        "*count += 1;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(invalid_increment_pass.error_count(), 0);

    let invalid_increment_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "*count++;".to_string(),
        "*count--;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(invalid_increment_fail.error_count(), 2);
    assert!(
        invalid_increment_fail
            .has_error(cpplint_core::categories::Category::RuntimeInvalidIncrement)
    );

    let deprecated_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int v = std::max(a, b);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(deprecated_pass.error_count(), 1);
    assert!(deprecated_pass.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));

    let deprecated_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int v = a >? b;".to_string(),
        "int w = a <?= b;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(deprecated_fail.error_count(), 2);
    assert!(deprecated_fail.has_error(cpplint_core::categories::Category::BuildDeprecated));
}

#[test]
fn test_storage_class_and_forward_decl_cases() {
    let storage_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "static int value;".to_string(),
        "extern const int other;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(storage_pass.error_count(), 0);

    let storage_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int static value;".to_string(),
        "const extern int other;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(storage_fail.error_count(), 2);
    assert!(storage_fail.has_error(cpplint_core::categories::Category::BuildStorageClass));

    let forward_decl_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "class Foo;".to_string(),
        "namespace a { class Bar; }".to_string(),
        "".to_string(),
    ]);
    assert_eq!(forward_decl_pass.error_count(), 0);

    let forward_decl_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "class Foo::Bar;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(forward_decl_fail.error_count(), 1);
    assert!(forward_decl_fail.has_error(cpplint_core::categories::Category::BuildForwardDecl));
}

#[test]
fn test_endif_comment_and_member_string_reference_cases() {
    let endif_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#if defined(FOO)".to_string(),
        "#endif  // FOO".to_string(),
        "".to_string(),
    ]);
    assert_eq!(endif_pass.error_count(), 0);

    let endif_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#if defined(FOO)".to_string(),
        "#endif FOO".to_string(),
        "".to_string(),
    ]);
    assert_eq!(endif_fail.error_count(), 1);
    assert!(endif_fail.has_error(cpplint_core::categories::Category::BuildEndifComment));

    let member_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "class Foo {".to_string(),
        " public:".to_string(),
        "  const string* value;".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(member_pass.error_count(), 1);
    assert!(member_pass.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));

    let member_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "class Foo {".to_string(),
        " public:".to_string(),
        "  const string& value;".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(member_fail.error_count(), 2);
    assert!(member_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));
    assert!(
        member_fail.has_error(cpplint_core::categories::Category::RuntimeMemberStringReferences)
    );
}

#[test]
fn test_memset_and_threadsafe_function_cases() {
    let memset_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <cstring>".to_string(),
        "memset(buf, 0, sizeof(buf));".to_string(),
        "memset(buf, 0, 16);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(memset_pass.error_count(), 0);

    let memset_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <cstring>".to_string(),
        "memset(buf, sizeof(buf), 0);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(memset_fail.error_count(), 1);
    assert!(memset_fail.has_error(cpplint_core::categories::Category::RuntimeMemset));

    let threadsafe_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int rand(seed);".to_string(),
        "foo->rand();".to_string(),
        "value = rand_r(&seed);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(threadsafe_pass.error_count(), 0);

    let threadsafe_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int value = rand();".to_string(),
        "foo + localtime(&now);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(threadsafe_fail.error_count(), 2);
    assert!(threadsafe_fail.has_error(cpplint_core::categories::Category::RuntimeThreadsafeFn));
}

#[test]
fn test_vlog_and_explicit_make_pair_cases() {
    let vlog_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "VLOG(2) << value;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(vlog_pass.error_count(), 0);

    let vlog_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "VLOG(INFO) << value;".to_string(),
        "  VLOG(ERROR) << value;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(vlog_fail.error_count(), 2);
    assert!(vlog_fail.has_error(cpplint_core::categories::Category::RuntimeVlog));

    let make_pair_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "auto p = std::make_pair(a, b);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(make_pair_pass.error_count(), 1);
    assert!(make_pair_pass.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));

    let make_pair_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "auto p = std::make_pair< int, int >(a, b);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(make_pair_fail.error_count(), 2);
    assert!(make_pair_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));
    assert!(make_pair_fail.has_error(cpplint_core::categories::Category::BuildExplicitMakePair));
}

#[test]
fn test_unapproved_cxx_headers_cases() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <vector>".to_string(),
        "#include <string>".to_string(),
        "".to_string(),
    ]);
    assert_eq!(pass_state.error_count(), 0);

    let cxx11_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <ratio>".to_string(),
        "#include <cfenv>".to_string(),
        "".to_string(),
    ]);
    assert_eq!(cxx11_fail.error_count(), 2);
    assert!(cxx11_fail.has_error(cpplint_core::categories::Category::BuildCpp11));

    let cxx17_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <filesystem>".to_string(),
        "".to_string(),
    ]);
    assert_eq!(cxx17_fail.error_count(), 1);
    assert!(cxx17_fail.has_error(cpplint_core::categories::Category::BuildCpp17));
}

#[test]
fn test_global_string_cases() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "string* pointer;".to_string(),
        "const string* const_pointer;".to_string(),
        "string Function();".to_string(),
        "string Class::operator*();".to_string(),
        "".to_string(),
    ]);
    assert_eq!(pass_state.error_count(), 1);
    assert!(pass_state.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));

    let global_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "string value;".to_string(),
        "static string other;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(global_fail.error_count(), 3);
    assert!(global_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));
    assert!(global_fail.has_error(cpplint_core::categories::Category::RuntimeString));

    let const_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "const string kValue = \"hello\";".to_string(),
        "".to_string(),
    ]);
    assert_eq!(const_fail.error_count(), 2);
    assert!(const_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));
    assert!(const_fail.has_error(cpplint_core::categories::Category::RuntimeString));

    let multiline_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "static string value =".to_string(),
        "    \"hello\";".to_string(),
        "".to_string(),
    ]);
    assert_eq!(multiline_fail.error_count(), 2);
    assert!(multiline_fail.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));
    assert!(multiline_fail.has_error(cpplint_core::categories::Category::RuntimeString));
}

#[test]
fn test_selfinit_and_printf_format_cases() {
    let selfinit_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "Foo() : value_(other_) {}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(selfinit_pass.error_count(), 0);

    let selfinit_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "Foo() : value_(value_) {}".to_string(),
        "Foo() : value_(CHECK_NOTNULL(value_)) {}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(selfinit_fail.error_count(), 2);
    assert!(selfinit_fail.has_error(cpplint_core::categories::Category::RuntimeInit));

    let printf_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <cstdio>".to_string(),
        "printf(\"%lld\", value);".to_string(),
        "printf(\"100% done\");".to_string(),
        "".to_string(),
    ]);
    assert_eq!(printf_pass.error_count(), 0);

    let printf_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <cstdio>".to_string(),
        "printf(\"%q\", value);".to_string(),
        "printf(\"%2$d\", value);".to_string(),
        "printf(\"\\%\", value);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(printf_fail.error_count(), 3);
    assert!(printf_fail.has_error(cpplint_core::categories::Category::RuntimePrintfFormat));
}

#[test]
fn test_printf_cases() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <cstdio>".to_string(),
        "snprintf(buf, 0, \"%s\", value);".to_string(),
        "snprintf(buf, sizeof(buf), \"%s\", value);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(pass_state.error_count(), 0);

    let snprintf_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <cstdio>".to_string(),
        "snprintf(buf, 32, \"%s\", value);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(snprintf_fail.error_count(), 1);
    assert!(snprintf_fail.has_error(cpplint_core::categories::Category::RuntimePrintf));

    let sprintf_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <cstdio>".to_string(),
        "sprintf(buf, \"%s\", value);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(sprintf_fail.error_count(), 1);
    assert!(sprintf_fail.has_error(cpplint_core::categories::Category::RuntimePrintf));

    let strfunc_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include <cstdio>".to_string(),
        "#include <cstring>".to_string(),
        "strcpy(buf, src);".to_string(),
        "strcat(buf, src);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(strfunc_fail.error_count(), 2);
    assert!(strfunc_fail.has_error(cpplint_core::categories::Category::RuntimePrintf));
}

#[test]
fn test_deprecated_operator_ampersand_case() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "class X {".to_string(),
        " public:".to_string(),
        "  int operator&(const X& x);".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(pass_state.error_count(), 0);

    let fail_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "class X {".to_string(),
        " public:".to_string(),
        "  int operator&();".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(fail_state.error_count(), 1);
    assert!(fail_state.has_error(cpplint_core::categories::Category::RuntimeOperator));
}

#[test]
fn test_runtime_int_cases() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "unsigned short port = 80;".to_string(),
        "long double precise = 1.0;".to_string(),
        "int16_t small = 1;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(pass_state.error_count(), 0);

    let port_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "short port = 80;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(port_fail.error_count(), 1);
    assert!(port_fail.has_error(cpplint_core::categories::Category::RuntimeInt));

    let decl_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "short value = 1;".to_string(),
        "long total = 2;".to_string(),
        "long long huge = 3;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(decl_fail.error_count(), 3);
    assert!(decl_fail.has_error(cpplint_core::categories::Category::RuntimeInt));
}

#[test]
fn test_runtime_references_cases() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "void Func(const Foo& x, Bar const& y);".to_string(),
        "void swap(Foo& a, Foo& b);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(pass_state.error_count(), 1);
    assert!(pass_state.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));

    let fail_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "void foo(Foo& s, Foo& f);".to_string(),
        "void foo(Bar*& p);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(fail_state.error_count(), 3);
    assert!(fail_state.has_error(cpplint_core::categories::Category::RuntimeReferences));
}

#[test]
fn test_runtime_arrays_cases() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int fixed[kBufferSize];".to_string(),
        "int small[4];".to_string(),
        "int bytes[sizeof(foo)];".to_string(),
        "int half[(arraysize(fixed_size_array)/2) << 1];".to_string(),
        "return a[some_var];".to_string(),
        "".to_string(),
    ]);
    assert_eq!(pass_state.error_count(), 0);

    let fail_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int a[any_old_variable];".to_string(),
        "int doublesize[some_var * 2];".to_string(),
        "namespace::Type buffer[len+1];".to_string(),
        "".to_string(),
    ]);
    assert_eq!(fail_state.error_count(), 3);
    assert!(fail_state.has_error(cpplint_core::categories::Category::RuntimeArrays));
}

#[test]
fn test_namespace_using_and_header_namespace_cases() {
    let namespace_using_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "using foo;".to_string(),
        "using std::literals;".to_string(),
        "using std::literals::chrono_literals;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(namespace_using_pass.error_count(), 0);

    let namespace_using_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "using namespace foo;".to_string(),
        "using namespace std::literals;".to_string(),
        "using namespace std::literals::chrono_literals;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(namespace_using_fail.error_count(), 3);
    assert!(namespace_using_fail.has_error(cpplint_core::categories::Category::BuildNamespaces));
    assert!(
        namespace_using_fail.has_error(cpplint_core::categories::Category::BuildNamespacesLiterals)
    );

    let header_namespace_pass = run_lint_with_filename(
        "test.cc",
        vec![
            "// Copyright 2026".to_string(),
            "namespace {}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(header_namespace_pass.error_count(), 0);

    let header_namespace_fail = run_lint_with_filename(
        "test.h",
        vec![
            "// Copyright 2026".to_string(),
            "#pragma once".to_string(),
            "namespace {}".to_string(),
            "".to_string(),
        ],
    );
    assert_eq!(header_namespace_fail.error_count(), 1);
    assert!(
        header_namespace_fail.has_error(cpplint_core::categories::Category::BuildNamespacesHeaders)
    );
}

#[test]
fn test_alt_tokens_cases() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#include \"base/false-and-false.h\"".to_string(),
        "true nand true;".to_string(),
        "true nor true;".to_string(),
        "#error false or false".to_string(),
        "".to_string(),
    ]);
    assert_eq!(pass_state.error_count(), 0);

    let fail_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "true or true;".to_string(),
        "true and true;".to_string(),
        "if (not true) return;".to_string(),
        "1 bitor 1;".to_string(),
        "1 xor 1;".to_string(),
        "x = compl 1;".to_string(),
        "x and_eq y;".to_string(),
        "x or_eq y;".to_string(),
        "x xor_eq y;".to_string(),
        "x not_eq y;".to_string(),
        "if (true and(foo)) return;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(fail_state.error_count(), 11);
    assert!(fail_state.has_error(cpplint_core::categories::Category::ReadabilityAltTokens));

    let multiple_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (true or true and (not true)) return;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(multiple_fail.error_count(), 3);
    assert!(multiple_fail.has_error(cpplint_core::categories::Category::ReadabilityAltTokens));
}

#[test]
fn test_nolint_cases() {
    let block_pass = run_lint(vec![
        "// Copyright 2026".to_string(),
        "// NOLINTBEGIN(build/include)".to_string(),
        "// NOLINTEND".to_string(),
        "".to_string(),
    ]);
    assert_eq!(block_pass.error_count(), 0);

    let block_suppress_all = run_lint(vec![
        "// Copyright 2026".to_string(),
        "// NOLINTBEGIN".to_string(),
        "long a = (int64_t) 65;".to_string(),
        "long a = (int64_t) 65;".to_string(),
        "// NOLINTEND".to_string(),
        "".to_string(),
    ]);
    assert_eq!(block_suppress_all.error_count(), 0);

    let no_end_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "// NOLINTBEGIN(build/include)".to_string(),
        "".to_string(),
    ]);
    assert_eq!(no_end_fail.error_count(), 1);
    assert!(no_end_fail.has_error(cpplint_core::categories::Category::ReadabilityNolint));

    let no_begin_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "// NOLINTEND".to_string(),
        "".to_string(),
    ]);
    assert_eq!(no_begin_fail.error_count(), 1);
    assert!(no_begin_fail.has_error(cpplint_core::categories::Category::ReadabilityNolint));

    let block_defined_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "// NOLINTBEGIN(build/include)".to_string(),
        "// NOLINTBEGIN(build/include)".to_string(),
        "// NOLINTEND".to_string(),
        "".to_string(),
    ]);
    assert_eq!(block_defined_fail.error_count(), 1);
    assert!(block_defined_fail.has_error(cpplint_core::categories::Category::ReadabilityNolint));

    let end_with_category_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "// NOLINTBEGIN(build/include)".to_string(),
        "// NOLINTEND(build/include)".to_string(),
        "".to_string(),
    ]);
    assert_eq!(end_with_category_fail.error_count(), 1);
    assert!(
        end_with_category_fail.has_error(cpplint_core::categories::Category::ReadabilityNolint)
    );

    let unknown_category_fail = run_lint(vec![
        "// Copyright 2026".to_string(),
        "// NOLINT(unknown/category)".to_string(),
        "".to_string(),
    ]);
    assert_eq!(unknown_category_fail.error_count(), 1);
    assert!(unknown_category_fail.has_error(cpplint_core::categories::Category::ReadabilityNolint));

    let line_suppress_one = run_lint(vec![
        "// Copyright 2026".to_string(),
        "long a = (int64_t) 65;  // NOLINT(runtime/int)".to_string(),
        "".to_string(),
    ]);
    assert_eq!(line_suppress_one.error_count(), 1);
    assert!(line_suppress_one.has_error(cpplint_core::categories::Category::ReadabilityCasting));

    let next_line_suppress = run_lint(vec![
        "// Copyright 2026".to_string(),
        "// NOLINTNEXTLINE".to_string(),
        "long a = (int64_t) 65;".to_string(),
        "long a = (int64_t) 65;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(next_line_suppress.error_count(), 2);
    assert!(next_line_suppress.has_error(cpplint_core::categories::Category::ReadabilityCasting));
    assert!(next_line_suppress.has_error(cpplint_core::categories::Category::RuntimeInt));
}

#[test]
fn test_global_suppression_cases() {
    let lint_c_file = run_lint_with_filters(
        "test.cpp",
        vec![
            "// Copyright 2026".to_string(),
            "// LINT_C_FILE".to_string(),
            "long a = (int64_t) 65;".to_string(),
            "long a = (int64_t) 65;".to_string(),
            "".to_string(),
        ],
        &["-build/include_what_you_use"],
    );
    assert_eq!(lint_c_file.error_count(), 2);
    assert!(lint_c_file.has_error(cpplint_core::categories::Category::RuntimeInt));

    let vim_mode_c = run_lint_with_filters(
        "test.cpp",
        vec![
            "// Copyright 2026".to_string(),
            "// vim: sw=8 filetype=c ts=8".to_string(),
            "long a = (int64_t) 65;".to_string(),
            "long a = (int64_t) 65;".to_string(),
            "".to_string(),
        ],
        &["-build/include_what_you_use"],
    );
    assert_eq!(vim_mode_c.error_count(), 2);
    assert!(vim_mode_c.has_error(cpplint_core::categories::Category::RuntimeInt));

    let lint_kernel_file = run_lint_with_filters(
        "test.cpp",
        vec![
            "// Copyright 2026".to_string(),
            "// LINT_KERNEL_FILE".to_string(),
            "\t\tint a = 0;".to_string(),
            "\t\tlong a = (int64_t) 65;".to_string(),
            "".to_string(),
        ],
        &["-build/include_what_you_use"],
    );
    assert_eq!(lint_kernel_file.error_count(), 2);
    assert!(lint_kernel_file.has_error(cpplint_core::categories::Category::ReadabilityCasting));
    assert!(lint_kernel_file.has_error(cpplint_core::categories::Category::RuntimeInt));
}

#[test]
fn test_runtime_casting_cases() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "Compute(arg, &(*func_ptr)(i, j));".to_string(),
        "".to_string(),
    ]);
    assert_eq!(pass_state.error_count(), 0);

    let fail_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int* x = &(int*)foo;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(fail_state.error_count(), 1);
    assert!(fail_state.has_error(cpplint_core::categories::Category::RuntimeCasting));
}

#[test]
fn test_indent_odd_pass_cases() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "  int two_space;".to_string(),
        "    int four_space;".to_string(),
        " public:".to_string(),
        "   private:".to_string(),
        " protected: \\".to_string(),
        "    int a;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(state.error_count(), 0);
    assert!(!state.has_error(cpplint_core::categories::Category::WhitespaceIndent));
}

#[test]
fn test_indent_odd_fail_cases() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        " int one_space;".to_string(),
        "   int three_space;".to_string(),
        " char* one_space = \"public:\";".to_string(),
        "".to_string(),
    ]);
    assert_eq!(state.error_count(), 3);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceIndent));
}

#[test]
fn test_end_of_line_spacing() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "int foo; ".to_string(),
        "// Hello there  ".to_string(),
        "".to_string(),
    ]);
    assert_eq!(state.error_count(), 2);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceEndOfLine));
}

#[test]
fn test_line_length_pass_cases() {
    let normal_line = format!("// x  {}", "xx".repeat(37));
    let utf_3b_line = format!("// x  {}", "あ".repeat(37));
    let utf_4b_line = format!("// x  {}", "😀".repeat(37));
    let utf_cb_line = format!("// x  {}", "ÀÀ".repeat(37));
    let path_line = format!("// //some/path/to/f{}", "ile".repeat(50));
    let url_line = format!("// Read http://g{}gle.com/", "oo".repeat(50));

    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        normal_line,
        utf_3b_line,
        utf_4b_line,
        utf_cb_line,
        path_line,
        url_line,
        "".to_string(),
    ]);
    assert_eq!(state.error_count(), 0);
    assert!(!state.has_error(cpplint_core::categories::Category::WhitespaceLineLength));
}

#[test]
fn test_line_length_fail_cases() {
    let normal_line = format!("// x  {}", "xx".repeat(38));
    let utf_3b_line = format!("// x  {}", "あ".repeat(38));
    let utf_4b_line = format!("// x  {}", "😀".repeat(38));
    let utf_cb_line = format!("// x  {}", "ÀÀ".repeat(38));
    let path_line = format!("// //some/path/to/f{} and comment", "ile".repeat(50));
    let url_line = format!("// Read http://g{}gle.com/ and comment", "oo".repeat(50));

    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        normal_line,
        utf_3b_line,
        utf_4b_line,
        utf_cb_line,
        path_line,
        url_line,
        "".to_string(),
    ]);
    assert_eq!(state.error_count(), 6);
    assert!(state.has_error(cpplint_core::categories::Category::WhitespaceLineLength));
}

#[test]
fn test_multiple_commands_pass_and_fail() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "switch (x) {".to_string(),
        "     case 0: func(); break;".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert!(!pass_state.has_error(cpplint_core::categories::Category::WhitespaceNewline));

    let fail_state = run_lint_with_verbose(
        "test.cpp",
        vec![
            "// Copyright 2026".to_string(),
            "int foo; int bar;".to_string(),
            "".to_string(),
        ],
        0,
    );
    assert_eq!(fail_state.error_count(), 1);
    assert!(fail_state.has_error(cpplint_core::categories::Category::WhitespaceNewline));
}

#[test]
fn test_empty_block_body_cases() {
    let conditional_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (true);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(conditional_state.error_count(), 1);
    assert!(
        conditional_state
            .has_error(cpplint_core::categories::Category::WhitespaceEmptyConditionalBody)
    );

    let loop_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "while (true);".to_string(),
        "for (;;);".to_string(),
        "".to_string(),
    ]);
    assert_eq!(loop_state.error_count(), 2);
    assert!(loop_state.has_error(cpplint_core::categories::Category::WhitespaceEmptyLoopBody));

    let empty_if_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (test,".to_string(),
        "    func({})) {".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(empty_if_state.error_count(), 1);
    assert!(empty_if_state.has_error(cpplint_core::categories::Category::WhitespaceEmptyIfBody));
}

#[test]
fn test_namespace_indentation_pass_and_fail() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "namespace test {".to_string(),
        "int a = 0;".to_string(),
        "void func {".to_string(),
        "    int b = 0;".to_string(),
        "}".to_string(),
        "#define macro \\".to_string(),
        "    do { \\".to_string(),
        "        something(); \\".to_string(),
        "    } while (0)".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert!(!pass_state.has_error(cpplint_core::categories::Category::WhitespaceIndentNamespace));

    let fail_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "namespace test {".to_string(),
        "    int a = 0;".to_string(),
        "void func {".to_string(),
        "    int b = 0;".to_string(),
        "}".to_string(),
        "#define macro \\".to_string(),
        "    do { \\".to_string(),
        "        something(); \\".to_string(),
        "    } while (0)".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(fail_state.error_count(), 1);
    assert!(fail_state.has_error(cpplint_core::categories::Category::WhitespaceIndentNamespace));

    let indented_namespace_decl_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#if defined(TEST)".to_string(),
        "    namespace test {".to_string(),
        "    int a = 0;".to_string(),
        "}".to_string(),
        "#endif".to_string(),
        "".to_string(),
    ]);
    assert!(
        indented_namespace_decl_state
            .has_error(cpplint_core::categories::Category::WhitespaceIndentNamespace)
    );
}

#[test]
fn test_trailing_semicolon_pass_and_fail() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "for (;;) {}".to_string(),
        "func = []() {".to_string(),
        "    func();".to_string(),
        "};".to_string(),
        "file_tocs_[i] = (FileToc) {a, b, c};".to_string(),
        "template<typename T>".to_string(),
        "concept C = requires(T a, T b) {".to_string(),
        "    requires a == b;".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert!(!pass_state.has_error(cpplint_core::categories::Category::ReadabilityBraces));

    let fail_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "for (;;) {};".to_string(),
        "while (foo) {};".to_string(),
        "switch (foo) {};".to_string(),
        "Function() {};".to_string(),
        "if (foo) {".to_string(),
        "    hello;".to_string(),
        "};".to_string(),
        "".to_string(),
    ]);
    assert_eq!(fail_state.error_count(), 5);
    assert!(fail_state.has_error(cpplint_core::categories::Category::ReadabilityBraces));
}

#[test]
fn test_multiline_comment_cases() {
    let complex_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "".to_string(),
        "int a = 0; /*".to_string(),
        "".to_string(),
        "*/".to_string(),
        "".to_string(),
    ]);
    assert_eq!(complex_state.error_count(), 1);
    assert!(
        complex_state.has_error(cpplint_core::categories::Category::ReadabilityMultilineComment)
    );

    let unclosed_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "/* This should be removed".to_string(),
        "".to_string(),
    ]);
    assert_eq!(unclosed_state.error_count(), 2);
    assert!(
        unclosed_state.has_error(cpplint_core::categories::Category::ReadabilityMultilineComment)
    );
}

#[test]
fn test_brace_else_indent_cases() {
    let state1 = run_lint_with_verbose(
        "test.cpp",
        vec![
            "// Copyright 2026".to_string(),
            "if (test)".to_string(),
            "    if (foo)".to_string(),
            "        int a = 0;".to_string(),
            "    else".to_string(),
            "        int a = 0;".to_string(),
            "".to_string(),
        ],
        4,
    );
    assert!(state1.has_error(cpplint_core::categories::Category::ReadabilityBraces));

    let state2 = run_lint_with_verbose(
        "test.cpp",
        vec![
            "// Copyright 2026".to_string(),
            "if (test)".to_string(),
            "    if (foo)".to_string(),
            "        int a = 0;".to_string(),
            "else".to_string(),
            "    int a = 0;".to_string(),
            "".to_string(),
        ],
        4,
    );
    assert!(state2.has_error(cpplint_core::categories::Category::ReadabilityBraces));
}

#[test]
fn test_multiline_string_cases() {
    let pass_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "char test[] = \"multiline\"".to_string(),
        "\"test\";".to_string(),
        "".to_string(),
    ]);
    assert_eq!(pass_state.error_count(), 0);
    assert!(!pass_state.has_error(cpplint_core::categories::Category::ReadabilityMultilineString));

    let fail_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "char test[] = \"multiline".to_string(),
        "test\";".to_string(),
        "".to_string(),
    ]);
    assert_eq!(fail_state.error_count(), 2);
    assert!(fail_state.has_error(cpplint_core::categories::Category::ReadabilityMultilineString));
}

#[test]
fn test_brace_else_one_side_cases() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "void func() {".to_string(),
        "    if (true)".to_string(),
        "        int a = 0;".to_string(),
        "    } else if (true) {".to_string(),
        "        int a = 0;".to_string(),
        "    } else".to_string(),
        "        int a = 0;".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(state.error_count(), 1);
    assert!(state.has_error(cpplint_core::categories::Category::ReadabilityBraces));
}

#[test]
fn test_brace_linefeed_cases() {
    let if_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "void func() {".to_string(),
        "    if (true)".to_string(),
        "    {".to_string(),
        "        int a = 0;".to_string(),
        "    } else if (true) {".to_string(),
        "        int a = 0;".to_string(),
        "    } else {".to_string(),
        "        int a = 0;".to_string(),
        "    }".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(if_state.error_count(), 1);
    assert!(if_state.has_error(cpplint_core::categories::Category::WhitespaceBraces));

    let elseif_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "void func() {".to_string(),
        "    if (true)".to_string(),
        "        int a = 0;".to_string(),
        "    else if (true)".to_string(),
        "    {".to_string(),
        "        int a = 0;".to_string(),
        "    } else {".to_string(),
        "        int a = 0;".to_string(),
        "    }".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(elseif_state.error_count(), 1);
    assert!(elseif_state.has_error(cpplint_core::categories::Category::WhitespaceBraces));

    let else_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "void func() {".to_string(),
        "    if (true)".to_string(),
        "        int a = 0;".to_string(),
        "    else if (true)".to_string(),
        "        int a = 0;".to_string(),
        "    else".to_string(),
        "    {".to_string(),
        "        int a = 0;".to_string(),
        "    }".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(else_state.error_count(), 1);
    assert!(else_state.has_error(cpplint_core::categories::Category::WhitespaceBraces));
}

#[test]
fn test_else_same_line_and_one_side_cases() {
    let newline_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "void func() {".to_string(),
        "    if (true) {".to_string(),
        "        int a = 0;".to_string(),
        "    }".to_string(),
        "    else if (true)".to_string(),
        "        int a = 0;".to_string(),
        "    else".to_string(),
        "        int a = 0;".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(newline_state.error_count(), 1);
    assert!(newline_state.has_error(cpplint_core::categories::Category::WhitespaceNewline));

    let one_side_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "void func() {".to_string(),
        "    if (true)".to_string(),
        "        int a = 0;".to_string(),
        "    } else if (true)".to_string(),
        "        int a = 0;".to_string(),
        "    else".to_string(),
        "        int a = 0;".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert_eq!(one_side_state.error_count(), 1);
    assert!(one_side_state.has_error(cpplint_core::categories::Category::ReadabilityBraces));
}

#[test]
fn test_controlled_statements_in_braces_cases() {
    let if_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (test) { hello; }".to_string(),
        "".to_string(),
    ]);
    assert_eq!(if_state.error_count(), 1);
    assert!(if_state.has_error(cpplint_core::categories::Category::WhitespaceNewline));

    let else_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (test) {".to_string(),
        "    int a = 0;".to_string(),
        "} else { hello; }".to_string(),
        "".to_string(),
    ]);
    assert_eq!(else_state.error_count(), 1);
    assert!(else_state.has_error(cpplint_core::categories::Category::WhitespaceNewline));
}

#[test]
fn test_brace_if_multiline_case() {
    let state1 = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (test)".to_string(),
        "    int a = 0;".to_string(),
        "    int a = 0;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(state1.error_count(), 1);
    assert!(state1.has_error(cpplint_core::categories::Category::ReadabilityBraces));
}

#[test]
fn test_brace_if_multiline_same_line_case() {
    let state2 = run_lint_with_verbose(
        "test.cpp",
        vec![
            "// Copyright 2026".to_string(),
            "if (test)".to_string(),
            "    int a = 0; int a = 0;".to_string(),
            "".to_string(),
        ],
        0,
    );
    assert_eq!(state2.error_count(), 2);
    assert!(state2.has_error(cpplint_core::categories::Category::ReadabilityBraces));
    assert!(state2.has_error(cpplint_core::categories::Category::WhitespaceNewline));
}

#[test]
fn test_brace_else_multiline_case() {
    let state3 = run_lint(vec![
        "// Copyright 2026".to_string(),
        "if (test)".to_string(),
        "    int a = 0;".to_string(),
        "else".to_string(),
        "    int a = 0;".to_string(),
        "    int a = 0;".to_string(),
        "".to_string(),
    ]);
    assert_eq!(state3.error_count(), 1);
    assert!(state3.has_error(cpplint_core::categories::Category::ReadabilityBraces));
}

#[test]
fn test_brace_else_macro_multiline_cases() {
    let after_block_state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#define TEST_NO_THROW(statement, fail) \\".to_string(),
        "  if (AlwaysTrue()) { \\".to_string(),
        "    try { \\".to_string(),
        "      statement; \\".to_string(),
        "    } catch (...) { \\".to_string(),
        "      fail(); \\".to_string(),
        "    } \\".to_string(),
        "  } else \\".to_string(),
        "    fail( \\".to_string(),
        "        \"Expected\", \\".to_string(),
        "        \"Actual\")".to_string(),
        "int terminator;".to_string(),
        "".to_string(),
    ]);
    assert!(after_block_state.has_error(cpplint_core::categories::Category::ReadabilityBraces));
}

#[test]
fn test_namespace_termination_comment_missing_in_macro_case() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "#define TYPED_TEST_P(SuiteName, TestName) \\".to_string(),
        "  namespace GTEST_SUITE_NAMESPACE_(SuiteName) { \\".to_string(),
        "  template <typename gtest_TypeParam_> \\".to_string(),
        "  class GTEST_TEST_CLASS_NAME_(SuiteName, TestName) { \\".to_string(),
        "   public: \\".to_string(),
        "    void TestBody() override; \\".to_string(),
        "  }; \\".to_string(),
        "  [[maybe_unused]] static bool gtest_##TestName##_defined_ = \\".to_string(),
        "      GTEST_TYPED_TEST_SUITE_P_STATE_(SuiteName).AddTestName( \\".to_string(),
        "          __FILE__, __LINE__, GTEST_STRINGIFY_(SuiteName), \\".to_string(),
        "          GTEST_STRINGIFY_(TestName)); \\".to_string(),
        "  } \\".to_string(),
        "  template <typename gtest_TypeParam_> \\".to_string(),
        "  void GTEST_SUITE_NAMESPACE_(SuiteName)::TestName<gtest_TypeParam_>::TestBody()"
            .to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::ReadabilityNamespace));
}

#[test]
fn test_distributions_line_246() {
    let state = run_lint(vec![
        "// Copyright 2026".to_string(),
        "void foo() {".to_string(),
        "  std::vector<Real> x;".to_string(),
        "}".to_string(),
        "".to_string(),
    ]);
    assert!(state.has_error(cpplint_core::categories::Category::BuildIncludeWhatYouUse));
}
