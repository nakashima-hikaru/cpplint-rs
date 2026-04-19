use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LintMessage {
    // Core / Suppression messages
    InvalidUtf8,
    NulByte,
    MixedLineEndings,
    NolintBlockNeverEnded,
    NolintBlockAlreadyDefined(usize),
    NotInNolintBlock,
    UnknownNolintCategory(String),
    NolintCategoriesNotSupportedInEnd(String),
    UnterminatedMultilineComment,

    // Legal
    NoCopyrightFound,

    // Whitespace
    AtLeastTwoSpacesBetweenCodeAndComments,
    TooManySpacesBeforeTodo,
    MissingUsernameInTodo,
    TodoShouldBeFollowedBySpace,
    ShouldHaveSpaceBetweenSlashesAndComment,
    MissingSpacesAround(String),
    ExtraSpaceForOperator(String),
    MissingSpacesAroundBracket,
    ExtraSpaceBeforeBracket,
    ExtraSpaceAfterParen,
    ExtraSpaceBeforeParenIn(String), // e.g., "if"
    ExtraSpaceBeforeParenInFuncCall,
    MismatchingSpacesInsideParen,
    MissingSpaceBeforeOpenParen,
    MissingSpaceBeforeOpenBrace,
    ExtraSpaceBeforeDoubleColon,
    ExtraSpaceAfterDoubleColon,
    ExtraSpaceBeforeSemicolon,
    ExtraSpaceAfterSemicolon,
    MissingSpaceBeforeSemicolon,
    ExtraSpaceBeforeComma,
    ExtraSpaceAfterComma,
    MissingSpaceAfterComma,
    ExtraSpaceBeforeCloseParen,
    ClosingParenShouldBeMovedToPreviousLine,
    LineLength(usize),
    TrailingWhitespace,
    TabFound,
    NewlineShouldBeAtEndOfFile,
    MultipleBlankLines,
    BlankLineAtStartOfBlock,
    BlankLineAtEndOfBlock,
    NoBlankLineAfterSection,
    ShouldBeIndented(String),      // e.g., "+1 space inside"
    ClosingBraceAlignment(String), // e.g., "class"

    // Runtime
    DeprecatedCastingStyle(String),
    AddressOfCast,
    ConstructorShouldBeExplicit(bool),
    CStyleCast(String, String),
    ChangingPointerInsteadOfValue,
    NonStandardMinMaxOperators,
    MemsetInvalidSize,
    MemsetZeroSize,
    ThreadsafeFunctionRecommended(String, String),
    GlobalStringCtor,
    SnprintfArgsNotNumeric,
    SnprintfArgsMismatch,
    SprintfRecommended,
    VlaFound(String),
    PrintfFormat(String),
    PrintfFormatDeprecatedQ,
    PrintfFormatUndefinedEscape,
    NonConstReference(String),
    UnaryOperatorAmpersand,
    PortsShouldBeUnsignedShort,
    CIntegerType(String),

    // Readability
    RedundantCast(String),
    BracesMissing(String),
    BracesRedundant(String),
    BuildExplicitMakePair,
    EmptyIfBody,
    EmptyConditionalBody(String),
    EmptyLoopBody(String),
    NamespaceIndented,
    TodoNoUsername,
    TodoNoSpace,
    MultilineCommentInLine,
    RawStringUnterminated,
    NamespaceMissingComment(String),
    RedundantVirtual,
    RedundantOverride,
    AltToken(String, String),

    // Build
    IncludeOrder(String, String),
    IncludeAlpha(String),
    IwyuAddInclude(String, String), // header, symbol
    MissingSelfHeader {
        file_from_repo: String,
        header: String,
        includes_use_aliases: bool,
    },
    HeaderGuardMissing(String),
    HeaderGuardWrong(String, String),
    EndifCommentMissing(String),
    NamespacesHeaders,
    NamespacesLiterals,
    NamespacesForwardDecl,
    AlreadyIncluded(String, String, usize), // include, filename, first_line
    DoNotIncludeExtensionFromOtherPackages(String),

    // Catch-all for migration
    Raw(String),
}

