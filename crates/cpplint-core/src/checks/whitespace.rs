use crate::cleanse::{CleansedLines, MatchedKeywords};
use crate::file_linter::FileLinter;
use crate::string_utils;
use aho_corasick::AhoCorasick;
use regex::{Regex, RegexSet};
use std::borrow::Cow;
use std::sync::LazyLock;
use unicode_width::UnicodeWidthStr;

static TODO_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^//(\s*)TODO(\(.+?\))?:?(\s|$)?"#).unwrap());
static ACCESS_SPECIFIER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^(.*)\b(public|private|protected|signals)(\s+(?:slots\s*)?)?:(?:[^:]|$)"#)
        .unwrap()
});

static CONTROL_STRUCT_AC: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::new([
        "if", "elif", "for", "while", "switch", "return", "new", "delete", "catch", "sizeof",
    ])
    .unwrap()
});
static REF_MATCHERS: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r#" \([^)]+\)\([^)]*(\)|,$)"#, // 元 FUNC_REF_RE
        r#" \([^)]+\)\[[^\]]+\]"#,     // 元 ARRAY_REF_RE
    ])
    .unwrap()
});

static OPERATOR_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\boperator_*\b"#).unwrap());
static VA_OPT_COMMA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\b__VA_OPT__\s*\(,\)"#).unwrap());
static OPERATOR_COMMA_CALL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\boperator\s*,\s*\("#).unwrap());
static BRACE_INLINE_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^.*\{\s*//"#).unwrap());
static COMMENT_SPACING_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r#"^//[^ ]*\w"#,      // 0: COMMENT_WITHOUT_SPACE
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
static CALL_SPACING_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r#"\w\s+\("#,                                   // 0: MAIN
        r#"_{0,2}asm_{0,2}\s+_{0,2}volatile_{0,2}\s+\("#, // 1: ASM_VOLATILE
        r#"#\s*define|typedef|using\s+\w+\s*="#,         // 2: DEFINE/TYPEDEF/USING
        r#"\w\s+\((\w+::)*\*\w+\)\("#,                    // 3: FUNCTION_POINTER
        r#"\bcase\s+\("#,                                 // 4: CASE
    ])
    .unwrap()
});

const CALL_SPACING_MAIN: usize = 0;
const CALL_SPACING_ASM: usize = 1;
const CALL_SPACING_DEFINE: usize = 2;
const CALL_SPACING_FUNC_PTR: usize = 3;
const CALL_SPACING_CASE: usize = 4;
static EXTRA_SPACE_BEFORE_CLOSE_PAREN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[^)]\s+\)\s*[^{\s]"#).unwrap());
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
static OPEN_BRACE_NEEDS_SPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(.*[^ ({>])\{"#).unwrap());
static BRACED_INIT_TRAILING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^[\s}]*[{.;,)<>\]:]"#).unwrap());
static FIXED_WIDTH_BRACED_INT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:int8_t|int16_t|int32_t|int64_t|uint8_t|uint16_t|uint32_t|uint64_t)\s*\{"#)
        .unwrap()
});

