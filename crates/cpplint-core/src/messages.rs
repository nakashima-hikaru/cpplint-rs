use crate::iwyu::IwyuHeader;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OperatorSymbol {
    Eq,
    Ne,
    Lt,
    Gt,
    LShift,
    RShift,
    Colon,
    Bang,
    BangSpaced,
    Tilde,
}

impl OperatorSymbol {
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "=" => Some(Self::Eq),
            "!=" => Some(Self::Ne),
            "<" => Some(Self::Lt),
            ">" => Some(Self::Gt),
            "<<" => Some(Self::LShift),
            ">>" => Some(Self::RShift),
            ":" => Some(Self::Colon),
            "!" => Some(Self::Bang),
            "! " => Some(Self::BangSpaced),
            "~" => Some(Self::Tilde),
            _ => None,
        }
    }

    pub fn as_display_str(self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Gt => ">",
            Self::LShift => "<<",
            Self::RShift => ">>",
            Self::Colon => ":",
            Self::Bang => "!",
            Self::BangSpaced => "! ",
            Self::Tilde => "~",
        }
    }

    pub fn as_fix_str(self) -> &'static str {
        match self {
            Self::BangSpaced => "!",
            _ => self.as_display_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LintMessage {
    // Core / Suppression messages
    InvalidUtf8,
    NulByte,
    MixedLineEndings,
    NolintBlockNeverEnded,
    NolintBlockAlreadyDefined(usize),
    NotInNolintBlock,
    UnknownNolintCategory(Box<str>),
    NolintCategoriesNotSupportedInEnd(Box<str>),
    UnterminatedMultilineComment,

    // Legal
    NoCopyrightFound,

    // Whitespace
    AtLeastTwoSpacesBetweenCodeAndComments,
    TooManySpacesBeforeTodo,
    MissingUsernameInTodo,
    TodoShouldBeFollowedBySpace,
    ShouldHaveSpaceBetweenSlashesAndComment,
    MissingSpacesAround(OperatorSymbol),
    ExtraSpaceForOperator(OperatorSymbol),
    MissingSpacesAroundBracket,
    ExtraSpaceBeforeBracket,
    ExtraSpaceAfterParen,
    ExtraSpaceAfterParenInFuncCall,
    ExtraSpaceBeforeParenIn(Box<str>), // e.g., "if"
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
    MissingSpaceBeforeElse,
    UnnecessarySemicolonAfterBrace,
    SemicolonDefiningEmptyStatementUseBraces,
    LineContainsOnlySemicolonUseBraces,
    ExtraSpaceBeforeLastSemicolonUseBraces,
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
    ShouldBeIndented(Box<str>),      // e.g., "+1 space inside"
    ClosingBraceAlignment(Box<str>), // e.g., "class"

    // Runtime
    DeprecatedCastingStyle(Box<str>),
    AddressOfCast,
    ConstructorShouldBeExplicit(bool),
    CStyleCast(Box<str>, Box<str>),
    ChangingPointerInsteadOfValue,
    NonStandardMinMaxOperators,
    MemsetInvalidSize,
    MemsetZeroSize,
    ThreadsafeFunctionSuggestion(Box<str>),
    GlobalStringCtor,
    SnprintfArgsNotNumeric,
    SnprintfArgsMismatch,
    SprintfRecommended,
    VlaFound(Box<str>),
    PrintfFormat(Box<str>),
    PrintfFormatDeprecatedQ,
    PrintfFormatUndefinedEscape,
    NonConstReference(Box<str>),
    UnaryOperatorAmpersand,
    PortsShouldBeUnsignedShort,
    CIntegerType(Box<str>),

    // Readability
    RedundantCast(Box<str>),
    BracesMissing(Box<str>),
    BracesRedundant(Box<str>),
    BuildExplicitMakePair,
    EmptyIfBody,
    EmptyConditionalBody(Box<str>),
    EmptyLoopBody(Box<str>),
    NamespaceIndented,
    TodoNoUsername,
    TodoNoSpace,
    MultilineCommentInLine,
    RawStringUnterminated,
    NamespaceMissingComment(Box<str>),
    RedundantVirtual,
    RedundantOverride,
    AltToken(Box<str>, Box<str>),
    CheckMacroSuggestion {
        replacement: Box<str>,
        check_macro: Box<str>,
        op: Box<str>,
    },

    // Build
    IncludeOrder(Box<str>, Box<str>),
    IncludeAlpha(Box<str>),
    IwyuAddInclude(IwyuHeader, Box<str>), // header, symbol
    MissingSelfHeader {
        file_from_repo: Box<str>,
        header: Box<str>,
        includes_use_aliases: bool,
    },
    HeaderGuardMissing(Box<str>),
    HeaderGuardWrong(Box<str>, Box<str>),
    EndifCommentMissing(Box<str>),
    NamespacesHeaders,
    NamespacesLiterals,
    NamespacesForwardDecl,
    AlreadyIncluded(Box<str>, Box<str>, usize), // include, filename, first_line
    DoNotIncludeExtensionFromOtherPackages(Box<str>),

    // Catch-all for migration
    Raw(Box<str>),
}

impl LintMessage {
    fn as_static_str(&self) -> Option<&'static str> {
        match self {
            Self::InvalidUtf8 => {
                Some("Line contains invalid UTF-8 (or Unicode replacement character).")
            }
            Self::NulByte => Some("Line contains NUL byte."),
            Self::MixedLineEndings => Some("Unexpected \\r (^M) found; better to use only \\n"),
            Self::NolintBlockNeverEnded => Some("NOLINT block never ended"),
            Self::NotInNolintBlock => Some("Not in a NOLINT block"),
            Self::UnterminatedMultilineComment => Some("Could not find end of multi-line comment"),
            Self::NoCopyrightFound => Some(
                "No copyright message found.  You should have a line: \"Copyright [year] <Copyright Owner>\"",
            ),
            Self::AtLeastTwoSpacesBetweenCodeAndComments => {
                Some("At least two spaces is best between code and comments")
            }
            Self::TooManySpacesBeforeTodo => Some("Too many spaces before TODO"),
            Self::MissingUsernameInTodo => Some(
                "Missing username in TODO; it should look like \"// TODO(my_username): Stuff.\"",
            ),
            Self::TodoShouldBeFollowedBySpace => {
                Some("TODO(my_username) should be followed by a space")
            }
            Self::ShouldHaveSpaceBetweenSlashesAndComment => {
                Some("Should have a space between // and comment")
            }
            Self::MissingSpacesAroundBracket => Some("Missing spaces around [ ]"),
            Self::ExtraSpaceBeforeBracket => Some("Extra space before ["),
            Self::ExtraSpaceAfterParen => Some("Extra space after ("),
            Self::ExtraSpaceAfterParenInFuncCall => Some("Extra space after ( in function call"),
            Self::ExtraSpaceBeforeParenInFuncCall => Some("Extra space before ( in function call"),
            Self::MismatchingSpacesInsideParen => Some("Mismatching spaces inside ()"),
            Self::MissingSpaceBeforeOpenParen => Some("Missing space before ("),
            Self::MissingSpaceBeforeOpenBrace => Some("Missing space before {"),
            Self::ExtraSpaceBeforeDoubleColon => Some("Extra space before ::"),
            Self::ExtraSpaceAfterDoubleColon => Some("Extra space after ::"),
            Self::ExtraSpaceBeforeSemicolon => Some("Extra space before ;"),
            Self::ExtraSpaceAfterSemicolon => Some("Extra space after ;"),
            Self::MissingSpaceBeforeSemicolon => Some("Missing space after ;"),
            Self::ExtraSpaceBeforeComma => Some("Extra space before ,"),
            Self::ExtraSpaceAfterComma => Some("Extra space after ,"),
            Self::MissingSpaceAfterComma => Some("Missing space after ,"),
            Self::MissingSpaceBeforeElse => Some("Missing space before else"),
            Self::UnnecessarySemicolonAfterBrace => Some("You don't need a ; after a }"),
            Self::SemicolonDefiningEmptyStatementUseBraces => {
                Some("Semicolon defining empty statement. Use {} instead.")
            }
            Self::LineContainsOnlySemicolonUseBraces => Some(
                "Line contains only semicolon. If this should be an empty statement, use {} instead.",
            ),
            Self::ExtraSpaceBeforeLastSemicolonUseBraces => Some(
                "Extra space before last semicolon. If this should be an empty statement, use {} instead.",
            ),
            Self::ExtraSpaceBeforeCloseParen => Some("Extra space before )"),
            Self::ClosingParenShouldBeMovedToPreviousLine => {
                Some("Closing ) should be moved to the previous line")
            }
            Self::TrailingWhitespace => Some("Lines should not have trailing whitespace"),
            Self::TabFound => Some("Tab found; better to use spaces"),
            Self::NewlineShouldBeAtEndOfFile => {
                Some("Could not find a newline character at the end of the file.")
            }
            Self::MultipleBlankLines | Self::BlankLineAtStartOfBlock => {
                Some("Blank line at the start of a code block.  Is this needed?")
            }
            Self::BlankLineAtEndOfBlock => {
                Some("Blank line at the end of a code block.  Is this needed?")
            }
            Self::NoBlankLineAfterSection => Some("No blank line after section"),
            Self::BuildExplicitMakePair => Some(
                "For C++11-compatibility, omit template arguments from make_pair OR use pair directly OR if appropriate, construct a pair directly",
            ),
            Self::EmptyIfBody => Some("If statement had no body and no else clause"),
            Self::AddressOfCast => Some(
                "Are you taking an address of a cast?  This is dangerous: could be a temp var.  Take the address before doing the cast, rather than after",
            ),
            Self::ChangingPointerInsteadOfValue => {
                Some("Changing pointer instead of value (or unused value of operator*).")
            }
            Self::NonStandardMinMaxOperators => {
                Some(">? and <? (max and min) operators are non-standard and deprecated.")
            }
            Self::MemsetInvalidSize => Some("Why not use a numeric size for memset?"),
            Self::MemsetZeroSize => Some("Why not use zero as the value for memset?"),
            Self::GlobalStringCtor => Some(
                "For global/static strings, use a char array instead of a std::string to avoid dynamic initialization.",
            ),
            Self::SnprintfArgsNotNumeric => {
                Some("snprintf with non-numeric second argument is potentially unsafe.")
            }
            Self::SnprintfArgsMismatch => {
                Some("snprintf size argument should be the size of the buffer.")
            }
            Self::SprintfRecommended => Some("Consider using snprintf instead of sprintf."),
            Self::PrintfFormatDeprecatedQ => {
                Some("%q in format strings is deprecated.  Use %ll instead.")
            }
            Self::PrintfFormatUndefinedEscape => {
                Some("%, [, (, and { are undefined character escapes.  Unescape them.")
            }
            Self::UnaryOperatorAmpersand => Some("Unary operator& is dangerous.  Do not use it."),
            Self::PortsShouldBeUnsignedShort => {
                Some("Use \"unsigned short\" for ports, not \"short\"")
            }
            Self::NamespaceIndented => Some("Namespace should not be indented."),
            Self::TodoNoUsername => Some(
                "Missing username in TODO; it should look like \"// TODO(my_username): Stuff.\"",
            ),
            Self::TodoNoSpace => Some("TODO(my_username) should be followed by a space"),
            Self::MultilineCommentInLine => {
                Some("Multi-line comment found on a single line. Use // instead.")
            }
            Self::RawStringUnterminated => Some("Unterminated raw string."),
            Self::RedundantVirtual => {
                Some("virtual is redundant since override/final already implies a virtual function")
            }
            Self::RedundantOverride => Some("override is redundant when final is present"),
            Self::NamespacesHeaders => Some("Do not use using-directives in headers."),
            Self::NamespacesLiterals => {
                Some("Do not use using-directives for literals in headers.")
            }
            Self::NamespacesForwardDecl => Some("Do not use forward declarations in headers."),
            Self::ConstructorShouldBeExplicit(one_arg) => {
                if *one_arg {
                    Some("Constructors callable with one argument should be marked explicit.")
                } else {
                    Some("Single-parameter constructors should be marked explicit.")
                }
            }
            _ => None,
        }
    }
}