impl LintMessage {
    pub fn to_msg(&self) -> String {
        match self {
            Self::InvalidUtf8 => "Line contains invalid UTF-8 (or Unicode replacement character).".to_string(),
            Self::NulByte => "Line contains NUL byte.".to_string(),
            Self::MixedLineEndings => "Unexpected \\r (^M) found; better to use only \\n".to_string(),
            Self::NolintBlockNeverEnded => "NOLINT block never ended".to_string(),
            Self::NolintBlockAlreadyDefined(line) => format!("NOLINT block already defined on line {}", line),
            Self::NotInNolintBlock => "Not in a NOLINT block".to_string(),
            Self::UnknownNolintCategory(cat) => format!("Unknown NOLINT error category: {}", cat),
            Self::NolintCategoriesNotSupportedInEnd(cat) => format!("NOLINT categories not supported in block END: {}", cat),
            Self::UnterminatedMultilineComment => "Could not find end of multi-line comment".to_string(),

            Self::NoCopyrightFound => "No copyright message found.  You should have a line: \"Copyright [year] <Copyright Owner>\"".to_string(),

            Self::AtLeastTwoSpacesBetweenCodeAndComments => "At least two spaces is best between code and comments".to_string(),
            Self::TooManySpacesBeforeTodo => "Too many spaces before TODO".to_string(),
            Self::MissingUsernameInTodo => "Missing username in TODO; it should look like \"// TODO(my_username): Stuff.\"".to_string(),
            Self::TodoShouldBeFollowedBySpace => "TODO(my_username) should be followed by a space".to_string(),
            Self::ShouldHaveSpaceBetweenSlashesAndComment => "Should have a space between // and comment".to_string(),
            Self::MissingSpacesAround(op) => format!("Missing spaces around {}", op),
            Self::ExtraSpaceForOperator(op) => format!("Extra space for operator {}", op),
            Self::MissingSpacesAroundBracket => "Missing spaces around [ ]".to_string(),
            Self::ExtraSpaceBeforeBracket => "Extra space before [".to_string(),
            Self::ExtraSpaceAfterParen => "Extra space after (".to_string(),
            Self::ExtraSpaceBeforeParenIn(ctx) => format!("Extra space before ( in {} (", ctx),
            Self::ExtraSpaceBeforeParenInFuncCall => "Extra space before ( in function call".to_string(),
            Self::MismatchingSpacesInsideParen => "Mismatching spaces inside ()".to_string(),
            Self::MissingSpaceBeforeOpenParen => "Missing space before (".to_string(),
            Self::MissingSpaceBeforeOpenBrace => "Missing space before {".to_string(),
            Self::ExtraSpaceBeforeDoubleColon => "Extra space before ::".to_string(),
            Self::ExtraSpaceAfterDoubleColon => "Extra space after ::".to_string(),
            Self::ExtraSpaceBeforeSemicolon => "Extra space before ;".to_string(),
            Self::ExtraSpaceAfterSemicolon => "Extra space after ;".to_string(),
            Self::MissingSpaceBeforeSemicolon => "Missing space after ;".to_string(),
            Self::ExtraSpaceBeforeComma => "Extra space before ,".to_string(),
            Self::ExtraSpaceAfterComma => "Extra space after ,".to_string(),
            Self::MissingSpaceAfterComma => "Missing space after ,".to_string(),
            Self::ExtraSpaceBeforeCloseParen => "Extra space before )".to_string(),
            Self::ClosingParenShouldBeMovedToPreviousLine => "Closing ) should be moved to the previous line".to_string(),
            Self::LineLength(len) => format!("Lines should be <= {} characters long", len),
            Self::TrailingWhitespace => "Lines should not have trailing whitespace".to_string(),
            Self::TabFound => "Tab found; better to use spaces".to_string(),
            Self::NewlineShouldBeAtEndOfFile => "Could not find a newline character at the end of the file.".to_string(),
            Self::MultipleBlankLines => "Blank line at the start of a code block.  Is this needed?".to_string(), 
            Self::BlankLineAtStartOfBlock => "Blank line at the start of a code block.  Is this needed?".to_string(),
            Self::BlankLineAtEndOfBlock => "Blank line at the end of a code block.  Is this needed?".to_string(),
            Self::NoBlankLineAfterSection => "No blank line after section".to_string(),
            Self::ShouldBeIndented(msg) => format!("should be indented {}", msg),
            Self::BuildExplicitMakePair => "For C++11-compatibility, omit template arguments from make_pair OR use pair directly OR if appropriate, construct a pair directly".to_string(),
            Self::EmptyIfBody => "If statement had no body and no else clause".to_string(),
            Self::EmptyConditionalBody(kind) => format!("Empty conditional bodies should use {}", kind),
            Self::EmptyLoopBody(kind) => format!("Empty loop bodies should use {}", kind),
            Self::ClosingBraceAlignment(expected) => format!("Closing brace should be aligned with beginning of the statement, e.g. {}", expected),

            Self::DeprecatedCastingStyle(t) => format!("Using deprecated casting style.  Use static_cast<{0}>(...) instead", t),
            Self::AddressOfCast => "Are you taking an address of a cast?  This is dangerous: could be a temp var.  Take the address before doing the cast, rather than after".to_string(),
            Self::ConstructorShouldBeExplicit(one_arg) => if *one_arg {
                "Constructors callable with one argument should be marked explicit.".to_string()
            } else {
                "Single-parameter constructors should be marked explicit.".to_string()
            },
            Self::CStyleCast(cast_type, type_str) => format!("Using C-style cast.  Use {0}<{1}>(...) instead", cast_type, type_str),
            Self::ChangingPointerInsteadOfValue => "Changing pointer instead of value (or unused value of operator*).".to_string(),
            Self::NonStandardMinMaxOperators => ">? and <? (max and min) operators are non-standard and deprecated.".to_string(),
            Self::MemsetInvalidSize => "Why not use a numeric size for memset?".to_string(),
            Self::MemsetZeroSize => "Why not use zero as the value for memset?".to_string(),
            Self::ThreadsafeFunctionRecommended(old, new) => format!("Consider using {} instead of {}, which is not thread-safe.", new, old),
            Self::GlobalStringCtor => "For global/static strings, use a char array instead of a std::string to avoid dynamic initialization.".to_string(),
            Self::SnprintfArgsNotNumeric => "snprintf with non-numeric second argument is potentially unsafe.".to_string(),
            Self::SnprintfArgsMismatch => "snprintf size argument should be the size of the buffer.".to_string(),
            Self::SprintfRecommended => "Consider using snprintf instead of sprintf.".to_string(),
            Self::VlaFound(name) => format!("Variable-length array {} found. Use a fixed-size array or a vector instead.", name),
            Self::PrintfFormat(f) => format!("Printf format string contains {}", f),
            Self::PrintfFormatDeprecatedQ => "%q in format strings is deprecated.  Use %ll instead.".to_string(),
            Self::PrintfFormatUndefinedEscape => "%, [, (, and { are undefined character escapes.  Unescape them.".to_string(),
            Self::NonConstReference(name) => format!("Is {} a non-const reference? If so, make it a pointer or a const reference.", name),
            Self::UnaryOperatorAmpersand => "Unary operator& is dangerous.  Do not use it.".to_string(),
            Self::PortsShouldBeUnsignedShort => "Use \"unsigned short\" for ports, not \"short\"".to_string(),
            Self::CIntegerType(ty) => format!("Use int16_t/int64_t/etc, rather than the C type {}", ty),

            Self::RedundantCast(t) => format!("Redundant cast to {}", t),
            Self::BracesMissing(kind) => format!("Else/If should always be enclosed in braces: {}", kind),
            Self::BracesRedundant(kind) => format!("Redundant braces around {}", kind),
            Self::NamespaceIndented => "Namespace should not be indented.".to_string(),
            Self::TodoNoUsername => "Missing username in TODO; it should look like \"// TODO(my_username): Stuff.\"".to_string(),
            Self::TodoNoSpace => "TODO(my_username) should be followed by a space".to_string(),
            Self::MultilineCommentInLine => "Multi-line comment found on a single line. Use // instead.".to_string(),
            Self::RawStringUnterminated => "Unterminated raw string.".to_string(),
            Self::NamespaceMissingComment(name) => format!("Namespace should be terminated with \"// namespace {}\"", name),
            Self::RedundantVirtual => "virtual is redundant since override/final already implies a virtual function".to_string(),
            Self::RedundantOverride => "override is redundant when final is present".to_string(),
            Self::AltToken(token, key) => format!("Use operator {} instead of {}", token, key),

            Self::IncludeOrder(msg, stem) => format!("{}. Should be: {}.h, c system, c++ system, other.", msg, stem),
            Self::IncludeAlpha(include) => format!("Include \"{}\" not in alphabetical order", include),
            Self::IwyuAddInclude(header, symbol) => format!("Add #include <{}> for {}", header, symbol),
            Self::MissingSelfHeader {
                file_from_repo,
                header,
                includes_use_aliases,
            } => {
                let mut message = format!("{} should include its header file {}", file_from_repo, header);
                if *includes_use_aliases {
                    message.push_str(". Relative paths like . and .. are not allowed.");
                }
                message
            }

            Self::HeaderGuardMissing(path) => format!("No #ifndef header guard found, should be {}", path),
            Self::HeaderGuardWrong(_found, expected) => format!("#ifndef header guard has wrong name, should be {}", expected),
            Self::EndifCommentMissing(expected) => format!("#endif line should be \"#endif  // {}\"", expected),
            Self::NamespacesHeaders => "Do not use using-directives in headers.".to_string(),
            Self::NamespacesLiterals => "Do not use using-directives for literals in headers.".to_string(),
            Self::NamespacesForwardDecl => "Do not use forward declarations in headers.".to_string(),
            Self::AlreadyIncluded(include, file, line) => format!("\"{}\" already included at {}:{}", include, file, line),
            Self::DoNotIncludeExtensionFromOtherPackages(ext) => format!("Do not include .{} files from other packages", ext),

            Self::Raw(msg) => msg.clone(),
        }
    }
}

impl PartialEq<&str> for LintMessage {
    fn eq(&self, other: &&str) -> bool {
        &self.to_msg() == other
    }
}

impl PartialEq<str> for LintMessage {
    fn eq(&self, other: &str) -> bool {
        self.to_msg() == other
    }
}

impl fmt::Display for LintMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_msg())
    }
}

impl From<&str> for LintMessage {
    fn from(s: &str) -> Self {
        Self::Raw(s.to_string())
    }
}

impl From<String> for LintMessage {
    fn from(s: String) -> Self {
        Self::Raw(s)
    }
}

impl From<&String> for LintMessage {
    fn from(s: &String) -> Self {
        Self::Raw(s.clone())
    }
}

impl From<std::sync::Arc<str>> for LintMessage {
    fn from(s: std::sync::Arc<str>) -> Self {
        Self::Raw(s.to_string())
    }
}
