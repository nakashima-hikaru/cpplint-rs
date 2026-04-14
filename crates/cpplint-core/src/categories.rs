use std::fmt;
use std::str::FromStr;

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

impl FromStr for Category {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "build/c++11" => Ok(Self::BuildCpp11),
            "build/c++17" => Ok(Self::BuildCpp17),
            "build/deprecated" => Ok(Self::BuildDeprecated),
            "build/endif_comment" => Ok(Self::BuildEndifComment),
            "build/explicit_make_pair" => Ok(Self::BuildExplicitMakePair),
            "build/forward_decl" => Ok(Self::BuildForwardDecl),
            "build/header_guard" => Ok(Self::BuildHeaderGuard),
            "build/include" => Ok(Self::BuildInclude),
            "build/include_subdir" => Ok(Self::BuildIncludeSubdir),
            "build/include_alpha" => Ok(Self::BuildIncludeAlpha),
            "build/include_order" => Ok(Self::BuildIncludeOrder),
            "build/include_what_you_use" => Ok(Self::BuildIncludeWhatYouUse),
            "build/namespaces_headers" => Ok(Self::BuildNamespacesHeaders),
            "build/namespaces_literals" => Ok(Self::BuildNamespacesLiterals),
            "build/namespaces" => Ok(Self::BuildNamespaces),
            "build/printf_format" => Ok(Self::BuildPrintfFormat),
            "build/storage_class" => Ok(Self::BuildStorageClass),
            "legal/copyright" => Ok(Self::LegalCopyright),
            "readability/alt_tokens" => Ok(Self::ReadabilityAltTokens),
            "readability/braces" => Ok(Self::ReadabilityBraces),
            "readability/casting" => Ok(Self::ReadabilityCasting),
            "readability/check" => Ok(Self::ReadabilityCheck),
            "readability/constructors" => Ok(Self::ReadabilityConstructors),
            "readability/fn_size" => Ok(Self::ReadabilityFnSize),
            "readability/inheritance" => Ok(Self::ReadabilityInheritance),
            "readability/multiline_comment" => Ok(Self::ReadabilityMultilineComment),
            "readability/multiline_string" => Ok(Self::ReadabilityMultilineString),
            "readability/namespace" => Ok(Self::ReadabilityNamespace),
            "readability/nolint" => Ok(Self::ReadabilityNolint),
            "readability/nul" => Ok(Self::ReadabilityNul),
            "readability/strings" => Ok(Self::ReadabilityStrings),
            "readability/todo" => Ok(Self::ReadabilityTodo),
            "readability/utf8" => Ok(Self::ReadabilityUtf8),
            "runtime/arrays" => Ok(Self::RuntimeArrays),
            "runtime/casting" => Ok(Self::RuntimeCasting),
            "runtime/explicit" => Ok(Self::RuntimeExplicit),
            "runtime/int" => Ok(Self::RuntimeInt),
            "runtime/init" => Ok(Self::RuntimeInit),
            "runtime/invalid_increment" => Ok(Self::RuntimeInvalidIncrement),
            "runtime/member_string_references" => Ok(Self::RuntimeMemberStringReferences),
            "runtime/memset" => Ok(Self::RuntimeMemset),
            "runtime/operator" => Ok(Self::RuntimeOperator),
            "runtime/printf" => Ok(Self::RuntimePrintf),
            "runtime/printf_format" => Ok(Self::RuntimePrintfFormat),
            "runtime/references" => Ok(Self::RuntimeReferences),
            "runtime/string" => Ok(Self::RuntimeString),
            "runtime/threadsafe_fn" => Ok(Self::RuntimeThreadsafeFn),
            "runtime/vlog" => Ok(Self::RuntimeVlog),
            "whitespace/blank_line" => Ok(Self::WhitespaceBlankLine),
            "whitespace/braces" => Ok(Self::WhitespaceBraces),
            "whitespace/comma" => Ok(Self::WhitespaceComma),
            "whitespace/comments" => Ok(Self::WhitespaceComments),
            "whitespace/empty_conditional_body" => Ok(Self::WhitespaceEmptyConditionalBody),
            "whitespace/empty_if_body" => Ok(Self::WhitespaceEmptyIfBody),
            "whitespace/empty_loop_body" => Ok(Self::WhitespaceEmptyLoopBody),
            "whitespace/end_of_line" => Ok(Self::WhitespaceEndOfLine),
            "whitespace/ending_newline" => Ok(Self::WhitespaceEndingNewline),
            "whitespace/forcolon" => Ok(Self::WhitespaceForcolon),
            "whitespace/indent" => Ok(Self::WhitespaceIndent),
            "whitespace/indent_namespace" => Ok(Self::WhitespaceIndentNamespace),
            "whitespace/line_length" => Ok(Self::WhitespaceLineLength),
            "whitespace/newline" => Ok(Self::WhitespaceNewline),
            "whitespace/operators" => Ok(Self::WhitespaceOperators),
            "whitespace/parens" => Ok(Self::WhitespaceParens),
            "whitespace/semicolon" => Ok(Self::WhitespaceSemicolon),
            "whitespace/tab" => Ok(Self::WhitespaceTab),
            "whitespace/todo" => Ok(Self::WhitespaceTodo),
            _ => Err(()),
        }
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
