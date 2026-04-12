/// The explicit list of error categories.
pub const ERROR_CATEGORIES: &[&str] = &[
    "build/c++11",
    "build/c++17",
    "build/deprecated",
    "build/endif_comment",
    "build/explicit_make_pair",
    "build/forward_decl",
    "build/header_guard",
    "build/include",
    "build/include_subdir",
    "build/include_alpha",
    "build/include_order",
    "build/include_what_you_use",
    "build/namespaces_headers",
    "build/namespaces_literals",
    "build/namespaces",
    "build/printf_format",
    "build/storage_class",
    "legal/copyright",
    "readability/alt_tokens",
    "readability/braces",
    "readability/casting",
    "readability/check",
    "readability/constructors",
    "readability/fn_size",
    "readability/inheritance",
    "readability/multiline_comment",
    "readability/multiline_string",
    "readability/namespace",
    "readability/nolint",
    "readability/nul",
    "readability/strings",
    "readability/todo",
    "readability/utf8",
    "runtime/arrays",
    "runtime/casting",
    "runtime/explicit",
    "runtime/int",
    "runtime/init",
    "runtime/invalid_increment",
    "runtime/member_string_references",
    "runtime/memset",
    "runtime/operator",
    "runtime/printf",
    "runtime/printf_format",
    "runtime/references",
    "runtime/string",
    "runtime/threadsafe_fn",
    "runtime/vlog",
    "whitespace/blank_line",
    "whitespace/braces",
    "whitespace/comma",
    "whitespace/comments",
    "whitespace/empty_conditional_body",
    "whitespace/empty_if_body",
    "whitespace/empty_loop_body",
    "whitespace/end_of_line",
    "whitespace/ending_newline",
    "whitespace/forcolon",
    "whitespace/indent",
    "whitespace/indent_namespace",
    "whitespace/line_length",
    "whitespace/newline",
    "whitespace/operators",
    "whitespace/parens",
    "whitespace/semicolon",
    "whitespace/tab",
    "whitespace/todo",
];

/// Error categories no longer enforced, but kept for backwards compatibility in NOLINT.
pub const LEGACY_ERROR_CATEGORIES: &[&str] =
    &["build/class", "readability/streams", "readability/function"];

/// Prefixes for categories from other tools (e.g., clang-tidy) that should be ignored in NOLINT.
pub const OTHER_NOLINT_CATEGORY_PREFIXES: &[&str] = &[
    "clang-analyzer-",
    "abseil-",
    "altera-",
    "android-",
    "boost-",
    "bugprone-",
    "cert-",
    "concurrency-",
    "cppcoreguidelines-",
    "darwin-",
    "fuchsia-",
    "google-",
    "hicpp-",
    "linuxkernel-",
    "llvm-",
    "llvmlibc-",
    "misc-",
    "modernize-",
    "mpi-",
    "objc-",
    "openmp-",
    "performance-",
    "portability-",
    "readability-",
    "zircon-",
];

/// Returns true if the category is a valid error category.
pub fn is_error_category(category: &str) -> bool {
    ERROR_CATEGORIES.contains(&category)
}

/// Returns true if the category is a legacy error category.
pub fn is_legacy_error_category(category: &str) -> bool {
    LEGACY_ERROR_CATEGORIES.contains(&category)
}

/// Returns true if the category is from another tool (based on prefix).
pub fn is_other_nolint_category(category: &str) -> bool {
    OTHER_NOLINT_CATEGORY_PREFIXES
        .iter()
        .any(|prefix| category.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_error_category() {
        assert!(is_error_category("build/include"));
        assert!(!is_error_category("invalid/category"));
    }

    #[test]
    fn test_is_other_nolint_category() {
        assert!(is_other_nolint_category("clang-analyzer-dead-store"));
        assert!(!is_other_nolint_category("readability/braces"));
    }
}