static SPACE_BEFORE_LAST_SEMICOLON_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\s+;\s*$"#).unwrap());
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

    let comment_matches = COMMENT_SPACING_SET.matches(comment);
    if comment_matches.matched(0) && !comment_matches.matched(1) {
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
        && let Some((prefix, operator, suffix)) = find_operator_method(elided_line)
    {
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
        && (keywords.bits() & (MatchedKeywords::IF | MatchedKeywords::WHILE | MatchedKeywords::FOR))
            == 0
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
            && let Some(end_pos) = find_less_spacing(line_to_check)
        {
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
            && let Some(start_pos) = find_greater_spacing(line_to_check)
        {
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

    if let Some((left, right)) = find_lshift_spacing(line_to_check) {
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

    if has_rshift_spacing(line_to_check) {
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

fn find_less_spacing(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    for i in (1..bytes.len() - 1).rev() {
        if bytes[i] == b'<' {
            let prev = bytes[i - 1];
            if prev.is_ascii_whitespace() || prev == b'<' {
                continue;
            }
            let next = bytes[i + 1];
            if next.is_ascii_whitespace() || next == b'=' || next == b'<' || next == b',' {
                continue;
            }
            return Some(i);
        }
    }
    None
}

fn find_greater_spacing(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    for i in (1..bytes.len() - 1).rev() {
        if bytes[i] == b'>' {
            let prev = bytes[i - 1];
            if prev.is_ascii_whitespace() || prev == b'-' || prev == b'>' {
                continue;
            }
            let next = bytes[i + 1];
            if next.is_ascii_whitespace() || next == b'=' || next == b'>' || next == b',' {
                continue;
            }
            return Some(i);
        }
    }
    None
}

fn find_lshift_spacing(s: &str) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();
    if bytes.len() < 3 {
        return None;
    }
    for i in 1..bytes.len().saturating_sub(1) {
        if bytes[i] == b'<' && bytes[i + 1] == b'<' {
            let next_idx = i + 2;
            if next_idx >= bytes.len() {
                continue;
            }
            let next_b = bytes[next_idx];
            if next_b.is_ascii_whitespace() || next_b == b',' || next_b == b'=' || next_b == b'<' {
                continue;
            }

            let mut prefix_end = i;
            let upper = &s[..i].to_ascii_uppercase();
            if upper.ends_with("ULL") {
                prefix_end = prefix_end.saturating_sub(3);
            } else if upper.ends_with("LL") || upper.ends_with("UL") {
                prefix_end = prefix_end.saturating_sub(2);
            } else if upper.ends_with("L") {
                prefix_end = prefix_end.saturating_sub(1);
            }

            if prefix_end == 0 {
                continue;
            }
            let prefix = &s[..prefix_end];
            let left = if prefix.ends_with("operator") {
                "operator"
            } else {
                let prev_char = prefix.chars().last().unwrap();
                if prev_char.is_ascii_whitespace() || prev_char == '(' || prev_char == '<' {
                    continue;
                }
                &prefix[prefix.len() - prev_char.len_utf8()..]
            };

            let next_char = s[next_idx..].chars().next().unwrap();
            let right = &s[next_idx..next_idx + next_char.len_utf8()];

            return Some((left, right));
        }
    }
    None
}

fn has_rshift_spacing(s: &str) -> bool {
    let bytes = s.as_bytes();
    for i in 0..bytes.len().saturating_sub(2) {
        if bytes[i] == b'>' && bytes[i + 1] == b'>' {
            let next = bytes[i + 2];
            if next.is_ascii_alphabetic() || next == b'_' {
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

fn check_parenthesis_spacing(
    linter: &mut FileLinter,
    elided_line: &str,
    raw_line: &str,
    linenum: usize,
    keywords: &MatchedKeywords,
) {
    if (keywords.bits()
        & (MatchedKeywords::IF
            | MatchedKeywords::FOR
            | MatchedKeywords::WHILE
            | MatchedKeywords::SWITCH))
        != 0
    {
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
    if (keywords.bits()
        & (MatchedKeywords::IF | MatchedKeywords::FOR | MatchedKeywords::SWITCH))
        != 0
    {
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
    if keywords.has_any_control_struct()
        && CONTROL_STRUCT_AC
            .find_iter(fncall)
            .any(|mat| string_utils::is_word_match(fncall, mat.start(), mat.end()))
    {
        return;
    }

    if REF_MATCHERS.is_match(fncall) {
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

    let spacing_matches = CALL_SPACING_SET.matches(fncall);
    if spacing_matches.matched(CALL_SPACING_MAIN) && !spacing_matches.matched(CALL_SPACING_FUNC_PTR)
    {
        let mut exception_mask = 0u32;
        if spacing_matches.matched(CALL_SPACING_ASM) {
            exception_mask |= MatchedKeywords::VA_OPT;
        }
        if spacing_matches.matched(CALL_SPACING_DEFINE) {
            exception_mask |= MatchedKeywords::TYPEDEF | MatchedKeywords::USING;
        }
        if spacing_matches.matched(CALL_SPACING_CASE) {
            exception_mask |= MatchedKeywords::CASE;
        }

        if (keywords.bits() & exception_mask) == 0 {
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
            HEADER_BLANK_LINE_SET.is_match(prev_line)
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

pub fn check(linter: &mut FileLinter, clean_lines: &CleansedLines, linenum: usize) {
    let raw_line = &clean_lines.raw_lines[linenum];
    let line_without_raw_strings = &clean_lines.lines_without_raw_strings[linenum];
    let line = &clean_lines.lines[linenum];
    let elided_line = &clean_lines.elided[linenum];

    let has_slash = raw_line.contains('/');
    let mut has_colon = false;
    let mut has_paren = false;
    let mut has_comma = false;
    let mut has_semicolon = false;
    let mut has_brace = false;

    for &b in elided_line.as_bytes() {
        match b {
            b':' => has_colon = true,
            b'(' | b')' => has_paren = true,
            b',' => has_comma = true,
            b';' => has_semicolon = true,
            b'{' => has_brace = true,
            _ => {}
        }
    }

    let keywords = &clean_lines.keywords[linenum];

    if has_slash {
        check_comment_spacing(linter, clean_lines, linenum);
    }

    check_blank_line_rules(linter, clean_lines, linenum);

    if linenum > 0 {
        check_section_spacing(linter, clean_lines, linenum, &keywords);
    }

    if has_colon || keywords.has_access() {
        check_access_specifier_indentation(linter, clean_lines, linenum, &keywords);
    }

    if elided_line.contains('}') {
        check_class_closing_brace_alignment(linter, clean_lines, linenum);
    }

    check_tabs_and_line_length(linter, raw_line, line_without_raw_strings, linenum);
    check_indentation(linter, clean_lines, raw_line, line, linenum);

    if elided_line.contains('[') && has_extra_space_before_bracket(elided_line) {
        linter.error(
            linenum,
            r#"whitespace/braces"#,
            5,
            r#"Extra space before ["#,
        );
    }

    if keywords.has_for()
        && has_colon
        && RANGE_FOR_COLON_SET.is_match(elided_line)
    {
        linter.error(
            linenum,
            r#"whitespace/forcolon"#,
            2,
            r#"Missing space around colon in range-based for loop"#,
        );
    }

    check_operator_spacing(linter, clean_lines, elided_line, linenum, &keywords);

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

    if has_comma || has_semicolon || raw_line.contains(";/*") {
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

        if has_comma {
            let check_line_bytes = comma_check_line.as_bytes();
            let original_line_bytes = line.as_bytes();
            let mut missing_comma_space = false;
            for i in 0..check_line_bytes.len().saturating_sub(1) {
                if check_line_bytes[i] == b','
                    && !matches!(check_line_bytes[i + 1], b',' | b' ' | b'\t' | b'\n' | b'\r')
                {
                    if i < original_line_bytes.len().saturating_sub(1)
                        && original_line_bytes[i] == b','
                        && !matches!(
                            original_line_bytes[i + 1],
                            b',' | b' ' | b'\t' | b'\n' | b'\r'
                        )
                    {
                        missing_comma_space = true;
                        break;
                    }
                }
            }

            if missing_comma_space {
                linter.error(
                    linenum,
                    r#"whitespace/comma"#,
                    3,
                    r#"Missing space after ,"#,
                );
            }
        }

        if has_semicolon || raw_line.contains(";/*") {
            let elided_bytes = elided_line.as_bytes();
            let mut missing_semicolon_space = false;
            for i in 0..elided_bytes.len().saturating_sub(1) {
                if elided_bytes[i] == b';'
                    && !matches!(
                        elided_bytes[i + 1],
                        b' ' | b'\t' | b'\n' | b'\r' | b'}' | b';' | b'\\' | b')' | b'/'
                    )
                {
                    missing_semicolon_space = true;
                    break;
                }
            }

            let semicolon_before_block_comment = raw_line.contains(";/*");
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
                    r#"whitespace/semicolon"#,
                    3,
                    r#"Missing space after ;"#,
                );
            }
        }
    }

    if has_semicolon {
        let semicolon_count = elided_line.bytes().filter(|&b| b == b';').count();
        let switch_case_single_line =
            (keywords.has_case() || keywords.has_default()) && elided_line.contains("break;");

        if semicolon_count > 1 && !keywords.has_for() && !switch_case_single_line {
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
                    "whitespace/newline",
                    0,
                    "More than one command on the same line",
                );
            }
        }
    }

    // 10. Brace and semicolon spacing.
    if has_brace {
        if let Some(brace_pos) = elided_line.find('{') {
            if brace_pos > 0 {
                let prefix = &elided_line[..brace_pos];
                let last_char = prefix.chars().last();
                if let Some(c) = last_char {
                    if !matches!(c, ' ' | '(' | '{' | '>') {
                        let missing_space_before_qualified_brace =
                            QUALIFIED_BRACE_RE.is_match(elided_line);
                        if (!is_braced_initialization(clean_lines, elided_line, linenum)
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
                    }
                }
            }
        }
    }

    if elided_line.contains("}else") {
        linter.error(
            linenum,
            r#"whitespace/braces"#,
            5,
            r#"Missing space before else"#,
        );
    }

    if let Some(colon_pos) = elided_line.find(':') {
        let suffix = &elided_line[colon_pos + 1..];
        let trimmed_suffix = suffix.trim();
        if trimmed_suffix == ";" {
            linter.error(
                linenum,
                r#"whitespace/semicolon"#,
                5,
                r#"Semicolon defining empty statement. Use {} instead."#,
            );
        }
    } else if elided_line.trim() == ";" {
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
