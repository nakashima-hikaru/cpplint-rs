use crate::categories::Category;
use crate::cleanse::{CleansedLines, LineFeatures, MatchedKeywords};
use crate::facts::FileFacts;
use crate::file_linter::FileLinter;
use crate::string_utils;
use aho_corasick::AhoCorasick;
use regex::{Regex, RegexSet};
use std::borrow::Cow;
use std::sync::LazyLock;
use unicode_width::UnicodeWidthStr;

static TODO_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^//(\s*)TODO(\(.+?\))?:?(\s|$)?"#).unwrap());
/// Parses an access specifier (`public:`, `private:`, `protected:`, `signals:`).
/// Also allows optional `slots` after the specifier (e.g., `public slots:`).
/// Returns `(prefix_len, specifier_string, has_slots)` if matched.
fn parse_access_specifier(line: &str) -> Option<(usize, &'static str, bool)> {
    let bytes = line.as_bytes();
    for specifier in &["public", "private", "protected", "signals"] {
        let mut search_start = 0;
        while let Some(relative_pos) = line[search_start..].find(specifier) {
            let pos = search_start + relative_pos;
            search_start = pos + specifier.len();

            // Check word boundary before and after
            if pos > 0 && string_utils::is_word_char(bytes[pos - 1]) {
                continue;
            }
            if pos + specifier.len() < bytes.len()
                && string_utils::is_word_char(bytes[pos + specifier.len()])
            {
                continue;
            }

            // After specifier: optional spaces, optional `slots`, optional spaces, then `:`
            let mut i = pos + specifier.len();
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }

            let mut has_slots = false;
            if line[i..].starts_with("slots") {
                let slots_end = i + 5;
                // Ensure `slots` is isolated
                if slots_end >= bytes.len() || !string_utils::is_word_char(bytes[slots_end]) {
                    has_slots = true;
                    i = slots_end;
                    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                        i += 1;
                    }
                }
            }

            if i < bytes.len() && bytes[i] == b':' {
                // Must not be followed by another colon (not `::`)
                if i + 1 >= bytes.len() || bytes[i + 1] != b':' {
                    return Some((pos, *specifier, has_slots));
                }
            }
        }
    }
    None
}

static CONTROL_STRUCT_AC: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::new([
        "if", "elif", "for", "while", "switch", "return", "new", "delete", "catch", "sizeof",
    ])
    .unwrap()
});
/// Manual replacement for REF_MATCHERS.
/// Detects ` (...)(...` or ` (...)\[...` patterns that indicate function/array reference calls.
/// Original patterns:
///   FUNC_REF:  ` \([^)]+\)\([^)]*(\)|,$)`
///   ARRAY_REF: ` \([^)]+\)\[[^\]]+\]`
fn has_ref_call(fncall: &str) -> bool {
    // Quick pre-check: needs both '(' and either another '(' or '[' after a ')'
    if !fncall.contains(' ') {
        return false;
    }
    let bytes = fncall.as_bytes();
    let mut i = 1usize; // start at 1 so we can check bytes[i-1]
    while i < bytes.len() {
        if bytes[i] != b'(' || bytes[i - 1] != b' ' {
            i += 1;
            continue;
        }
        // Found ` (` at position i.
        // Find the matching ')'
        let start = i;
        let mut depth = 1usize;
        let mut j = start + 1;
        while j < bytes.len() && depth > 0 {
            match bytes[j] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            j += 1;
        }
        if depth != 0 {
            // Unmatched paren — no ref call here
            i += 1;
            continue;
        }
        // j points one past the closing ')'. Check for '(' or '[' immediately after.
        if j < bytes.len() && (bytes[j] == b'(' || bytes[j] == b'[') {
            if bytes[j] == b'[' {
                // ARRAY_REF: ` (...)\[...\]` — just need at least one char before ']'
                let k = j + 1;
                if let Some(close) = memchr::memchr(b']', &bytes[k..])
                    && close > 0
                {
                    return true;
                }
            } else {
                // FUNC_REF: ` (...)(...)` where inner must end with ')' or ','
                let inner_start = j;
                let mut depth2 = 1usize;
                let mut k = inner_start + 1;
                while k < bytes.len() && depth2 > 0 {
                    match bytes[k] {
                        b'(' => depth2 += 1,
                        b')' => depth2 -= 1,
                        _ => {}
                    }
                    k += 1;
                }
                if depth2 == 0 {
                    let after = bytes.get(k).copied();
                    if matches!(after, None | Some(b')') | Some(b',')) {
                        return true;
                    }
                }
                // If depth2 != 0 (unmatched inner paren to EOF), NOT a ref call on this line
            }
        }
        i = start + 1;
    }
    false
}

/// Manual replacement for OPERATOR_NAME_RE (`\boperator_*\b`).
fn has_operator_name(line: &str) -> bool {
    let mut offset = 0;
    while let Some(pos) = line[offset..].find("operator") {
        let start = offset + pos;
        let end = start + 8;
        if string_utils::is_word_match(line, start, end) {
            return true;
        }
        offset = start + 1;
    }
    false
}

static COMMENT_SPACING_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r#"^//[^ ]*\w"#,        // 0: COMMENT_WITHOUT_SPACE
        r#"^(///|//!)(\s+|$)"#, // 1: DOC_COMMENT
    ])
    .unwrap()
});
static PREV_LINE_CONTINUATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[\",=><] *$"#).unwrap());
static RANGE_FOR_COLON_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r#"for\s*\(.*[^:]:[^: ]"#, // 0: LEFT
        r#"for\s*\(.*[^: ]:[^:]"#, // 1: RIGHT
    ])
    .unwrap()
});
static SCOPE_OR_LABEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*(?:public|private|protected|signals)(?:\s+(?:slots\s*)?)?:\s*\\?\s*$"#)
        .unwrap()
});

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
/// Result of `CallSpacingFlags::scan(fncall)`.  A compact replacement for CALL_SPACING_SET.
#[derive(Default)]
struct CallSpacingFlags {
    /// Pattern 0: `\w\s+(` — word-char followed by space(s) before `(`
    main: bool,
    /// Pattern 1: asm volatile pattern
    asm: bool,
    /// Pattern 2: `#define`, `typedef`, or `using \w+ =`
    define: bool,
    /// Pattern 3: `\w\s+(…::)*\*\w+\)(`  — function-pointer cast
    func_ptr: bool,
    /// Pattern 4: `\bcase\s+(`
    case: bool,
}