impl fmt::Display for LintMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(msg) = self.as_static_str() {
            return f.write_str(msg);
        }

        match self {
            Self::NolintBlockAlreadyDefined(line) => {
                write!(f, "NOLINT block already defined on line {}", line)
            }
            Self::UnknownNolintCategory(cat) => write!(f, "Unknown NOLINT error category: {}", cat),
            Self::NolintCategoriesNotSupportedInEnd(cat) => {
                write!(f, "NOLINT categories not supported in block END: {}", cat)
            }
            Self::MissingSpacesAround(op) => {
                write!(f, "Missing spaces around {}", op.as_display_str())
            }
            Self::ExtraSpaceForOperator(op) => {
                write!(f, "Extra space for operator {}", op.as_display_str())
            }
            Self::ExtraSpaceBeforeParenIn(ctx) => write!(f, "Extra space before ( in {} (", ctx),
            Self::LineLength(len) => write!(f, "Lines should be <= {} characters long", len),
            Self::ShouldBeIndented(msg) => write!(f, "should be indented {}", msg),
            Self::EmptyConditionalBody(kind) => {
                write!(f, "Empty conditional bodies should use {}", kind)
            }
            Self::EmptyLoopBody(kind) => write!(f, "Empty loop bodies should use {}", kind),
            Self::ClosingBraceAlignment(expected) => {
                write!(
                    f,
                    "Closing brace should be aligned with beginning of the statement, e.g. {}",
                    expected
                )
            }
            Self::DeprecatedCastingStyle(t) => {
                write!(
                    f,
                    "Using deprecated casting style.  Use static_cast<{}>(...) instead",
                    t
                )
            }
            Self::CStyleCast(cast_type, type_str) => {
                write!(
                    f,
                    "Using C-style cast.  Use {}<{}>(...) instead",
                    cast_type, type_str
                )
            }
            Self::ThreadsafeFunctionSuggestion(funcname) => {
                write!(
                    f,
                    "Consider using {}_r(...) instead of {}(...) for improved thread safety.",
                    funcname, funcname
                )
            }
            Self::VlaFound(name) => {
                write!(
                    f,
                    "Variable-length array {} found. Use a fixed-size array or a vector instead.",
                    name
                )
            }
            Self::PrintfFormat(fmt_part) => write!(f, "Printf format string contains {}", fmt_part),
            Self::NonConstReference(name) => write!(
                f,
                "Is {} a non-const reference? If so, make it a pointer or a const reference.",
                name
            ),
            Self::CIntegerType(ty) => {
                write!(f, "Use int16_t/int64_t/etc, rather than the C type {}", ty)
            }
            Self::RedundantCast(t) => write!(f, "Redundant cast to {}", t),
            Self::BracesMissing(kind) => {
                write!(f, "Else/If should always be enclosed in braces: {}", kind)
            }
            Self::BracesRedundant(kind) => write!(f, "Redundant braces around {}", kind),
            Self::NamespaceMissingComment(name) => {
                write!(
                    f,
                    "Namespace should be terminated with \"// namespace {}\"",
                    name
                )
            }
            Self::AltToken(token, key) => write!(f, "Use operator {} instead of {}", token, key),
            Self::CheckMacroSuggestion {
                replacement,
                check_macro,
                op,
            } => write!(
                f,
                "Consider using {} instead of {}(a {} b)",
                replacement, check_macro, op
            ),
            Self::IncludeOrder(msg, stem) => {
                write!(
                    f,
                    "{}. Should be: {}.h, c system, c++ system, other.",
                    msg, stem
                )
            }
            Self::IncludeAlpha(include) => {
                write!(f, "Include \"{}\" not in alphabetical order", include)
            }
            Self::IwyuAddInclude(header, symbol) => {
                write!(f, "Add #include <{}> for {}", header.as_str(), symbol)
            }
            Self::MissingSelfHeader {
                file_from_repo,
                header,
                includes_use_aliases,
            } => {
                write!(
                    f,
                    "{} should include its header file {}",
                    file_from_repo, header
                )?;
                if *includes_use_aliases {
                    f.write_str(". Relative paths like . and .. are not allowed.")?;
                }
                Ok(())
            }
            Self::HeaderGuardMissing(path) => {
                write!(f, "No #ifndef header guard found, should be {}", path)
            }
            Self::HeaderGuardWrong(_found, expected) => {
                write!(
                    f,
                    "#ifndef header guard has wrong name, should be {}",
                    expected
                )
            }
            Self::EndifCommentMissing(expected) => {
                write!(f, "#endif line should be \"#endif  // {}\"", expected)
            }
            Self::AlreadyIncluded(include, file, line) => {
                write!(f, "\"{}\" already included at {}:{}", include, file, line)
            }
            Self::DoNotIncludeExtensionFromOtherPackages(ext) => {
                write!(f, "Do not include .{} files from other packages", ext)
            }
            Self::Raw(msg) => f.write_str(msg),
            _ => unreachable!("unhandled LintMessage variant in Display"),
        }
    }
}

impl From<&str> for LintMessage {
    fn from(s: &str) -> Self {
        Self::Raw(s.into())
    }
}

impl From<String> for LintMessage {
    fn from(s: String) -> Self {
        Self::Raw(s.into_boxed_str())
    }
}

impl From<&String> for LintMessage {
    fn from(s: &String) -> Self {
        Self::Raw(s.as_str().into())
    }
}

impl From<std::sync::Arc<str>> for LintMessage {
    fn from(s: std::sync::Arc<str>) -> Self {
        Self::Raw(s.as_ref().into())
    }
}
