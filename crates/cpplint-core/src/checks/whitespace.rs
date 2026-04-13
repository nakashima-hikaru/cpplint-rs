use crate::cleanse::CleansedLines;
use crate::file_linter::FileLinter;
use crate::string_utils;
use aho_corasick::AhoCorasick;
use regex::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;
use unicode_width::UnicodeWidthStr;

static TODO_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^//(\s*)TODO(\(.+?\))?:?(\s|$)?"#).unwrap());
static ACCESS_SPECIFIER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^(.*)\b(public|private|protected|signals)(\s+(?:slots\s*)?)?:(?:[^:]|$)"#)
        .unwrap()
});
static OPERATOR_METHOD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(.*\boperator\b)(\S+)(\s*\(.*)$"#).unwrap());
static CONTROL_STRUCT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\b(if|elif|for|while|switch|return|new|delete|catch|sizeof)\b"#).unwrap()
});
static FUNC_REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#" \([^)]+\)\([^)]*(\)|,$)"#).unwrap());
static ARRAY_REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#" \([^)]+\)\[[^\]]+\]"#).unwrap());
static LESS_SPACING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(.*[^\s<])<[^\s=<,]"#).unwrap());
static GREATER_SPACING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(.*[^-\s>])>[^\s=>,]"#).unwrap());
static LSHIFT_SPACING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(operator|[^\s(<])(?:L|UL|LL|ULL|l|ul|ll|ull)?<<([^\s,=<])"#).unwrap()
});
static RSHIFT_SPACING_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#">>[a-zA-Z_]"#).unwrap());
static OPERATOR_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\boperator_*\b"#).unwrap());
static VA_OPT_COMMA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\b__VA_OPT__\s*\(,\)"#).unwrap());
static OPERATOR_COMMA_CALL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\boperator\s*,\s*\("#).unwrap());
static BRACE_INLINE_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^.*\{\s*//"#).unwrap());
static COMMENT_WITHOUT_SPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^//[^ ]*\w"#).unwrap());
static DOC_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(///|//!)(\s+|$)"#).unwrap());
static PREV_LINE_CONTINUATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[\",=><] *$"#).unwrap());
static RANGE_FOR_COLON_LEFT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"for\s*\(.*[^:]:[^: ]"#).unwrap());
static RANGE_FOR_COLON_RIGHT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"for\s*\(.*[^: ]:[^:]"#).unwrap());
static SCOPE_OR_LABEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*(?:public|private|protected|signals)(?:\s+(?:slots\s*)?)?:\s*\\?\s*$"#)
        .unwrap()
});
static CONTROL_BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\b(if|while|for) "#).unwrap());
static CONTROL_PARENS_SPACE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\b(if|for|while|switch)\s*\(([ ]*)(.).*[^ ]+([ ]*)\)\s*\{\s*$"#).unwrap()
});
static CONTROL_PARENS_MISSING_SPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\b(if\(|for\(|while\(|switch\()"#).unwrap());
static IF_FOR_SWITCH_CALL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\b(if|for|switch)\s*\((.*)\)\s*\{"#).unwrap());
static WHILE_CALL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bwhile\s*\((.*)\)\s*[{;]"#).unwrap());
static FOR_CLOSING_SEMICOLON_EXCEPTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bfor\s*\(.*; \)"#).unwrap());
static EXTRA_SPACE_BEFORE_CALL_PAREN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\w\s+\("#).unwrap());
static ASM_VOLATILE_CALL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"_{0,2}asm_{0,2}\s+_{0,2}volatile_{0,2}\s+\("#).unwrap());
static DEFINE_TYPEDEF_USING_ASSIGN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"#\s*define|typedef|using\s+\w+\s*="#).unwrap());
static FUNCTION_POINTER_CALL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\w\s+\((\w+::)*\*\w+\)\("#).unwrap());
static CASE_PAREN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\bcase\s+\("#).unwrap());
static EXTRA_SPACE_BEFORE_CLOSE_PAREN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[^)]\s+\)\s*[^{\s]"#).unwrap());
static INITLIST_CONTINUATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^ {6}\w"#).unwrap());
static FUNCTION_HEADER_BLANK_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^ {4}\w[^\(]*\)\s*(const\s*)?(\{\s*$|:)"#).unwrap());
static INITLIST_HEADER_BLANK_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^ {4}:"#).unwrap());
static MISSING_SPACE_AFTER_COMMA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#",[^,\s]"#).unwrap());
static MISSING_SPACE_AFTER_SEMICOLON_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#";[^\s};\\)/]"#).unwrap());
static MULTI_COMMAND_INITLIST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^[^{};]*\[[^\[\]]*\][^{}]*\{[^{}\n\r]*\}"#).unwrap());
static OPEN_BRACE_NEEDS_SPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(.*[^ ({>])\{"#).unwrap());
const KEYWORDS: [&str; 24] = [
    "if",
    "for",
    "while",
    "switch",
    "case",
    "default",
    "return",
    "new",
    "delete",
    "catch",
    "operator",
    "__VA_OPT__",
    "public",
    "private",
    "protected",
    "signals",
    "slots",
    "sizeof",
    "elif",
    "typedef",
    "using",
    "static_cast",
    "reinterpret_cast",
    "const_cast",
];

static KEYWORDS_AC: LazyLock<AhoCorasick> = LazyLock::new(|| AhoCorasick::new(KEYWORDS).unwrap());

#[derive(Default, Clone, Copy, PartialEq, Eq)]
struct MatchedKeywords(u32);

impl MatchedKeywords {
    const IF: u32 = 1 << 0;
    const FOR: u32 = 1 << 1;
    const WHILE: u32 = 1 << 2;
    const SWITCH: u32 = 1 << 3;
    const CASE: u32 = 1 << 4;
    const DEFAULT: u32 = 1 << 5;
    const RETURN: u32 = 1 << 6;
    const NEW: u32 = 1 << 7;
    const DELETE: u32 = 1 << 8;
    const CATCH: u32 = 1 << 9;
    const OPERATOR: u32 = 1 << 10;
    const VA_OPT: u32 = 1 << 11;
    const ACCESS: u32 = 1 << 12;
    const SIZEOF: u32 = 1 << 13;
    const ELIF: u32 = 1 << 14;
    const TYPEDEF: u32 = 1 << 15;
    const USING: u32 = 1 << 16;
    const CAST: u32 = 1 << 17;

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    fn from_line(line: &str) -> Self {
        if !line.bytes().any(|b| b.is_ascii_alphabetic()) {
            return Self::default();
        }
        let mut bits = 0u32;
        for mat in KEYWORDS_AC.find_iter(line) {
            bits |= match mat.pattern().as_usize() {
                0 => Self::IF,
                1 => Self::FOR,
                2 => Self::WHILE,
                3 => Self::SWITCH,
                4 => Self::CASE,
                5 => Self::DEFAULT,
                6 => Self::RETURN,
                7 => Self::NEW,
                8 => Self::DELETE,
                9 => Self::CATCH,
                10 => Self::OPERATOR,
                11 => Self::VA_OPT,
                12..=16 => Self::ACCESS,
                17 => Self::SIZEOF,
                18 => Self::ELIF,
                19 => Self::TYPEDEF,
                20 => Self::USING,
                21..=23 => Self::CAST,
                _ => 0,
            };
        }
        Self(bits)
    }

    #[inline(always)]
    fn has_if(&self) -> bool {
        (self.0 & Self::IF) != 0
    }
    #[inline(always)]
    fn has_for(&self) -> bool {
        (self.0 & Self::FOR) != 0
    }
    #[inline(always)]
    fn has_while(&self) -> bool {
        (self.0 & Self::WHILE) != 0
    }
    #[inline(always)]
    fn has_switch(&self) -> bool {
        (self.0 & Self::SWITCH) != 0
    }
    #[inline(always)]
    fn has_case(&self) -> bool {
        (self.0 & Self::CASE) != 0
    }
    #[inline(always)]
    fn has_default(&self) -> bool {
        (self.0 & Self::DEFAULT) != 0
    }
    #[inline(always)]
    fn has_return(&self) -> bool {
        (self.0 & Self::RETURN) != 0
    }
    #[inline(always)]
    fn has_new(&self) -> bool {
        (self.0 & Self::NEW) != 0
    }
    #[inline(always)]
    fn has_delete(&self) -> bool {
        (self.0 & Self::DELETE) != 0
    }
    #[inline(always)]
    fn has_catch(&self) -> bool {
        (self.0 & Self::CATCH) != 0
    }
    #[inline(always)]
    fn has_operator(&self) -> bool {
        (self.0 & Self::OPERATOR) != 0
    }
    #[inline(always)]
    fn has_va_opt(&self) -> bool {
        (self.0 & Self::VA_OPT) != 0
    }
    #[inline(always)]
    fn has_access(&self) -> bool {
        (self.0 & Self::ACCESS) != 0
    }
    #[inline(always)]
    fn has_sizeof(&self) -> bool {
        (self.0 & Self::SIZEOF) != 0
    }
    #[inline(always)]
    fn has_elif(&self) -> bool {
        (self.0 & Self::ELIF) != 0
    }
    #[inline(always)]
    fn has_typedef(&self) -> bool {
        (self.0 & Self::TYPEDEF) != 0
    }
    #[inline(always)]
    fn has_using(&self) -> bool {
        (self.0 & Self::USING) != 0
    }
}
static BRACED_INIT_TRAILING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^[\s}]*[{.;,)<>\]:]"#).unwrap());
static FIXED_WIDTH_BRACED_INT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:int8_t|int16_t|int32_t|int64_t|uint8_t|uint16_t|uint32_t|uint64_t)\s*\{"#)
        .unwrap()
});
static EMPTY_STATEMENT_AFTER_COLON_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#":\s*;\s*$"#).unwrap());
static ONLY_SEMICOLON_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^\s*;\s*$"#).unwrap());
static SPACE_BEFORE_LAST_SEMICOLON_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\s+;\s*$"#).unwrap());
static CLASS_OR_STRUCT_AC: LazyLock<AhoCorasick> =
    LazyLock::new(|| AhoCorasick::new(["class", "struct"]).unwrap());
static IFNDEF_ENDIF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*#(ifndef|endif)\b"#).unwrap());
static HTTP_URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*//.*https?://\S*$"#).unwrap());
static SINGLE_WORD_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*//\s*[^\s]*$"#).unwrap());
static ID_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^// \$Id:.*#[0-9]+ \$$"#).unwrap());
static DOXYGEN_COPY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*/// [@\\](copydoc|copydetails|copybrief) .*$"#).unwrap());
static QUALIFIED_BRACE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\)\s*(?:const|override|final|noexcept(?:\s*\([^)]*\))?)\{"#).unwrap()
});

fn should_skip_line_length(raw_line: &str) -> bool {
    raw_line.starts_with("#include")
        || IFNDEF_ENDIF_RE.is_match(raw_line)
        || HTTP_URL_RE.is_match(raw_line)
        || SINGLE_WORD_COMMENT_RE.is_match(raw_line)
        || ID_COMMENT_RE.is_match(raw_line)
        || DOXYGEN_COPY_RE.is_match(raw_line)
}

fn contains_class_or_struct_word(line: &str) -> bool {
    CLASS_OR_STRUCT_AC
        .find_iter(line)
        .any(|mat| string_utils::is_word_match(line, mat.start(), mat.end()))
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_comment_spacing(linter: &mut FileLinter, clean_lines: &CleansedLines, linenum: usize) {
    let line = &clean_lines.lines_without_raw_strings[linenum];
    let Some(comment_pos) = line.find("//") else {
        return;
    };

    if crate::cleanse::is_cpp_string(&line[..comment_pos]) {
        return;
    }

    let next_line_start = clean_lines
        .lines_without_raw_strings
        .get(linenum + 1)
        .map(|next| next.len() - next.trim_start().len())
        .unwrap_or(0);

    let allows_single_space_after_scope =
        BRACE_INLINE_COMMENT_RE.is_match(line) && next_line_start == comment_pos;
    if !allows_single_space_after_scope
        && ((comment_pos >= 1 && !line.as_bytes()[comment_pos - 1].is_ascii_whitespace())
            || (comment_pos >= 2 && !line.as_bytes()[comment_pos - 2].is_ascii_whitespace()))
    {
        linter.error(
            linenum,
            "whitespace/comments",
            2,
            "At least two spaces is best between code and comments",
        );
    }

    let comment = &line[comment_pos..];
    if let Some(captures) = TODO_COMMENT_RE.captures(comment) {
        let leading_spaces = captures.get(1).map(|m| m.as_str().len()).unwrap_or(0);
        if leading_spaces > 1 {
            linter.error(linenum, "whitespace/todo", 2, "Too many spaces before TODO");
        }

        if captures.get(2).is_none() {
            linter.error(
                linenum,
                "readability/todo",
                2,
                "Missing username in TODO; it should look like \"// TODO(my_username): Stuff.\"",
            );
        }

        let suffix = captures.get(3).map(|m| m.as_str()).unwrap_or("");
        if captures.get(3).is_none() || (!suffix.is_empty() && suffix != " ") {
            linter.error(
                linenum,
                "whitespace/todo",
                2,
                "TODO(my_username) should be followed by a space",
            );
        }
    }

    if COMMENT_WITHOUT_SPACE_RE.is_match(comment) && !DOC_COMMENT_RE.is_match(comment) {
        linter.error(
            linenum,
            "whitespace/comments",
            4,
            "Should have a space between // and comment",
        );
    }
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_line_continuation(rest: &str) -> bool {
    let trimmed = rest.trim_start();
    trimmed.starts_with('\\') && trimmed[1..].trim().is_empty()
}

fn has_extra_space_before_bracket(line: &str) -> bool {
    let bytes = line.as_bytes();
    for (idx, byte) in bytes.iter().enumerate() {
        if *byte != b'[' || bytes.get(idx + 1) == Some(&b'[') {
            continue;
        }

        let mut space_start = idx;
        while space_start > 0 && bytes[space_start - 1].is_ascii_whitespace() {
            space_start -= 1;
        }
        if space_start == idx || space_start == 0 || !is_word_byte(bytes[space_start - 1]) {
            continue;
        }

        let mut token_start = space_start - 1;
        while token_start > 0
            && (is_word_byte(bytes[token_start - 1]) || bytes[token_start - 1] == b'&')
        {
            token_start -= 1;
        }
        let token = &line[token_start..space_start];
        if matches!(token, "auto" | "auto&" | "delete" | "return" | "using") {
            continue;
        }
        return true;
    }
    false
}

fn has_extra_space_after_function_call_paren(line: &str) -> bool {
    let bytes = line.as_bytes();
    for (idx, byte) in bytes.iter().enumerate() {
        if *byte != b'(' {
            continue;
        }

        let mut prev = idx;
        while prev > 0 && bytes[prev - 1].is_ascii_whitespace() {
            prev -= 1;
        }
        if prev == 0 || !is_word_byte(bytes[prev - 1]) {
            continue;
        }

        let mut after = idx + 1;
        while after < bytes.len() && bytes[after].is_ascii_whitespace() {
            after += 1;
        }
        if after == idx + 1 {
            continue;
        }

        if is_line_continuation(&line[after..]) {
            continue;
        }
        return true;
    }
    false
}

fn has_extra_space_after_open_paren(line: &str) -> bool {
    let bytes = line.as_bytes();
    for (idx, byte) in bytes.iter().enumerate() {
        if *byte != b'(' {
            continue;
        }

        let mut after = idx + 1;
        while after < bytes.len() && bytes[after].is_ascii_whitespace() {
            after += 1;
        }
        if after == idx + 1 {
            continue;
        }

        let rest = &line[after..];
        if rest.starts_with('(') {
            if after.saturating_sub(idx + 1) > 1 {
                return true;
            }
            continue;
        }
        if is_line_continuation(rest) {
            continue;
        }
        return true;
    }
    false
}

fn has_extra_space_after_leading_nested_open_paren(line: &str) -> bool {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('(') {
        return false;
    }

    let rest = &trimmed[1..];
    let space_count = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();
    space_count > 1 && rest[space_count..].starts_with('(')
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_operator_spacing(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines,
    elided_line: &str,
    linenum: usize,
    keywords: &MatchedKeywords,
) {
    if !elided_line.as_bytes().iter().any(|&b| {
        matches!(
            b,
            b'=' | b'<'
                | b'>'
                | b'!'
                | b'~'
                | b'+'
                | b'-'
                | b'*'
                | b'/'
                | b'%'
                | b'&'
                | b'|'
                | b'^'
        )
    }) {
        return;
    }

    let mut masked_line: std::borrow::Cow<'_, str> = std::borrow::Cow::Borrowed(elided_line);
    if keywords.has_operator()
        && let Some(captures) = OPERATOR_METHOD_RE.captures(elided_line)
    {
        let prefix = captures.get(1).map_or("", |m| m.as_str());
        let operator = captures.get(2).map_or("", |m| m.as_str());
        let suffix = captures.get(3).map_or("", |m| m.as_str());
        let mut replaced = String::with_capacity(prefix.len() + operator.len() + suffix.len());
        replaced.push_str(prefix);
        replaced.extend(std::iter::repeat_n('_', operator.len()));
        replaced.push_str(suffix);
        masked_line = std::borrow::Cow::Owned(replaced);
    }

    let raw_trimmed = clean_lines.raw_lines[linenum].trim();
    if raw_trimmed.starts_with("/*! ")
        && raw_trimmed.contains("*/")
        && (raw_trimmed.contains("http://") || raw_trimmed.contains("https://"))
        && linter.facts().namespace_top_level_depth(linenum).is_some()
    {
        linter.error(
            linenum,
            "whitespace/operators",
            4,
            "Extra space for operator ! ",
        );
        return;
    }

    let line_to_check = masked_line.as_ref();

    if line_to_check.contains('=')
        && !CONTROL_BLOCK_RE.is_match(line_to_check)
        && !line_to_check.contains("operator=")
        && has_missing_assignment_space(line_to_check)
    {
        linter.error(
            linenum,
            "whitespace/operators",
            4,
            "Missing spaces around =",
        );
    }

    if let Some(op) = find_missing_comparison_space(line_to_check) {
        linter.error(
            linenum,
            "whitespace/operators",
            3,
            &format!("Missing spaces around {}", op),
        );
    } else if !line_to_check.starts_with('#') || !line_to_check.contains("include") {
        if line_to_check.contains('<')
            && let Some(captures) = LESS_SPACING_RE.captures(line_to_check)
        {
            let end_pos = captures.get(1).map(|m| m.end()).unwrap_or(0);
            if crate::line_utils::close_expression(clean_lines, linenum, end_pos).is_none() {
                linter.error(
                    linenum,
                    "whitespace/operators",
                    3,
                    "Missing spaces around <",
                );
            }
        }

        if line_to_check.contains('>')
            && let Some(captures) = GREATER_SPACING_RE.captures(line_to_check)
        {
            let start_pos = captures.get(1).map(|m| m.end()).unwrap_or(0);
            if crate::line_utils::reverse_close_expression(clean_lines, linenum, start_pos)
                .is_none()
            {
                linter.error(
                    linenum,
                    "whitespace/operators",
                    3,
                    "Missing spaces around >",
                );
            }
        }
    }

    if let Some(captures) = LSHIFT_SPACING_RE.captures(line_to_check) {
        let left = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        let right = captures.get(2).map(|m| m.as_str()).unwrap_or("");
        let left_is_digit = left.len() == 1 && left.as_bytes()[0].is_ascii_digit();
        let right_is_digit = right.len() == 1 && right.as_bytes()[0].is_ascii_digit();
        let operator_semicolon = left == "operator" && right == ";";
        if !(operator_semicolon || left_is_digit && right_is_digit) {
            linter.error(
                linenum,
                "whitespace/operators",
                3,
                "Missing spaces around <<",
            );
        }
    }

    if RSHIFT_SPACING_RE.is_match(line_to_check) {
        linter.error(
            linenum,
            "whitespace/operators",
            3,
            "Missing spaces around >>",
        );
    }

    if let Some(op) = find_extra_unary_space(line_to_check) {
        linter.error(
            linenum,
            "whitespace/operators",
            4,
            &format!("Extra space for operator {}", op),
        );
    }
}

fn has_missing_assignment_space(s: &str) -> bool {
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'=' {
            // Check for compound assignments or comparisons (e.g., +=, ==, !=, >=)
            if i > 0 {
                let prev = bytes[i - 1];
                if matches!(
                    prev,
                    b'>' | b'<'
                        | b'='
                        | b'!'
                        | b'&'
                        | b'^'
                        | b'|'
                        | b'+'
                        | b'-'
                        | b'*'
                        | b'/'
                        | b'%'
                ) {
                    continue;
                }
            }
            if let Some(&next) = bytes.get(i + 1) {
                if next == b'=' {
                    continue;
                }
            }

            let mut missing = false;
            if i > 0 {
                let prev = bytes[i - 1];
                if (prev.is_ascii_alphanumeric() || prev == b'.')
                    && (i < 8 || &s[i - 8..i] != "operator")
                {
                    missing = true;
                }
            }
            if !missing && i + 1 < bytes.len() {
                let next = bytes[i + 1];
                if next.is_ascii_alphanumeric() || next == b'.' {
                    missing = true;
                }
            }
            if missing {
                return true;
            }
        }
    }
    false
}

fn find_missing_comparison_space(s: &str) -> Option<&'static str> {
    let bytes = s.as_bytes();
    if bytes.len() < 4 {
        return None;
    }

    for (i, window) in bytes.windows(2).enumerate() {
        let op = match window {
            b"==" => "==",
            b"!=" => "!=",
            b"<=" => "<=",
            b">=" => ">=",
            b"||" => "||",
            _ => continue,
        };

        // op is at i, i+1
        if i > 0 && i + 2 < bytes.len() {
            let prev = bytes[i - 1];
            let next = bytes[i + 2];

            let prev_is_op_char =
                matches!(prev, b'<' | b'>' | b'=' | b'!' | b'|') || prev.is_ascii_whitespace();
            let next_is_op_char =
                matches!(next, b'<' | b'>' | b'=' | b'!' | b'|' | b',' | b';' | b')')
                    || next.is_ascii_whitespace();

            if !prev_is_op_char && !next_is_op_char {
                return Some(op);
            }
        }
    }
    None
}

fn find_extra_unary_space(s: &str) -> Option<&'static str> {
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'!' | b'~' => {
                if let Some(&next) = bytes.get(i + 1) {
                    if next.is_ascii_whitespace() {
                        return Some(if b == b'!' { "!" } else { "~" });
                    }
                }
            }
            b'-' | b'+' => {
                // Check for -- or ++
                if let Some(&next) = bytes.get(i + 1) {
                    if next == b {
                        // We found -- or ++ at i, i+1
                        // Original regex: [\s]--[\s;]
                        if i > 0 && bytes[i - 1].is_ascii_whitespace() {
                            if let Some(&after) = bytes.get(i + 2) {
                                if after.is_ascii_whitespace() || after == b';' {
                                    return Some(if b == b'-' { "--" } else { "++" });
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_parenthesis_spacing(
    linter: &mut FileLinter,
    elided_line: &str,
    raw_line: &str,
    linenum: usize,
    keywords: &MatchedKeywords,
) {
    if keywords.has_if() || keywords.has_for() || keywords.has_while() || keywords.has_switch() {
        if let Some(captures) = CONTROL_PARENS_MISSING_SPACE_RE.captures(elided_line) {
            linter.error(
                linenum,
                "whitespace/parens",
                5,
                &format!(
                    "Missing space before ( in {}",
                    captures.get(1).map(|m| m.as_str()).unwrap_or("")
                ),
            );
        }

        if let Some(captures) = CONTROL_PARENS_SPACE_RE.captures(elided_line) {
            let keyword = captures.get(1).map(|m| m.as_str()).unwrap_or("");
            let left_spaces = captures.get(2).map(|m| m.as_str().len()).unwrap_or(0);
            let first_char = captures.get(3).map(|m| m.as_str()).unwrap_or("");
            let right_spaces = captures.get(4).map(|m| m.as_str().len()).unwrap_or(0);
            let for_closing_semicolon_exception = keyword == "for"
                && left_spaces == 0
                && FOR_CLOSING_SEMICOLON_EXCEPTION_RE.is_match(elided_line);
            let for_opening_semicolon_exception =
                keyword == "for" && first_char == ";" && left_spaces == 1 + right_spaces;

            if left_spaces != right_spaces
                && !for_closing_semicolon_exception
                && !for_opening_semicolon_exception
                && !raw_line.contains("/*")
            {
                linter.error(
                    linenum,
                    "whitespace/parens",
                    5,
                    &format!("Mismatching spaces inside () in {}", keyword),
                );
            }
            if left_spaces != 0 && left_spaces != 1 {
                linter.error(
                    linenum,
                    "whitespace/parens",
                    5,
                    &format!(
                        "Should have zero or one spaces inside ( and ) in {}",
                        keyword
                    ),
                );
            }
        }
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_spacing_for_function_call(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines,
    elided_line: &str,
    raw_line: &str,
    linenum: usize,
    keywords: &MatchedKeywords,
) {
    if !elided_line.contains('(') && !elided_line.contains(')') {
        return;
    }
    if keywords.has_if() || keywords.has_for() || keywords.has_switch() {
        if let Some(captures) = IF_FOR_SWITCH_CALL_RE.captures(elided_line) {
            check_spacing_for_function_call_base(
                linter,
                elided_line,
                captures.get(2).map(|m| m.as_str()).unwrap_or(""),
                raw_line,
                linenum,
                keywords,
            );
        }
    } else if keywords.has_while()
        && let Some(captures) = WHILE_CALL_RE.captures(elided_line)
    {
        check_spacing_for_function_call_base(
            linter,
            elided_line,
            captures.get(1).map(|m| m.as_str()).unwrap_or(""),
            raw_line,
            linenum,
            keywords,
        );
        return;
    }

    check_spacing_for_function_call_base(
        linter,
        elided_line,
        elided_line,
        raw_line,
        linenum,
        keywords,
    );
    if raw_line.trim_end() != raw_line && raw_line.trim_end().ends_with('(') {
        check_spacing_for_function_call_base(
            linter, raw_line, raw_line, raw_line, linenum, keywords,
        );
    }
    if clean_lines.has_comment[linenum]
        && clean_lines.elided[linenum].trim().is_empty()
        && is_interior_block_comment_line(raw_line)
    {
        check_spacing_for_function_call_base(
            linter, raw_line, raw_line, raw_line, linenum, keywords,
        );
    }
}

fn is_interior_block_comment_line(raw_line: &str) -> bool {
    let trimmed = raw_line.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with("//")
        && !trimmed.starts_with("/*")
        && !trimmed.starts_with('*')
}

fn check_spacing_for_function_call_base(
    linter: &mut FileLinter,
    line: &str,
    fncall: &str,
    raw_line: &str,
    linenum: usize,
    keywords: &MatchedKeywords,
) {
    if (keywords.has_if()
        || keywords.has_elif()
        || keywords.has_for()
        || keywords.has_while()
        || keywords.has_switch()
        || keywords.has_return()
        || keywords.has_new()
        || keywords.has_delete()
        || keywords.has_catch()
        || keywords.has_sizeof())
        && CONTROL_STRUCT_RE.is_match(fncall)
    {
        return;
    }

    if FUNC_REF_RE.is_match(fncall) || ARRAY_REF_RE.is_match(fncall) {
        return;
    }

    if has_extra_space_after_function_call_paren(fncall) {
        linter.error(
            linenum,
            "whitespace/parens",
            4,
            "Extra space after ( in function call",
        );
    } else if has_extra_space_after_leading_nested_open_paren(fncall)
        || has_extra_space_after_open_paren(fncall)
    {
        linter.error(linenum, "whitespace/parens", 2, "Extra space after (");
    }

    if EXTRA_SPACE_BEFORE_CALL_PAREN_RE.is_match(fncall)
        && (!keywords.has_va_opt() || !ASM_VOLATILE_CALL_RE.is_match(fncall))
        && (!keywords.has_typedef() && !keywords.has_using()
            || !DEFINE_TYPEDEF_USING_ASSIGN_RE.is_match(fncall))
        && !FUNCTION_POINTER_CALL_RE.is_match(fncall)
        && (!keywords.has_case() || !CASE_PAREN_RE.is_match(fncall))
    {
        let confidence = if keywords.has_operator() && OPERATOR_NAME_RE.is_match(line) {
            0
        } else {
            4
        };
        linter.error(
            linenum,
            "whitespace/parens",
            confidence,
            "Extra space before ( in function call",
        );
    }

    if !EXTRA_SPACE_BEFORE_CLOSE_PAREN_RE.is_match(fncall) {
        return;
    }
    if raw_line.contains("/*") {
        return;
    }

    if fncall.chars().next().is_some_and(char::is_whitespace)
        && fncall.trim_start().starts_with(')')
    {
        linter.error(
            linenum,
            "whitespace/parens",
            2,
            "Closing ) should be moved to the previous line",
        );
    } else {
        linter.error(linenum, "whitespace/parens", 2, "Extra space before )");
    }
}

fn is_braced_initialization(
    clean_lines: &CleansedLines,
    elided_line: &str,
    linenum: usize,
) -> bool {
    let Some(captures) = OPEN_BRACE_NEEDS_SPACE_RE.captures(elided_line) else {
        return false;
    };
    let leading_text = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    let start_pos = captures.get(1).map(|m| m.end()).unwrap_or(0);
    let Some((end_linenum, end_pos)) =
        crate::line_utils::close_expression(clean_lines, linenum, start_pos)
    else {
        return false;
    };

    let trailing_limit = usize::min(end_linenum + 3, clean_lines.elided.len());
    let mut trailing_text = Cow::Borrowed(&clean_lines.elided[end_linenum][end_pos..]);
    if end_linenum + 1 < trailing_limit {
        let extra_capacity = (end_linenum + 1..trailing_limit)
            .map(|offset| clean_lines.elided[offset].len())
            .sum::<usize>();
        let mut owned = String::with_capacity(trailing_text.len() + extra_capacity);
        owned.push_str(trailing_text.as_ref());
        for offset in end_linenum + 1..trailing_limit {
            owned.push_str(&clean_lines.elided[offset]);
        }
        trailing_text = Cow::Owned(owned);
    }

    let leading_trimmed = leading_text.trim_end();
    if leading_trimmed == "namespace" || leading_trimmed.starts_with("namespace ") {
        return false;
    }
    BRACED_INIT_TRAILING_RE.is_match(trailing_text.as_ref())
        || (!leading_trimmed.ends_with(')') && !leading_trimmed.ends_with(']'))
            && looks_like_type_name(leading_text)
}

fn looks_like_type_name(expr: &str) -> bool {
    let token = expr
        .split_whitespace()
        .last()
        .unwrap_or(expr)
        .trim_end_matches(['&', '*', ':', '(']);
    let token = token.rsplit("::").next().unwrap_or(token);

    matches!(
        token,
        "bool"
            | "char"
            | "double"
            | "float"
            | "int"
            | "long"
            | "short"
            | "signed"
            | "unsigned"
            | "size_t"
            | "ptrdiff_t"
            | "uint8_t"
            | "uint16_t"
            | "uint32_t"
            | "uint64_t"
            | "int8_t"
            | "int16_t"
            | "int32_t"
            | "int64_t"
    ) || token.ends_with("_t")
        || token
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_blank_line_rules(linter: &mut FileLinter, clean_lines: &CleansedLines, linenum: usize) {
    let line = &clean_lines.lines_without_raw_strings[linenum];
    if !crate::line_utils::is_blank_line(line) {
        return;
    }
    if linenum == 0 {
        return;
    }
    let prev_raw = clean_lines.raw_lines[linenum - 1].trim();
    let prev_is_comment = prev_raw.starts_with("//")
        || prev_raw == "/*"
        || prev_raw.starts_with('*')
        || prev_raw.ends_with("*/");

    let prev_line = &clean_lines.elided[linenum - 1];
    let prev_raw_line = clean_lines.raw_lines[linenum - 1].trim();
    if (crate::line_utils::namespace_decl_start_line(&clean_lines.elided, linenum - 1).is_some()
        && prev_line.trim_end().ends_with('{'))
        || (prev_raw_line.starts_with("extern ") && prev_raw_line.ends_with('{'))
    {
        return;
    }

    if let Some(prevbrace) = prev_line.rfind('{')
        && !prev_line[prevbrace..].contains('}')
    {
        if prev_is_comment {
            return;
        }
        let exception = if INITLIST_CONTINUATION_RE.is_match(prev_line) {
            let mut search_position = linenum.checked_sub(2);
            while let Some(position) = search_position {
                if !INITLIST_CONTINUATION_RE.is_match(&clean_lines.elided[position]) {
                    break;
                }
                search_position = position.checked_sub(1);
            }
            search_position
                .map(|position| clean_lines.elided[position].starts_with("    :"))
                .unwrap_or(false)
        } else {
            FUNCTION_HEADER_BLANK_LINE_RE.is_match(prev_line)
                || INITLIST_HEADER_BLANK_LINE_RE.is_match(prev_line)
        };

        if !exception {
            linter.error(
                linenum,
                "whitespace/blank_line",
                2,
                "Redundant blank line at the start of a code block should be deleted.",
            );
        }
    }

    if let Some(captures) = ACCESS_SPECIFIER_RE.captures(prev_line) {
        linter.error(
            linenum,
            "whitespace/blank_line",
            3,
            &format!(
                "Do not leave a blank line after \"{}:\"",
                captures.get(2).map(|m| m.as_str()).unwrap_or("")
            ),
        );
    }

    if let Some(next_line) = clean_lines.lines_without_raw_strings.get(linenum + 1)
        && !next_line.is_empty()
        && next_line.trim_start().starts_with('}')
        && !next_line.contains("} else ")
    {
        let closes_extern_block = linter
            .facts()
            .matching_block_start(linenum + 1)
            .is_some_and(|start| {
                let start_line = clean_lines.raw_lines[start].trim();
                start_line.starts_with("extern ") && start_line.ends_with('{')
            });
        let closes_namespace_block = linter
            .facts()
            .matching_block_start(linenum + 1)
            .is_some_and(|start| {
                crate::line_utils::namespace_decl_start_line(&clean_lines.elided, start).is_some()
            });
        if closes_extern_block || closes_namespace_block {
            return;
        }
        linter.error(
            linenum,
            "whitespace/blank_line",
            3,
            "Redundant blank line at the end of a code block should be deleted.",
        );
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_section_spacing(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines,
    linenum: usize,
    keywords: &MatchedKeywords,
) {
    if !keywords.has_access() {
        return;
    }
    let line = &clean_lines.lines_without_raw_strings[linenum];
    let Some(captures) = ACCESS_SPECIFIER_RE.captures(line) else {
        return;
    };

    let Some(class_range) = linter.facts().enclosing_class_range(linenum) else {
        return;
    };
    let class_start = class_range.start;
    let class_end = class_range.end;

    if class_end.saturating_sub(class_start) <= 24 || linenum <= class_start {
        return;
    }
    if !clean_lines.lines_without_raw_strings[class_start].contains('{') {
        return;
    }
    let only_class_head_before_section = clean_lines
        .raw_lines
        .iter()
        .zip(clean_lines.elided.iter())
        .skip(class_start + 1)
        .take(linenum.saturating_sub(class_start + 1))
        .all(|(raw, elided)| {
            let raw_trimmed = raw.trim();
            let trimmed = elided.trim();
            raw_trimmed.is_empty()
                || trimmed.is_empty()
                || raw_trimmed.starts_with("//")
                || raw_trimmed.starts_with("/*")
                || trimmed == "{"
                || trimmed.starts_with(':')
                || trimmed.starts_with("public ")
                || trimmed.starts_with("private ")
                || trimmed.starts_with("protected ")
                || trimmed.starts_with("template <")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("struct ")
                || trimmed.ends_with('{')
        });
    if only_class_head_before_section {
        return;
    }

    let prev_line = &clean_lines.lines_without_raw_strings[linenum - 1];
    if crate::line_utils::is_blank_line(prev_line)
        || clean_lines.raw_lines[linenum - 1]
            .trim_start()
            .starts_with("//")
        || clean_lines.raw_lines[linenum - 1]
            .trim_start()
            .starts_with("/*")
        || contains_class_or_struct_word(prev_line)
        || prev_line.ends_with('\\')
    {
        return;
    }

    let mut end_class_head = class_start;
    for i in class_start..linenum {
        if clean_lines.lines_without_raw_strings[i]
            .trim_end()
            .ends_with('}')
        {
            end_class_head = i;
            break;
        }
    }

    if end_class_head < linenum.saturating_sub(1) {
        linter.error(
            linenum,
            "whitespace/blank_line",
            3,
            &format!(
                "\"{}:\" should be preceded by a blank line",
                captures.get(2).map(|m| m.as_str()).unwrap_or("")
            ),
        );
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_access_specifier_indentation(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines,
    linenum: usize,
    keywords: &MatchedKeywords,
) {
    if !keywords.has_access() {
        return;
    }
    let raw_line = clean_lines.raw_lines[linenum].trim_start();
    if raw_line.starts_with("//") || raw_line.starts_with("/*") {
        return;
    }
    let line = &clean_lines.lines_without_raw_strings[linenum];
    let Some(captures) = ACCESS_SPECIFIER_RE.captures(line) else {
        return;
    };
    let Some(class_range) = linter.facts().enclosing_class_range(linenum) else {
        return;
    };

    let prefix = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    let class_indent = crate::line_utils::get_indent_level(
        &clean_lines.lines_without_raw_strings[class_range.start],
    );
    if prefix.len() == class_indent + 1 && prefix.chars().all(|ch| ch == ' ') {
        return;
    }
    if class_indent == 0 && prefix == "\t" {
        return;
    }

    let kind = if linter
        .facts()
        .enclosing_class_is_struct(linenum)
        .unwrap_or(false)
    {
        "struct"
    } else {
        "class"
    };
    let parent = match linter.facts().nearest_class_name(linenum) {
        Some(name) if !name.is_empty() => format!("{} {}", kind, name),
        _ => kind.to_string(),
    };
    let access = captures.get(2).map(|m| m.as_str()).unwrap_or("");
    let slots = captures.get(3).map(|m| m.as_str()).unwrap_or("");
    linter.error(
        linenum,
        "whitespace/indent",
        3,
        &format!(
            "{}{}: should be indented +1 space inside {}",
            access, slots, parent
        ),
    );
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_class_closing_brace_alignment(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines,
    linenum: usize,
) {
    let Some(class_range) = linter.facts().enclosing_class_range(linenum) else {
        return;
    };
    if linenum != class_range.end {
        return;
    }

    let line = &clean_lines.lines_without_raw_strings[linenum];
    if !line.trim_start().starts_with('}') {
        return;
    }

    let class_indent = crate::line_utils::get_indent_level(
        &clean_lines.lines_without_raw_strings[class_range.start],
    );
    let closing_indent = crate::line_utils::get_indent_level(line);
    if closing_indent == class_indent {
        return;
    }

    let kind = if linter
        .facts()
        .enclosing_class_is_struct(linenum)
        .unwrap_or(false)
    {
        "struct"
    } else {
        "class"
    };
    let parent = match linter.facts().nearest_class_name(linenum) {
        Some(name) if !name.is_empty() => format!("{} {}", kind, name),
        _ => kind.to_string(),
    };
    linter.error(
        linenum,
        "whitespace/indent",
        3,
        &format!(
            "Closing brace should be aligned with beginning of {}",
            parent
        ),
    );
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_tabs_and_line_length(
    linter: &mut FileLinter,
    raw_line: &str,
    line_without_raw_strings: &str,
    linenum: usize,
) {
    if raw_line.contains('\t') {
        linter.error(
            linenum,
            "whitespace/tab",
            1,
            "Tab found; better to use spaces",
        );
    }

    let width = UnicodeWidthStr::width(line_without_raw_strings);
    if width > linter.options().line_length && !should_skip_line_length(line_without_raw_strings) {
        linter.error(
            linenum,
            "whitespace/line_length",
            2,
            &format!(
                "Lines should be <= {} characters long",
                linter.options().line_length
            ),
        );
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_indentation(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines,
    raw_line: &str,
    line: &str,
    linenum: usize,
) {
    if raw_line.ends_with(' ') || raw_line.ends_with('\t') {
        linter.error(
            linenum,
            "whitespace/end_of_line",
            4,
            "Line ends in whitespace.  Consider deleting these extra spaces.",
        );
    }

    let comment_only_line = clean_lines.has_comment[linenum] && line.trim().is_empty();
    let indent_line = if comment_only_line { raw_line } else { line };
    let initial_spaces = crate::line_utils::get_indent_level(raw_line);
    let prev_line_allows_continuation = linenum > 0
        && PREV_LINE_CONTINUATION_RE.is_match(&clean_lines.lines_without_raw_strings[linenum - 1]);
    let is_scope_or_label = SCOPE_OR_LABEL_RE.is_match(indent_line);
    let is_raw_string_line =
        clean_lines.raw_lines[linenum].as_str() != line && line.trim_start().starts_with("\"\"");
    let should_check_indent = !raw_line.trim().is_empty() || initial_spaces > 0;

    if should_check_indent
        && !prev_line_allows_continuation
        && (initial_spaces == 1 || initial_spaces == 3)
        && !is_scope_or_label
        && !is_raw_string_line
    {
        linter.error(
            linenum,
            "whitespace/indent",
            3,
            "Weird number of spaces at line-start.  Are you using a 2-space indent?",
        );
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn check(linter: &mut FileLinter, clean_lines: &CleansedLines, linenum: usize) {
    let raw_line = &clean_lines.raw_lines[linenum];
    let line_without_raw_strings = &clean_lines.lines_without_raw_strings[linenum];
    let line = &clean_lines.lines[linenum];
    let elided_line = &clean_lines.elided[linenum];

    let keywords = MatchedKeywords::from_line(elided_line);

    check_comment_spacing(linter, clean_lines, linenum);
    check_blank_line_rules(linter, clean_lines, linenum);
    if linenum > 0 {
        check_section_spacing(linter, clean_lines, linenum, &keywords);
    }
    check_access_specifier_indentation(linter, clean_lines, linenum, &keywords);
    check_class_closing_brace_alignment(linter, clean_lines, linenum);

    check_tabs_and_line_length(linter, raw_line, line_without_raw_strings, linenum);

    check_indentation(linter, clean_lines, raw_line, line, linenum);

    // 5. Check for redundant space before [
    if has_extra_space_before_bracket(elided_line) {
        linter.error(
            linenum,
            r#"whitespace/braces"#,
            5,
            r#"Extra space before ["#,
        );
    }

    // 6. Check for space around colon in range-based for
    if keywords.has_for()
        && (RANGE_FOR_COLON_LEFT_RE.is_match(elided_line)
            || RANGE_FOR_COLON_RIGHT_RE.is_match(elided_line))
    {
        linter.error(
            linenum,
            r#"whitespace/forcolon"#,
            2,
            r#"Missing space around colon in range-based for loop"#,
        );
    }

    // 7. Check operator spacing.
    check_operator_spacing(linter, clean_lines, elided_line, linenum, &keywords);

    // 8. Check paren spacing.
    check_parenthesis_spacing(linter, elided_line, raw_line, linenum, &keywords);
    check_spacing_for_function_call(
        linter,
        clean_lines,
        elided_line,
        raw_line,
        linenum,
        &keywords,
    );

    // 9. Check comma and semicolon spacing.
    let comma_check_line: Cow<'_, str> = if keywords.has_operator() || keywords.has_va_opt() {
        let replaced = VA_OPT_COMMA_RE.replace_all(elided_line, "");
        Cow::Owned(
            OPERATOR_COMMA_CALL_RE
                .replace_all(&replaced, "F(")
                .into_owned(),
        )
    } else {
        Cow::Borrowed(elided_line)
    };

    if MISSING_SPACE_AFTER_COMMA_RE.is_match(comma_check_line.as_ref())
        && MISSING_SPACE_AFTER_COMMA_RE.is_match(line)
    {
        linter.error(
            linenum,
            r#"whitespace/comma"#,
            3,
            r#"Missing space after ,"#,
        );
    }

    let semicolon_before_block_comment = raw_line.contains(";/*");
    if MISSING_SPACE_AFTER_SEMICOLON_RE.is_match(elided_line) || semicolon_before_block_comment {
        let mut target_linenum = linenum;
        if semicolon_before_block_comment && !raw_line.contains("*/") {
            while target_linenum + 1 < clean_lines.raw_lines.len() {
                target_linenum += 1;
                if clean_lines.raw_lines[target_linenum].contains("*/") {
                    break;
                }
            }
        }
        linter.error(
            target_linenum,
            r#"whitespace/semicolon"#,
            3,
            r#"Missing space after ;"#,
        );
    }

    let semicolon_count = elided_line.chars().filter(|&c| c == ';').count();
    let prev_line = if linenum > 0 {
        crate::line_utils::get_previous_non_blank_line(
            &clean_lines.lines_without_raw_strings,
            linenum,
        )
        .map(|(_, line)| line)
        .unwrap_or("")
    } else {
        ""
    };
    let switch_case_single_line =
        (keywords.has_case() || keywords.has_default()) && elided_line.contains("break;");
    if semicolon_count > 1
        && !MULTI_COMMAND_INITLIST_RE.is_match(line)
        && !keywords.has_for()
        && (!prev_line.contains("for") || prev_line.contains(';'))
        && !switch_case_single_line
    {
        linter.error(
            linenum,
            "whitespace/newline",
            0,
            "More than one command on the same line",
        );
    }

    // 10. Brace and semicolon spacing.
    let missing_space_before_qualified_brace = QUALIFIED_BRACE_RE.is_match(elided_line);
    if OPEN_BRACE_NEEDS_SPACE_RE.is_match(elided_line)
        && (!is_braced_initialization(clean_lines, elided_line, linenum)
            || missing_space_before_qualified_brace)
        && !FIXED_WIDTH_BRACED_INT_RE.is_match(elided_line)
    {
        linter.error(
            linenum,
            r#"whitespace/braces"#,
            5,
            r#"Missing space before {"#,
        );
    }

    if elided_line.contains("}else") {
        linter.error(
            linenum,
            r#"whitespace/braces"#,
            5,
            r#"Missing space before else"#,
        );
    }

    if EMPTY_STATEMENT_AFTER_COLON_RE.is_match(elided_line) {
        linter.error(
            linenum,
            r#"whitespace/semicolon"#,
            5,
            r#"Semicolon defining empty statement. Use {} instead."#,
        );
    } else if ONLY_SEMICOLON_RE.is_match(elided_line) {
        linter.error(
            linenum,
            r#"whitespace/semicolon"#,
            5,
            r#"Line contains only semicolon. If this should be an empty statement, use {} instead."#,
        );
    } else if SPACE_BEFORE_LAST_SEMICOLON_RE.is_match(elided_line)
        && !string_utils::contains_word(elided_line, "for")
    {
        linter.error(
            linenum,
            r#"whitespace/semicolon"#,
            5,
            r#"Extra space before last semicolon. If this should be an empty statement, use {} instead."#,
        );
    }
}

pub fn check_eof_newline(linter: &mut FileLinter, raw_lines: &[String]) {
    if raw_lines.is_empty() {
        return;
    }

    let last_line = &raw_lines[raw_lines.len() - 1];
    if !last_line.is_empty() {
        linter.error(
            raw_lines.len() - 1,
            "whitespace/ending_newline",
            5,
            "Could not find a newline character at the end of the file.",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bracket_space_helper_skips_exceptions() {
        assert!(has_extra_space_before_bracket("value [index]"));
        assert!(!has_extra_space_before_bracket("value[[index]]"));
        assert!(!has_extra_space_before_bracket("return [value]"));
        assert!(!has_extra_space_before_bracket("auto& [x, y] = pair;"));
    }

    #[test]
    fn paren_space_helpers_match_expected_cases() {
        assert!(has_extra_space_after_function_call_paren("call( value)"));
        assert!(!has_extra_space_after_function_call_paren("call( \\"));
        assert!(has_extra_space_after_open_paren("( value)"));
        assert!(!has_extra_space_after_open_paren("( \\"));
    }
}