impl CallSpacingFlags {
    fn scan(s: &str) -> Self {
        let bytes = s.as_bytes();
        let mut flags = CallSpacingFlags::default();

        // Scan for `\w \s+ (` (MAIN + FUNC_PTR)
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'(' {
                // Look back past whitespace for a word char
                if i >= 2 {
                    let mut ws_end = i;
                    while ws_end > 0 && bytes[ws_end - 1].is_ascii_whitespace() {
                        ws_end -= 1;
                    }
                    if ws_end < i && ws_end > 0 && string_utils::is_word_char(bytes[ws_end - 1]) {
                        flags.main = true;
                        // Check for FUNC_PTR pattern: `\w \s+ ( (\w+::)* \* \w+ ) (`
                        // The '(' we're at could be the end of a cast like `(::*foo)(`
                        // We check if the previous non-space token looks like `(*name)` ending here
                        // Simple heuristic: scan back for `)` after `(`
                        let after_paren_start = ws_end; // after the space-separated word
                        // After main match, check if s[after_paren_start..i+?] contains `(::*…)(`
                        if !flags.func_ptr {
                            // Slice from word-char position to end, check for `(*…)(`
                            let sub = &s[after_paren_start.saturating_sub(1)..i];
                            if sub.contains("::*") || sub.contains("(*") {
                                flags.func_ptr = true;
                            }
                        }
                    }
                }
            }
            i += 1;
        }

        // ASM pattern: asm…volatile…(
        if s.contains("asm") && s.contains("volatile") && s.contains('(') {
            flags.asm = true;
        }

        // DEFINE pattern: uses `#`, `typedef`, or `using … =`
        if s.contains('#') || s.contains("typedef") || (s.contains("using") && s.contains('=')) {
            flags.define = true;
        }

        // FUNC_PTR pattern: `\w\s+(\w+::)*\*\w+\)(`  — pointer-to-member function call.
        // The key discriminator over MAIN is `::*` or `(*`.
        if flags.main && (s.contains("::*") || s.contains("(*")) {
            flags.func_ptr = true;
        }

        // CASE pattern: `\bcase\s+(`
        if let Some(pos) = s.find("case")
            && string_utils::is_word_match(s, pos, pos + 4)
        {
            let after = s[pos + 4..].trim_start();
            if after.starts_with('(') {
                flags.case = true;
            }
        }

        flags
    }
}

/// Manual replacement for EXTRA_SPACE_BEFORE_CLOSE_PAREN_RE (`[^)]\s+\)\s*[^{\s]`).
/// Returns true if the string matches: a non-`)` char, then whitespace, then `)`,
/// then (skipping optional whitespace) a char that is neither `{` nor whitespace.
fn has_extra_space_before_close_paren(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 3 {
        return false;
    }
    // Search for ')' at position >= 2 (need at least one non-) char + one space before it)
    let mut i = 2usize;
    while i < bytes.len() {
        if bytes[i] != b')' {
            i += 1;
            continue;
        }
        let paren_pos = i;
        // Must be preceded by at least one whitespace byte
        if !bytes[paren_pos - 1].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        // Walk back past all whitespace to find what comes before
        let mut back = paren_pos - 1;
        while back > 0 && bytes[back - 1].is_ascii_whitespace() {
            back -= 1;
        }
        // The char immediately before the whitespace run must not be ')':
        // - back == 0 means all chars before paren_pos are whitespace (bytes[0] is whitespace = not ')')
        // - otherwise check bytes[back - 1]
        if back > 0 && bytes[back - 1] == b')' {
            i += 1;
            continue;
        }
        // After ')': skip whitespace and check for a char that is not '{' and not whitespace
        let mut after = paren_pos + 1;
        while after < bytes.len() && bytes[after].is_ascii_whitespace() {
            after += 1;
        }
        if after < bytes.len() && bytes[after] != b'{' {
            return true;
        }
        i += 1;
    }
    false
}
static INITLIST_CONTINUATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^ {6}\w"#).unwrap());
static HEADER_BLANK_LINE_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r#"^ {4}\w[^\(]*\)\s*(const\s*)?(\{\s*$|:)"#, // 0: FUNCTION
        r#"^ {4}:"#,                                  // 1: INITLIST
    ])
    .unwrap()
});

static MULTI_COMMAND_INITLIST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^[^{};]*\[[^\[\]]*\][^{}]*\{[^{}\n\r]*\}"#).unwrap());
static BRACED_INIT_TRAILING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^[\s}]*[{.;,)<>\]:]"#).unwrap());
static FIXED_WIDTH_BRACED_INT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:int8_t|int16_t|int32_t|int64_t|uint8_t|uint16_t|uint32_t|uint64_t)\s*\{"#)
        .unwrap()
});
static CLASS_OR_STRUCT_AC: LazyLock<AhoCorasick> =
    LazyLock::new(|| AhoCorasick::new(["class", "struct"]).unwrap());
static SKIP_LINE_LENGTH_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r#"^\s*#(ifndef|endif)\b"#,
        r#"^\s*//.*https?://\S*$"#,
        r#"^\s*//\s*[^\s]*$"#,
        r#"^// \$Id:.*#[0-9]+ \$$"#,
        r#"^\s*/// [@\\](copydoc|copydetails|copybrief) .*$"#,
    ])
    .unwrap()
});
static QUALIFIED_BRACE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\)\s*(?:const|override|final|noexcept(?:\s*\([^)]*\))?)\{"#).unwrap()
});

fn should_skip_line_length(raw_line: &str) -> bool {
    raw_line.starts_with("#include") || SKIP_LINE_LENGTH_SET.is_match(raw_line)
}

