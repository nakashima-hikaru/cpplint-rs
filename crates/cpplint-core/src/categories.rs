use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    BuildCpp11,
    BuildCpp17,
    BuildDeprecated,
    BuildEndifComment,
    BuildExplicitMakePair,
    BuildForwardDecl,
    BuildHeaderGuard,
    BuildInclude,
    BuildIncludeSubdir,
    BuildIncludeAlpha,
    BuildIncludeOrder,
    BuildIncludeWhatYouUse,
    BuildNamespacesHeaders,
    BuildNamespacesLiterals,
    BuildNamespaces,
    BuildPrintfFormat,
    BuildStorageClass,
    LegalCopyright,
    ReadabilityAltTokens,
    ReadabilityBraces,
    ReadabilityCasting,
    ReadabilityCheck,
    ReadabilityConstructors,
    ReadabilityFnSize,
    ReadabilityInheritance,
    ReadabilityMultilineComment,
    ReadabilityMultilineString,
    ReadabilityNamespace,
    ReadabilityNolint,
    ReadabilityNul,
    ReadabilityStrings,
    ReadabilityTodo,
    ReadabilityUtf8,
    RuntimeArrays,
    RuntimeCasting,
    RuntimeExplicit,
    RuntimeInt,
    RuntimeInit,
    RuntimeInvalidIncrement,
    RuntimeMemberStringReferences,
    RuntimeMemset,
    RuntimeOperator,
    RuntimePrintf,
    RuntimePrintfFormat,
    RuntimeReferences,
    RuntimeString,
    RuntimeThreadsafeFn,
    RuntimeVlog,
    WhitespaceBlankLine,
    WhitespaceBraces,
    WhitespaceComma,
    WhitespaceComments,
    WhitespaceEmptyConditionalBody,
    WhitespaceEmptyIfBody,
    WhitespaceEmptyLoopBody,
    WhitespaceEndOfLine,
    WhitespaceEndingNewline,
    WhitespaceForcolon,
    WhitespaceIndent,
    WhitespaceIndentNamespace,
    WhitespaceLineLength,
    WhitespaceNewline,
    WhitespaceOperators,
    WhitespaceParens,
    WhitespaceSemicolon,
    WhitespaceTab,
    WhitespaceTodo,
}

impl Category {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::BuildCpp11 => "build/c++11",
            Self::BuildCpp17 => "build/c++17",
            Self::BuildDeprecated => "build/deprecated",
            Self::BuildEndifComment => "build/endif_comment",
            Self::BuildExplicitMakePair => "build/explicit_make_pair",
            Self::BuildForwardDecl => "build/forward_decl",
            Self::BuildHeaderGuard => "build/header_guard",
            Self::BuildInclude => "build/include",
            Self::BuildIncludeSubdir => "build/include_subdir",
            Self::BuildIncludeAlpha => "build/include_alpha",
            Self::BuildIncludeOrder => "build/include_order",
            Self::BuildIncludeWhatYouUse => "build/include_what_you_use",
            Self::BuildNamespacesHeaders => "build/namespaces_headers",
            Self::BuildNamespacesLiterals => "build/namespaces_literals",
            Self::BuildNamespaces => "build/namespaces",
            Self::BuildPrintfFormat => "build/printf_format",
            Self::BuildStorageClass => "build/storage_class",
            Self::LegalCopyright => "legal/copyright",
            Self::ReadabilityAltTokens => "readability/alt_tokens",
            Self::ReadabilityBraces => "readability/braces",
            Self::ReadabilityCasting => "readability/casting",
            Self::ReadabilityCheck => "readability/check",
            Self::ReadabilityConstructors => "readability/constructors",
            Self::ReadabilityFnSize => "readability/fn_size",
            Self::ReadabilityInheritance => "readability/inheritance",
            Self::ReadabilityMultilineComment => "readability/multiline_comment",
            Self::ReadabilityMultilineString => "readability/multiline_string",
            Self::ReadabilityNamespace => "readability/namespace",
            Self::ReadabilityNolint => "readability/nolint",
            Self::ReadabilityNul => "readability/nul",
            Self::ReadabilityStrings => "readability/strings",
            Self::ReadabilityTodo => "readability/todo",
            Self::ReadabilityUtf8 => "readability/utf8",
            Self::RuntimeArrays => "runtime/arrays",
            Self::RuntimeCasting => "runtime/casting",
            Self::RuntimeExplicit => "runtime/explicit",
            Self::RuntimeInt => "runtime/int",
            Self::RuntimeInit => "runtime/init",
            Self::RuntimeInvalidIncrement => "runtime/invalid_increment",
            Self::RuntimeMemberStringReferences => "runtime/member_string_references",
            Self::RuntimeMemset => "runtime/memset",
            Self::RuntimeOperator => "runtime/operator",
            Self::RuntimePrintf => "runtime/printf",
            Self::RuntimePrintfFormat => "runtime/printf_format",
            Self::RuntimeReferences => "runtime/references",
            Self::RuntimeString => "runtime/string",
            Self::RuntimeThreadsafeFn => "runtime/threadsafe_fn",
            Self::RuntimeVlog => "runtime/vlog",
            Self::WhitespaceBlankLine => "whitespace/blank_line",
            Self::WhitespaceBraces => "whitespace/braces",
            Self::WhitespaceComma => "whitespace/comma",
            Self::WhitespaceComments => "whitespace/comments",
            Self::WhitespaceEmptyConditionalBody => "whitespace/empty_conditional_body",
            Self::WhitespaceEmptyIfBody => "whitespace/empty_if_body",
            Self::WhitespaceEmptyLoopBody => "whitespace/empty_loop_body",
            Self::WhitespaceEndOfLine => "whitespace/end_of_line",
            Self::WhitespaceEndingNewline => "whitespace/ending_newline",
            Self::WhitespaceForcolon => "whitespace/forcolon",
            Self::WhitespaceIndent => "whitespace/indent",
            Self::WhitespaceIndentNamespace => "whitespace/indent_namespace",
            Self::WhitespaceLineLength => "whitespace/line_length",
            Self::WhitespaceNewline => "whitespace/newline",
            Self::WhitespaceOperators => "whitespace/operators",
            Self::WhitespaceParens => "whitespace/parens",
            Self::WhitespaceSemicolon => "whitespace/semicolon",
            Self::WhitespaceTab => "whitespace/tab",
            Self::WhitespaceTodo => "whitespace/todo",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

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
    let Some(dash_idx) = category.find('-') else {
        return false;
    };

    match &category[..=dash_idx] {
        "abseil-" | "altera-" | "android-" | "boost-" | "bugprone-" | "cert-" | "concurrency-"
        | "cppcoreguidelines-" | "darwin-" | "fuchsia-" | "google-" | "hicpp-" | "linuxkernel-"
        | "llvm-" | "llvmlibc-" | "misc-" | "modernize-" | "mpi-" | "objc-" | "openmp-"
        | "performance-" | "portability-" | "readability-" | "zircon-" => true,
        "clang-" => category[dash_idx + 1..].starts_with("analyzer-"),
        _ => false,
    }
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