fn contains_class_or_struct_word(line: &str) -> bool {
    CLASS_OR_STRUCT_AC
        .find_iter(line)
        .any(|mat| string_utils::is_word_match(line, mat.start(), mat.end()))
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_comment_spacing(linter: &mut FileLinter, clean_lines: &CleansedLines<'_>, linenum: usize) {
    let line = &clean_lines.lines_without_raw_strings[linenum];
    let Some(comment_pos) = line.find("//") else {
        return;
    };

    let prefix = &line[..comment_pos];
    if prefix.contains('"') && crate::cleanse::is_cpp_string(prefix) {
        return;
    }

    let next_line_start = clean_lines
        .lines_without_raw_strings
        .get(linenum + 1)
        .map(|next| next.len() - next.trim_start().len())
        .unwrap_or(0);

    let allows_single_space_after_scope =
        has_brace_before_comment(line, comment_pos) && next_line_start == comment_pos;
    if !allows_single_space_after_scope
        && ((comment_pos >= 1 && !line.as_bytes()[comment_pos - 1].is_ascii_whitespace())
            || (comment_pos >= 2 && !line.as_bytes()[comment_pos - 2].is_ascii_whitespace()))
    {
        linter.error(
            linenum,
            Category::WhitespaceComments,
            2,
            crate::messages::LintMessage::AtLeastTwoSpacesBetweenCodeAndComments,
        );
    }

    let comment = &line[comment_pos..];
    if let Some(captures) = TODO_COMMENT_RE.captures(comment) {
        let leading_spaces = captures.get(1).map(|m| m.as_str().len()).unwrap_or(0);
        if leading_spaces > 1 {
            linter.error(
                linenum,
                Category::WhitespaceTodo,
                2,
                crate::messages::LintMessage::TooManySpacesBeforeTodo,
            );
        }

        if captures.get(2).is_none() {
            linter.error(
                linenum,
                Category::ReadabilityTodo,
                2,
                crate::messages::LintMessage::MissingUsernameInTodo,
            );
        }

        let suffix = captures.get(3).map(|m| m.as_str()).unwrap_or("");
        if captures.get(3).is_none() || (!suffix.is_empty() && suffix != " ") {
            linter.error(
                linenum,
                Category::WhitespaceTodo,
                2,
                crate::messages::LintMessage::TodoShouldBeFollowedBySpace,
            );
        }
    }

    let comment_matches = COMMENT_SPACING_SET.matches(comment);
    if comment_matches.matched(0) && !comment_matches.matched(1) {
        linter.error(
            linenum,
            Category::WhitespaceComments,
            4,
            crate::messages::LintMessage::ShouldHaveSpaceBetweenSlashesAndComment,
        );
    }
}

/// Manual replacement for BRACE_INLINE_COMMENT_RE (`^.*\{\s*//`).
/// Checks whether there is a `{` followed only by whitespace before the comment at `comment_pos`.
fn has_brace_before_comment(line: &str, comment_pos: usize) -> bool {
    let prefix = &line.as_bytes()[..comment_pos];
    // Walk backwards from comment_pos, skipping whitespace, looking for '{'
    let mut i = prefix.len();
    while i > 0 && prefix[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    i > 0 && prefix[i - 1] == b'{'
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
    facts: &FileFacts<'_>,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
    keywords: &MatchedKeywords,
) {
    let mut masked_line_owned: Option<String> = None;
    if keywords.has_operator()
        && elided_line.contains('(')
        && let Some((prefix, operator, suffix)) = find_operator_method(elided_line)
    {
        let mut replaced = String::with_capacity(prefix.len() + operator.len() + suffix.len());
        replaced.push_str(prefix);
        replaced.extend(std::iter::repeat_n('_', operator.len()));
        replaced.push_str(suffix);
        masked_line_owned = Some(replaced);
    }

    let raw_trimmed = clean_lines.raw_lines[linenum].trim();
    if raw_trimmed.starts_with("/*! ")
        && raw_trimmed.contains("*/")
        && (raw_trimmed.contains("http://") || raw_trimmed.contains("https://"))
        && facts.namespace_top_level_depth(linenum).is_some()
    {
        linter.error(
            linenum,
            Category::WhitespaceOperators,
            4,
            crate::messages::LintMessage::ExtraSpaceForOperator(
                crate::messages::OperatorSymbol::BangSpaced,
            ),
        );
        return;
    }

    let line_to_check = masked_line_owned.as_deref().unwrap_or(elided_line);
    let check_assignment = !keywords
        .intersects(MatchedKeywords::IF | MatchedKeywords::WHILE | MatchedKeywords::FOR)
        && !line_to_check.contains("operator=");
    let analysis = OperatorSpacingAnalysis::scan(line_to_check, check_assignment);

    if analysis.missing_assignment_space {
        linter.error(
            linenum,
            Category::WhitespaceOperators,
            4,
            crate::messages::LintMessage::MissingSpacesAround(crate::messages::OperatorSymbol::Eq),
        );
    }

    if let Some(op) = analysis.missing_comparison_space {
        if let Some(op) = crate::messages::OperatorSymbol::from_str_opt(op) {
            linter.error(
                linenum,
                Category::WhitespaceOperators,
                3,
                crate::messages::LintMessage::MissingSpacesAround(op),
            );
        }
    } else if !line_to_check.starts_with('#') || !line_to_check.contains("include") {
        if let Some(end_pos) = analysis.less_pos
            && crate::line_utils::close_expression(clean_lines, linenum, end_pos).is_none()
        {
            linter.error(
                linenum,
                Category::WhitespaceOperators,
                3,
                crate::messages::LintMessage::MissingSpacesAround(
                    crate::messages::OperatorSymbol::Lt,
                ),
            );
        }

        if let Some(start_pos) = analysis.greater_pos
            && crate::line_utils::reverse_close_expression(clean_lines, linenum, start_pos)
                .is_none()
        {
            linter.error(
                linenum,
                Category::WhitespaceOperators,
                3,
                crate::messages::LintMessage::MissingSpacesAround(
                    crate::messages::OperatorSymbol::Gt,
                ),
            );
        }
    }

    if let Some(pos) = analysis.lshift_pos
        && let Some((left, right)) = lshift_tokens_at(line_to_check, pos)
    {
        let left_is_digit = left.len() == 1 && left.as_bytes()[0].is_ascii_digit();
        let right_is_digit = right.len() == 1 && right.as_bytes()[0].is_ascii_digit();
        let operator_definition = left == "operator" && (right == ";" || right == "(");
        if !(operator_definition || left_is_digit && right_is_digit) {
            linter.error(
                linenum,
                Category::WhitespaceOperators,
                3,
                crate::messages::LintMessage::MissingSpacesAround(
                    crate::messages::OperatorSymbol::LShift,
                ),
            );
        }
    }

    if analysis.rshift_spacing {
        linter.error(
            linenum,
            Category::WhitespaceOperators,
            3,
            crate::messages::LintMessage::MissingSpacesAround(
                crate::messages::OperatorSymbol::RShift,
            ),
        );
    }

    if let Some(op) = analysis
        .extra_unary_space
        .and_then(crate::messages::OperatorSymbol::from_str_opt)
    {
        linter.error(
            linenum,
            Category::WhitespaceOperators,
            4,
            crate::messages::LintMessage::ExtraSpaceForOperator(op),
        );
    }
}

fn find_operator_method(s: &str) -> Option<(&str, &str, &str)> {
    // Regex: ^(.*\boperator\b)(\S+)(\s*\(.*)$
    // Search for "operator" from right to left to mimic greedy (.*)
    let mut offset = s.len();
    while let Some(pos) = s[..offset].rfind("operator") {
        let end_pos = pos + 8;
        // Check word boundaries
        let prev_ok = pos == 0 || !s[pos - 1..pos].chars().next()?.is_ascii_alphanumeric();
        let next_ok = end_pos == s.len()
            || !s[end_pos..end_pos + 1]
                .chars()
                .next()?
                .is_ascii_alphanumeric();

        if prev_ok && next_ok {
            let prefix = &s[..end_pos];
            let rest = &s[end_pos..];

            // Find end of operator (\S+)
            let op_end = rest.find(char::is_whitespace).or_else(|| rest.find('('))?;
            if op_end == 0 {
                offset = pos;
                continue;
            }
            let operator = &rest[..op_end];
            let suffix = &rest[op_end..];

            // Suffix must contain '('
            if suffix.contains('(') {
                return Some((prefix, operator, suffix));
            }
        }
        offset = pos;
    }
    None
}

fn ends_with_case_insensitive(s: &str, suffix: &str) -> bool {
    let s_bytes = s.as_bytes();
    let suf_bytes = suffix.as_bytes();
    if s_bytes.len() < suf_bytes.len() {
        return false;
    }
    s_bytes[s_bytes.len() - suf_bytes.len()..]
        .iter()
        .zip(suf_bytes)
        .all(|(a, b)| a.to_ascii_uppercase() == *b)
}

#[derive(Default)]
struct OperatorSpacingAnalysis {
    missing_assignment_space: bool,
    missing_comparison_space: Option<&'static str>,
    less_pos: Option<usize>,
    greater_pos: Option<usize>,
    lshift_pos: Option<usize>,
    rshift_spacing: bool,
    extra_unary_space: Option<&'static str>,
}

impl OperatorSpacingAnalysis {
    fn scan(s: &str, check_assignment: bool) -> Self {
        let bytes = s.as_bytes();
        let mut analysis = Self::default();
        let mut i = 0usize;

        while i < bytes.len() {
            match bytes[i] {
                b'=' => {
                    if check_assignment
                        && !analysis.missing_assignment_space
                        && has_missing_assignment_space_at(s, bytes, i)
                    {
                        analysis.missing_assignment_space = true;
                    }
                    if analysis.missing_comparison_space.is_none()
                        && bytes.get(i + 1) == Some(&b'=')
                        && has_missing_comparison_space_at(bytes, i)
                    {
                        analysis.missing_comparison_space = Some("==");
                        i += 1;
                    }
                }
                b'!' => {
                    if analysis.missing_comparison_space.is_none()
                        && bytes.get(i + 1) == Some(&b'=')
                        && has_missing_comparison_space_at(bytes, i)
                    {
                        analysis.missing_comparison_space = Some("!=");
                        i += 1;
                    } else if analysis.extra_unary_space.is_none()
                        && bytes
                            .get(i + 1)
                            .is_some_and(|next| next.is_ascii_whitespace())
                    {
                        analysis.extra_unary_space = Some("!");
                    }
                }
                b'<' => {
                    if analysis.missing_comparison_space.is_none()
                        && bytes.get(i + 1) == Some(&b'=')
                        && has_missing_comparison_space_at(bytes, i)
                    {
                        analysis.missing_comparison_space = Some("<=");
                        i += 1;
                    } else if analysis.lshift_pos.is_none() && bytes.get(i + 1) == Some(&b'<') {
                        if let Some(&next_b) = bytes.get(i + 2)
                            && !(next_b.is_ascii_whitespace()
                                || matches!(next_b, b',' | b'=' | b'<'))
                        {
                            analysis.lshift_pos = Some(i);
                        }
                        i += 1;
                    }
                }
                b'>' => {
                    if analysis.missing_comparison_space.is_none()
                        && bytes.get(i + 1) == Some(&b'=')
                        && has_missing_comparison_space_at(bytes, i)
                    {
                        analysis.missing_comparison_space = Some(">=");
                        i += 1;
                    } else if !analysis.rshift_spacing && bytes.get(i + 1) == Some(&b'>') {
                        if let Some(&next) = bytes.get(i + 2)
                            && (next.is_ascii_alphabetic() || next == b'_')
                        {
                            analysis.rshift_spacing = true;
                        }
                        i += 1;
                    }
                }
                b'|' if analysis.missing_comparison_space.is_none()
                    && bytes.get(i + 1) == Some(&b'|')
                    && has_missing_comparison_space_at(bytes, i) =>
                {
                    analysis.missing_comparison_space = Some("||");
                    i += 1;
                }
                b'~' if analysis.extra_unary_space.is_none()
                    && bytes
                        .get(i + 1)
                        .is_some_and(|next| next.is_ascii_whitespace()) =>
                {
                    analysis.extra_unary_space = Some("~");
                }
                b'+' | b'-'
                    if analysis.extra_unary_space.is_none()
                        && bytes.get(i + 1) == Some(&bytes[i])
                        && i > 0
                        && bytes[i - 1].is_ascii_whitespace()
                        && bytes
                            .get(i + 2)
                            .is_some_and(|after| after.is_ascii_whitespace() || *after == b';') =>
                {
                    analysis.extra_unary_space = Some(if bytes[i] == b'-' { "--" } else { "++" });
                    i += 1;
                }
                _ => {}
            }
            i += 1;
        }

        if bytes.len() >= 3 {
            let mut i = bytes.len() - 2;
            loop {
                let prev = bytes[i - 1];
                let next = bytes[i + 1];

                if analysis.less_pos.is_none()
                    && bytes[i] == b'<'
                    && !prev.is_ascii_whitespace()
                    && prev != b'<'
                    && !next.is_ascii_whitespace()
                    && !matches!(next, b'=' | b'<' | b',')
                {
                    analysis.less_pos = Some(i);
                }

                if analysis.greater_pos.is_none()
                    && bytes[i] == b'>'
                    && !prev.is_ascii_whitespace()
                    && prev != b'-'
                    && prev != b'>'
                    && !next.is_ascii_whitespace()
                    && !matches!(next, b'=' | b'>' | b',')
                {
                    analysis.greater_pos = Some(i);
                }

                if analysis.less_pos.is_some() && analysis.greater_pos.is_some() {
                    break;
                }
                if i == 1 {
                    break;
                }
                i -= 1;
            }
        }

        analysis
    }
}

fn has_missing_assignment_space_at(s: &str, bytes: &[u8], i: usize) -> bool {
    if i > 0
        && matches!(
            bytes[i - 1],
            b'>' | b'<' | b'=' | b'!' | b'&' | b'^' | b'|' | b'+' | b'-' | b'*' | b'/' | b'%'
        )
    {
        return false;
    }
    if bytes.get(i + 1) == Some(&b'=') {
        return false;
    }

    if i > 0 {
        let prev = bytes[i - 1];
        if (prev.is_ascii_alphanumeric() || prev == b'.') && (i < 8 || &s[i - 8..i] != "operator") {
            return true;
        }
    }

    bytes
        .get(i + 1)
        .is_some_and(|next| next.is_ascii_alphanumeric() || *next == b'.')
}

fn has_missing_comparison_space_at(bytes: &[u8], i: usize) -> bool {
    if i == 0 || i + 2 >= bytes.len() {
        return false;
    }

    let prev = bytes[i - 1];
    let next = bytes[i + 2];

    let prev_is_op_char =
        matches!(prev, b'<' | b'>' | b'=' | b'!' | b'|') || prev.is_ascii_whitespace();
    let next_is_op_char = matches!(next, b'<' | b'>' | b'=' | b'!' | b'|' | b',' | b';' | b')')
        || next.is_ascii_whitespace();

    !prev_is_op_char && !next_is_op_char
}

fn lshift_tokens_at(s: &str, i: usize) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();
    let next_idx = i + 2;
    if next_idx >= bytes.len() {
        return None;
    }
    let next_b = bytes[next_idx];
    if next_b.is_ascii_whitespace() || next_b == b',' || next_b == b'=' || next_b == b'<' {
        return None;
    }

    let mut prefix_end = i;
    let prefix_str = &s[..i];
    if ends_with_case_insensitive(prefix_str, "ULL") {
        prefix_end = prefix_end.saturating_sub(3);
    } else if ends_with_case_insensitive(prefix_str, "LL")
        || ends_with_case_insensitive(prefix_str, "UL")
    {
        prefix_end = prefix_end.saturating_sub(2);
    } else if ends_with_case_insensitive(prefix_str, "L") {
        prefix_end = prefix_end.saturating_sub(1);
    }

    if prefix_end == 0 {
        return None;
    }
    let prefix = &s[..prefix_end];
    let left = if prefix.ends_with("operator") {
        "operator"
    } else {
        let prev_char = prefix.chars().last()?;
        if prev_char.is_ascii_whitespace() || prev_char == '(' || prev_char == '<' {
            return None;
        }
        &prefix[prefix.len() - prev_char.len_utf8()..]
    };

    let next_char = s[next_idx..].chars().next()?;
    let right = &s[next_idx..next_idx + next_char.len_utf8()];

    Some((left, right))
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_parenthesis_spacing(
    linter: &mut FileLinter,
    elided_line: &str,
    raw_line: &str,
    linenum: usize,
    keywords: &MatchedKeywords,
) {
    if keywords.intersects(
        MatchedKeywords::IF
            | MatchedKeywords::FOR
            | MatchedKeywords::WHILE
            | MatchedKeywords::SWITCH,
    ) {
        if let Some(_captures) = CONTROL_PARENS_MISSING_SPACE_RE.captures(elided_line) {
            linter.error(
                linenum,
                Category::WhitespaceParens,
                5,
                crate::messages::LintMessage::MissingSpaceBeforeOpenParen,
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
                    Category::WhitespaceParens,
                    5,
                    crate::messages::LintMessage::MismatchingSpacesInsideParen,
                );
            }
            if left_spaces != 0 && left_spaces != 1 {
                linter.error(
                    linenum,
                    Category::WhitespaceParens,
                    5,
                    crate::messages::LintMessage::ShouldHaveZeroOrOneSpacesInsideParensIn(
                        keyword.into(),
                    ),
                );
            }
        }
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_spacing_for_function_call(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    raw_line: &str,
    linenum: usize,
    keywords: &MatchedKeywords,
) {
    if !elided_line.contains('(') && !elided_line.contains(')') {
        return;
    }
    if keywords.intersects(MatchedKeywords::IF | MatchedKeywords::FOR | MatchedKeywords::SWITCH) {
        if let Some(captures) = IF_FOR_SWITCH_CALL_RE.captures(elided_line) {
            check_spacing_for_function_call_base(
                linter,
                elided_line,
                captures.get(2).map(|m| m.as_str()).unwrap_or(""),
                raw_line,
                linenum,
                keywords,
            );
            return;
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
    let trimmed_raw_line = raw_line.trim_end();
    if trimmed_raw_line != raw_line && trimmed_raw_line.ends_with('(') {
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

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_spacing_for_function_call_base(
    linter: &mut FileLinter,
    line: &str,
    fncall: &str,
    raw_line: &str,
    linenum: usize,
    keywords: &MatchedKeywords,
) {
    if keywords.has_any_control_struct()
        && CONTROL_STRUCT_AC
            .find_iter(fncall)
            .any(|mat| string_utils::is_word_match(fncall, mat.start(), mat.end()))
    {
        return;
    }

    if has_ref_call(fncall) {
        return;
    }

    if has_extra_space_after_function_call_paren(fncall) {
        linter.error(
            linenum,
            Category::WhitespaceParens,
            4,
            crate::messages::LintMessage::ExtraSpaceAfterParenInFuncCall,
        );
    } else if has_extra_space_after_leading_nested_open_paren(fncall)
        || has_extra_space_after_open_paren(fncall)
    {
        linter.error(
            linenum,
            Category::WhitespaceParens,
            2,
            crate::messages::LintMessage::ExtraSpaceAfterParen,
        );
    }

    let spacing = CallSpacingFlags::scan(fncall);
    if spacing.asm {
        // Inline asm invocations such as `asm volatile (...)` are intentionally
        // exempt from function-call spacing checks.
        return;
    }
    if spacing.main && !spacing.func_ptr {
        let mut exception_mask = MatchedKeywords::empty();
        if spacing.define {
            exception_mask |= MatchedKeywords::TYPEDEF | MatchedKeywords::USING;
        }
        if spacing.case {
            exception_mask |= MatchedKeywords::CASE;
        }

        if !keywords.intersects(exception_mask) {
            let confidence = if keywords.has_operator() && has_operator_name(line) {
                0
            } else {
                4
            };
            linter.error(
                linenum,
                Category::WhitespaceParens,
                confidence,
                crate::messages::LintMessage::ExtraSpaceBeforeParenInFuncCall,
            );
        }
    }

    if !has_extra_space_before_close_paren(fncall) {
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
            Category::WhitespaceParens,
            2,
            crate::messages::LintMessage::ClosingParenShouldBeMovedToPreviousLine,
        );
    } else {
        linter.error(
            linenum,
            Category::WhitespaceParens,
            2,
            crate::messages::LintMessage::ExtraSpaceBeforeCloseParen,
        );
    }
}

fn is_braced_initialization(
    clean_lines: &CleansedLines<'_>,
    _elided_line: &str,
    leading_text: &str,
    brace_pos: usize,
    linenum: usize,
) -> bool {
    let start_pos = brace_pos;
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
            owned.push_str(clean_lines.elided[offset]);
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
fn check_blank_line_rules(
    linter: &mut FileLinter,
    facts: &FileFacts<'_>,
    clean_lines: &CleansedLines<'_>,
    linenum: usize,
) {
    let line = &clean_lines.lines_without_raw_strings[linenum];
    if !crate::line_utils::is_blank_line(line) {
        return;
    }
    if clean_lines.in_raw_string.get(linenum).copied().unwrap_or(false) {
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
                if !INITLIST_CONTINUATION_RE.is_match(clean_lines.elided[position]) {
                    break;
                }
                search_position = position.checked_sub(1);
            }
            search_position
                .map(|position| clean_lines.elided[position].starts_with("    :"))
                .unwrap_or(false)
        } else {
            HEADER_BLANK_LINE_SET.is_match(prev_line)
        };

        if !exception {
            linter.error(
                linenum,
                Category::WhitespaceBlankLine,
                2,
                crate::messages::LintMessage::BlankLineAtStartOfBlock,
            );
        }
    }

    if let Some((_, _specifier, _)) = parse_access_specifier(prev_line) {
        linter.error(
            linenum,
            Category::WhitespaceBlankLine,
            3,
            crate::messages::LintMessage::NoBlankLineAfterSection,
        );
    }

    if let Some(next_line) = clean_lines.lines_without_raw_strings.get(linenum + 1)
        && !next_line.is_empty()
        && next_line.trim_start().starts_with('}')
        && !next_line.contains("} else ")
    {
        let closes_extern_block = facts
            .matching_block_start(linenum + 1)
            .is_some_and(|start| {
                let start_line = clean_lines.raw_lines[start].trim();
                start_line.starts_with("extern ") && start_line.ends_with('{')
            });
        let closes_namespace_block = facts
            .matching_block_start(linenum + 1)
            .is_some_and(|start| {
                crate::line_utils::namespace_decl_start_line(&clean_lines.elided, start).is_some()
            });
        if closes_extern_block || closes_namespace_block {
            return;
        }
        linter.error(
            linenum,
            Category::WhitespaceBlankLine,
            3,
            crate::messages::LintMessage::BlankLineAtEndOfBlock,
        );
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_section_spacing(
    linter: &mut FileLinter,
    facts: &FileFacts<'_>,
    clean_lines: &CleansedLines<'_>,
    linenum: usize,
    keywords: &MatchedKeywords,
) {
    if !keywords.has_access() {
        return;
    }
    let line = &clean_lines.lines_without_raw_strings[linenum];
    let Some((_, _specifier, _)) = parse_access_specifier(line) else {
        return;
    };

    let Some(class_range) = facts.enclosing_class_range(linenum) else {
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
            Category::WhitespaceBlankLine,
            3,
            crate::messages::LintMessage::NoBlankLineAfterSection, // The variant fits the spirit
        );
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_access_specifier_indentation(
    linter: &mut FileLinter,
    facts: &FileFacts<'_>,
    clean_lines: &CleansedLines<'_>,
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
    let Some((prefix_len, specifier, has_slots)) = parse_access_specifier(line) else {
        return;
    };
    let Some(class_range) = facts.enclosing_class_range(linenum) else {
        return;
    };

    let class_indent = crate::line_utils::get_indent_level(
        clean_lines.lines_without_raw_strings[class_range.start],
    );
    if prefix_len == class_indent + 1 && line[..prefix_len].chars().all(|ch| ch == ' ') {
        return;
    }
    if class_indent == 0 && &line[..prefix_len] == "\t" {
        return;
    }

    let access = specifier;
    let slots = if has_slots { " slots" } else { "" };
    if prefix_len != class_indent + 1 {
        let message = match (
            facts.enclosing_class_is_struct(linenum).unwrap_or(false),
            facts.nearest_class_name(linenum),
        ) {
            (true, Some(name)) if !name.is_empty() => {
                format!(
                    "{}{}: should be indented +1 space inside struct {}",
                    access, slots, name
                )
            }
            (false, Some(name)) if !name.is_empty() => {
                format!(
                    "{}{}: should be indented +1 space inside class {}",
                    access, slots, name
                )
            }
            (true, _) => format!(
                "{}{}: should be indented +1 space inside struct",
                access, slots
            ),
            (false, _) => {
                format!(
                    "{}{}: should be indented +1 space inside class",
                    access, slots
                )
            }
        };
        linter.error(
            linenum,
            Category::WhitespaceIndent,
            3,
            crate::messages::LintMessage::ShouldBeIndented(message.into()),
        );
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_class_closing_brace_alignment(
    linter: &mut FileLinter,
    facts: &FileFacts<'_>,
    clean_lines: &CleansedLines<'_>,
    linenum: usize,
) {
    let Some(class_range) = facts.enclosing_class_range(linenum) else {
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
        clean_lines.lines_without_raw_strings[class_range.start],
    );
    let closing_indent = crate::line_utils::get_indent_level(line);
    if closing_indent == class_indent {
        return;
    }

    let parent = match (
        facts.enclosing_class_is_struct(linenum).unwrap_or(false),
        facts.nearest_class_name(linenum),
    ) {
        (true, Some(name)) if !name.is_empty() => format!("struct {}", name),
        (false, Some(name)) if !name.is_empty() => format!("class {}", name),
        (true, _) => "struct".to_string(),
        (false, _) => "class".to_string(),
    };
    linter.error(
        linenum,
        Category::WhitespaceIndent,
        3,
        crate::messages::LintMessage::ClosingBraceAlignment(parent.into()),
    );
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_tabs_and_line_length(
    linter: &mut FileLinter,
    line_without_raw_strings: &str,
    linenum: usize,
    line_features: LineFeatures,
) {
    let has_tab = line_features.contains(LineFeatures::RAW_HAS_TAB);
    if has_tab {
        linter.error(
            linenum,
            Category::WhitespaceTab,
            1,
            crate::messages::LintMessage::TabFound,
        );
    }

    let line_length_limit = linter.options().line_length;
    if !has_tab && line_without_raw_strings.len() <= line_length_limit {
        return;
    }

    let width = if !has_tab && line_without_raw_strings.is_ascii() {
        line_without_raw_strings.len()
    } else {
        UnicodeWidthStr::width(line_without_raw_strings)
    };
    if width > line_length_limit && !should_skip_line_length(line_without_raw_strings) {
        linter.error(
            linenum,
            Category::WhitespaceLineLength,
            2,
            crate::messages::LintMessage::LineLength(line_length_limit),
        );
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn check_indentation(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    raw_line: &str,
    line: &str,
    linenum: usize,
    keywords: &MatchedKeywords,
    line_features: LineFeatures,
) {
    if line_features.contains(LineFeatures::RAW_TRAILING_WHITESPACE) {
        linter.error(
            linenum,
            Category::WhitespaceEndOfLine,
            4,
            crate::messages::LintMessage::TrailingWhitespace,
        );
    }

    if !line_features.contains(LineFeatures::RAW_STARTS_WITH_WHITESPACE) {
        return;
    }

    let initial_spaces = crate::line_utils::get_indent_level(raw_line);
    if initial_spaces != 1 && initial_spaces != 3 {
        return;
    }

    let comment_only_line = clean_lines.has_comment[linenum] && line.trim().is_empty();
    let indent_line = if comment_only_line { raw_line } else { line };
    let prev_line_allows_continuation = linenum > 0
        && PREV_LINE_CONTINUATION_RE.is_match(clean_lines.lines_without_raw_strings[linenum - 1]);
    let is_scope_or_label = keywords.has_access() && SCOPE_OR_LABEL_RE.is_match(indent_line);
    let is_raw_string_line =
        clean_lines.raw_lines[linenum] != line && line.trim_start().starts_with("\"\"");

    if !prev_line_allows_continuation
        && (initial_spaces == 1 || initial_spaces == 3)
        && !is_scope_or_label
        && !is_raw_string_line
    {
        linter.error(
            linenum,
            Category::WhitespaceIndent,
            3,
            crate::messages::LintMessage::WeirdNumberOfSpacesAtLineStart,
        );
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn check(
    linter: &mut FileLinter,
    facts: &FileFacts<'_>,
    clean_lines: &CleansedLines<'_>,
    linenum: usize,
) {
    let raw_line = &clean_lines.raw_lines[linenum];
    let line_without_raw_strings = &clean_lines.lines_without_raw_strings[linenum];
    let line = &clean_lines.lines[linenum];
    let elided_line = &clean_lines.elided[linenum];
    let line_features = clean_lines.line_features[linenum];

    let has_slash = line_features.contains(LineFeatures::RAW_HAS_SLASH);
    let has_colon = line_features.contains(LineFeatures::COLON);
    let has_paren = line_features.contains(LineFeatures::PAREN);
    let has_comma = line_features.contains(LineFeatures::COMMA);
    let has_semicolon = line_features.contains(LineFeatures::SEMI);
    let has_brace = line_features.contains(LineFeatures::BRACE);
    let has_bracket = line_features.contains(LineFeatures::BRACKET);
    let has_operator = line_features.contains(LineFeatures::OP);
    let semicolon_before_block_comment =
        line_features.contains(LineFeatures::RAW_HAS_SEMICOLON_BLOCK_COMMENT);

    let keywords = clean_lines.keywords(linenum);

    if has_slash {
        check_comment_spacing(linter, clean_lines, linenum);
    }

    if line_features.contains(LineFeatures::LINE_WITHOUT_RAW_STRINGS_BLANK) {
        check_blank_line_rules(linter, facts, clean_lines, linenum);
    }

    if linenum > 0 {
        check_section_spacing(linter, facts, clean_lines, linenum, &keywords);
    }

    if has_colon || keywords.has_access() {
        check_access_specifier_indentation(linter, facts, clean_lines, linenum, &keywords);
    }

    if has_brace {
        check_class_closing_brace_alignment(linter, facts, clean_lines, linenum);
    }

    check_tabs_and_line_length(linter, line_without_raw_strings, linenum, line_features);
    check_indentation(
        linter,
        clean_lines,
        raw_line,
        line,
        linenum,
        &keywords,
        line_features,
    );

    if has_bracket && has_extra_space_before_bracket(elided_line) {
        linter.error(
            linenum,
            Category::WhitespaceBraces,
            5,
            crate::messages::LintMessage::ExtraSpaceBeforeBracket,
        );
    }

    if keywords.has_for() && has_colon && RANGE_FOR_COLON_SET.is_match(elided_line) {
        linter.error(
            linenum,
            Category::WhitespaceForcolon,
            2,
            crate::messages::LintMessage::MissingSpaceAroundRangeForColon,
        );
    }

    if has_operator {
        check_operator_spacing(linter, facts, clean_lines, elided_line, linenum, &keywords);
    }

    if has_paren {
        check_parenthesis_spacing(linter, elided_line, raw_line, linenum, &keywords);
        check_spacing_for_function_call(
            linter,
            clean_lines,
            elided_line,
            raw_line,
            linenum,
            &keywords,
        );
    }

    if has_comma || has_semicolon || semicolon_before_block_comment {
        if has_comma {
            let check_line_bytes = elided_line.as_bytes();
            let original_line_bytes = line.as_bytes();
            let mut missing_comma_space = false;

            let mut offset = 0;
            while let Some(idx) = elided_line[offset..].find(',') {
                let i = offset + idx;
                offset = i + 1;

                if i + 1 < check_line_bytes.len()
                    && !matches!(check_line_bytes[i + 1], b',' | b' ' | b'\t' | b'\n' | b'\r')
                    && i + 1 < original_line_bytes.len()
                    && original_line_bytes[i] == b','
                    && !matches!(
                        original_line_bytes[i + 1],
                        b',' | b' ' | b'\t' | b'\n' | b'\r'
                    )
                {
                    let mut is_exception = false;
                    let prefix = elided_line[..i].trim_end();
                    if keywords.has_operator() && prefix.ends_with("operator") {
                        let before_op = &prefix[..prefix.len() - 8]; // 8 is len of "operator"
                        if before_op.is_empty()
                            || !before_op.chars().last().unwrap().is_ascii_alphanumeric()
                        {
                            is_exception = true;
                        }
                    }
                    if !is_exception && keywords.has_va_opt() && prefix.ends_with('(') {
                        let before_paren = prefix[..prefix.len() - 1].trim_end();
                        if before_paren
                            .strip_suffix("__VA_OPT__")
                            .is_some_and(|before_va| {
                                (before_va.is_empty()
                                    || !before_va.chars().last().unwrap().is_ascii_alphanumeric())
                                    && elided_line[i + 1..].trim_start().starts_with(')')
                            })
                        {
                            is_exception = true;
                        }
                    }

                    if !is_exception {
                        missing_comma_space = true;
                        break;
                    }
                }
            }

            if missing_comma_space {
                linter.error(
                    linenum,
                    Category::WhitespaceComma,
                    3,
                    crate::messages::LintMessage::MissingSpaceAfterComma,
                );
            }
        }

        let mut missing_semicolon_space = false;
        let mut semicolon_count = 0;
        if has_semicolon {
            let elided_bytes = elided_line.as_bytes();
            let mut offset = 0;
            while let Some(idx) = elided_line[offset..].find(';') {
                semicolon_count += 1;
                let i = offset + idx;
                offset = i + 1;
                if i + 1 < elided_bytes.len()
                    && !matches!(
                        elided_bytes[i + 1],
                        b' ' | b'\t' | b'\n' | b'\r' | b'}' | b';' | b'\\' | b')' | b'/'
                    )
                {
                    missing_semicolon_space = true;
                }
            }
        }

        if missing_semicolon_space || semicolon_before_block_comment {
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
                Category::WhitespaceSemicolon,
                3,
                crate::messages::LintMessage::MissingSpaceBeforeSemicolon,
            );
        }

        if semicolon_count > 1 {
            let switch_case_single_line =
                (keywords.has_case() || keywords.has_default()) && elided_line.contains("break;");

            if !keywords.has_for() && !switch_case_single_line {
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

                if !MULTI_COMMAND_INITLIST_RE.is_match(line)
                    && (!prev_line.contains("for") || prev_line.contains(';'))
                {
                    linter.error(
                        linenum,
                        Category::WhitespaceNewline,
                        0,
                        crate::messages::LintMessage::MoreThanOneCommandOnTheSameLine,
                    );
                }
            }
        }
    }

    // 10. Brace and semicolon spacing.
    if has_brace
        && let Some(brace_pos) = elided_line.find('{')
        && brace_pos > 0
    {
        let prefix = &elided_line[..brace_pos];
        let last_char = prefix.chars().last();
        if let Some(c) = last_char
            && !matches!(c, ' ' | '(' | '{' | '>')
        {
            let missing_space_before_qualified_brace = QUALIFIED_BRACE_RE.is_match(elided_line);
            if (!is_braced_initialization(clean_lines, elided_line, prefix, brace_pos, linenum)
                || missing_space_before_qualified_brace)
                && !FIXED_WIDTH_BRACED_INT_RE.is_match(elided_line)
            {
                linter.error(
                    linenum,
                    Category::WhitespaceBraces,
                    5,
                    crate::messages::LintMessage::MissingSpaceBeforeOpenBrace,
                );
            }
        }
    }

    if elided_line.contains("}else") {
        linter.error(
            linenum,
            Category::WhitespaceBraces,
            5,
            crate::messages::LintMessage::MissingSpaceBeforeElse,
        );
    }

    if let Some(colon_pos) = elided_line.find(':') {
        let suffix = &elided_line[colon_pos + 1..];
        let trimmed_suffix = suffix.trim();
        if trimmed_suffix == ";" {
            linter.error(
                linenum,
                Category::WhitespaceSemicolon,
                5,
                crate::messages::LintMessage::SemicolonDefiningEmptyStatementUseBraces,
            );
        }
    } else if elided_line.trim() == ";" {
        linter.error(
            linenum,
            Category::WhitespaceSemicolon,
            5,
            crate::messages::LintMessage::LineContainsOnlySemicolonUseBraces,
        );
    } else if let Some(stripped) = elided_line.strip_suffix(';') {
        let before_semi = stripped.as_bytes();
        if before_semi.last().is_some_and(|c| c.is_ascii_whitespace())
            && !string_utils::contains_word(elided_line, "for")
        {
            linter.error(
                linenum,
                Category::WhitespaceSemicolon,
                5,
                crate::messages::LintMessage::ExtraSpaceBeforeLastSemicolonUseBraces,
            );
        }
    }
}

pub fn check_eof_newline<S: AsRef<str>>(linter: &mut FileLinter, raw_lines: &[S]) {
    if raw_lines.is_empty() {
        return;
    }

    let last_line = raw_lines[raw_lines.len() - 1].as_ref();
    if !last_line.is_empty() {
        linter.error(
            raw_lines.len() - 1,
            Category::WhitespaceEndingNewline,
            5,
            crate::messages::LintMessage::NewlineShouldBeAtEndOfFile,
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

    #[test]
    fn test_class_access() {
        assert_eq!(parse_access_specifier(" public:"), Some((1, "public", false)));
        assert_eq!(parse_access_specifier(" private:"), Some((1, "private", false)));
        assert_eq!(parse_access_specifier(" protected:"), Some((1, "protected", false)));
        assert_eq!(parse_access_specifier(" signals:"), Some((1, "signals", false)));
        assert_eq!(parse_access_specifier(" public slots:"), Some((1, "public", true)));
        assert_eq!(parse_access_specifier("protracted:"), None);
        assert_eq!(parse_access_specifier("public::"), None);
    }

    #[test]
    fn operator_spacing_analysis_matches_existing_cases() {
        let analysis = OperatorSpacingAnalysis::scan("value=1", true);
        assert!(analysis.missing_assignment_space);

        let analysis = OperatorSpacingAnalysis::scan("if (a==b)", false);
        assert_eq!(analysis.missing_comparison_space, Some("=="));
        let analysis = OperatorSpacingAnalysis::scan("if ((foo)||(bar))", false);
        assert_eq!(analysis.missing_comparison_space, Some("||"));
        let analysis = OperatorSpacingAnalysis::scan("if ((foo)||(bar)) return;", false);
        assert_eq!(analysis.missing_comparison_space, Some("||"));

        let analysis = OperatorSpacingAnalysis::scan("foo<<bar", false);
        assert_eq!(analysis.lshift_pos, Some(3));
        assert_eq!(lshift_tokens_at("foo<<bar", 3), Some(("o", "b")));

        let analysis = OperatorSpacingAnalysis::scan("foo>>bar", false);
        assert!(analysis.rshift_spacing);

        let analysis = OperatorSpacingAnalysis::scan("! value", false);
        assert_eq!(analysis.extra_unary_space, Some("!"));

        let analysis = OperatorSpacingAnalysis::scan("Foo::operator=(rhs)", true);
        assert!(!analysis.missing_assignment_space);
    }
}
