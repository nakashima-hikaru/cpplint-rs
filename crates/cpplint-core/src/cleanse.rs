use crate::options::Options;
use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use std::borrow::Cow;
use std::sync::LazyLock;

static INCLUDE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"^\s*#\s*include\s*([<"])([^>"]*)[>"].*$"#).unwrap());
static ESCAPE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"\\([abfnrtv?"\\\']|\d+|x[0-9a-fA-F]+)"#).unwrap());

const ALT_TOKEN_REPLACEMENT: &[(&str, &str)] = &[
    ("and", "&&"),
    ("and_eq", "&="),
    ("bitand", "&"),
    ("bitor", "|"),
    ("compl", "~"),
    ("not", "!"),
    ("not_eq", "!="),
    ("or", "||"),
    ("or_eq", "|="),
    ("xor", "^"),
    ("xor_eq", "^="),
];

static ALT_TOKEN_AC: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .build(ALT_TOKEN_REPLACEMENT.iter().map(|(token, _)| *token))
        .unwrap()
});

const KEYWORDS: &[&str] = &[
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
    "protected",
    "private",
    "signals",
    "slots",
    "sizeof",
    "elif",
    "typedef",
    "using",
    "static_cast",
    "reinterpret_cast",
    "const_cast",
    "else",
    "do",
];

static KEYWORDS_AC: LazyLock<AhoCorasick> = LazyLock::new(|| AhoCorasick::new(KEYWORDS).unwrap());

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct MatchedKeywords(u32);

impl MatchedKeywords {
    pub(crate) const IF: u32 = 1 << 0;
    pub(crate) const FOR: u32 = 1 << 1;
    pub(crate) const WHILE: u32 = 1 << 2;
    pub(crate) const SWITCH: u32 = 1 << 3;
    pub(crate) const CASE: u32 = 1 << 4;
    pub(crate) const DEFAULT: u32 = 1 << 5;
    pub(crate) const RETURN: u32 = 1 << 6;
    pub(crate) const NEW: u32 = 1 << 7;
    pub(crate) const DELETE: u32 = 1 << 8;
    pub(crate) const CATCH: u32 = 1 << 9;
    pub(crate) const OPERATOR: u32 = 1 << 10;
    pub(crate) const VA_OPT: u32 = 1 << 11;
    pub(crate) const ACCESS: u32 = 1 << 12;
    pub(crate) const SIZEOF: u32 = 1 << 13;
    pub(crate) const ELIF: u32 = 1 << 14;
    pub(crate) const TYPEDEF: u32 = 1 << 15;
    pub(crate) const USING: u32 = 1 << 16;
    pub(crate) const CAST: u32 = 1 << 17;
    pub(crate) const ELSE: u32 = 1 << 18;
    pub(crate) const DO: u32 = 1 << 19;

    pub fn from_line(line: &str) -> Self {
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
                24 => Self::ELSE,
                25 => Self::DO,
                _ => 0,
            };
        }
        Self(bits)
    }

    #[inline(always)]
    pub fn has_if(&self) -> bool {
        (self.0 & Self::IF) != 0
    }
    #[inline(always)]
    pub fn has_for(&self) -> bool {
        (self.0 & Self::FOR) != 0
    }
    #[inline(always)]
    pub fn has_while(&self) -> bool {
        (self.0 & Self::WHILE) != 0
    }
    #[inline(always)]
    pub fn has_switch(&self) -> bool {
        (self.0 & Self::SWITCH) != 0
    }
    #[inline(always)]
    pub fn has_case(&self) -> bool {
        (self.0 & Self::CASE) != 0
    }
    #[inline(always)]
    pub fn has_default(&self) -> bool {
        (self.0 & Self::DEFAULT) != 0
    }
    #[inline(always)]
    pub fn has_return(&self) -> bool {
        (self.0 & Self::RETURN) != 0
    }
    #[inline(always)]
    pub fn has_new(&self) -> bool {
        (self.0 & Self::NEW) != 0
    }
    #[inline(always)]
    pub fn has_delete(&self) -> bool {
        (self.0 & Self::DELETE) != 0
    }
    #[inline(always)]
    pub fn has_catch(&self) -> bool {
        (self.0 & Self::CATCH) != 0
    }
    #[inline(always)]
    pub fn has_operator(&self) -> bool {
        (self.0 & Self::OPERATOR) != 0
    }
    #[inline(always)]
    pub fn has_va_opt(&self) -> bool {
        (self.0 & Self::VA_OPT) != 0
    }
    #[inline(always)]
    pub fn has_access(&self) -> bool {
        (self.0 & Self::ACCESS) != 0
    }
    #[inline(always)]
    pub fn has_sizeof(&self) -> bool {
        (self.0 & Self::SIZEOF) != 0
    }
    #[inline(always)]
    pub fn has_elif(&self) -> bool {
        (self.0 & Self::ELIF) != 0
    }
    #[inline(always)]
    pub fn has_typedef(&self) -> bool {
        (self.0 & Self::TYPEDEF) != 0
    }
    #[inline(always)]
    pub fn has_using(&self) -> bool {
        (self.0 & Self::USING) != 0
    }
    #[inline(always)]
    pub fn has_else(&self) -> bool {
        (self.0 & Self::ELSE) != 0
    }
    #[inline(always)]
    pub fn has_do(&self) -> bool {
        (self.0 & Self::DO) != 0
    }
    #[inline(always)]
    pub fn has_any_cast(&self) -> bool {
        (self.0 & Self::CAST) != 0
    }

    #[inline(always)]
    pub fn bits(&self) -> u32 {
        self.0
    }

    #[inline(always)]
    pub fn has_any_control_struct(&self) -> bool {
        const MASK: u32 = MatchedKeywords::IF
            | MatchedKeywords::ELIF
            | MatchedKeywords::FOR
            | MatchedKeywords::WHILE
            | MatchedKeywords::SWITCH
            | MatchedKeywords::RETURN
            | MatchedKeywords::NEW
            | MatchedKeywords::DELETE
            | MatchedKeywords::CATCH
            | MatchedKeywords::SIZEOF;
        (self.0 & MASK) != 0
    }
}

const RAW_STRING_PREFIXES: &[&str] = &["u8R\"", "uR\"", "UR\"", "LR\"", "R\""];
static RAW_STRING_PREFIXES_AC: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .build(RAW_STRING_PREFIXES)
        .unwrap()
});

fn is_valid_alt_token_match(bytes: &[u8], start: usize, end: usize) -> bool {
    start > 0
        && matches!(bytes[start - 1], b' ' | b'=' | b'(')
        && (end == bytes.len() || matches!(bytes[end], b' ' | b'('))
}

pub fn find_alternate_tokens(line: &str) -> Vec<(&'static str, &'static str)> {
    let bytes = line.as_bytes();
    let mut matches = Vec::new();
    for mat in ALT_TOKEN_AC.find_iter(line) {
        let start = mat.start();
        let end = mat.end();
        if !is_valid_alt_token_match(bytes, start, end) {
            continue;
        }
        matches.push(ALT_TOKEN_REPLACEMENT[mat.pattern().as_usize()]);
    }
    matches
}

pub struct CleansedLines {
    pub elided: Vec<String>,
    elided_without_alternate_tokens: Option<Vec<String>>,
    pub lines: Vec<String>,
    pub raw_lines: Vec<String>,
    pub lines_without_raw_strings: Vec<String>,
    pub has_comment: Vec<bool>,
    pub keywords: Vec<MatchedKeywords>,
}

impl CleansedLines {
    pub fn new(raw_lines: Vec<String>) -> Self {
        let options = Options::new();
        Self::new_with_options(raw_lines, &options, "")
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn new_with_options(raw_lines: Vec<String>, options: &Options, filename: &str) -> Self {
        let n = raw_lines.len();
        let mut lines = Vec::with_capacity(n);
        let mut elided = Vec::with_capacity(n);
        let mut has_comment = Vec::with_capacity(n);
        let mut lines_without_raw_strings = Vec::with_capacity(n);
        let mut keywords = Vec::with_capacity(n);

        let mut in_block_comment = false;
        let mut raw_delimiter = String::new();
        let replace_alt_tokens = !options.should_print_error("readability/alt_tokens", filename, 0);
        let mut elided_without_alternate_tokens = replace_alt_tokens.then(|| Vec::with_capacity(n));

        for raw_line in &raw_lines {
            // 1. Cleanse raw strings
            let mut line_without_raw: Cow<'_, str> = Cow::Borrowed(raw_line);

            if !raw_delimiter.is_empty() {
                if let Some(pos) = raw_line.find(&raw_delimiter) {
                    let leading_space_count = raw_line
                        .bytes()
                        .take_while(|b| b.is_ascii_whitespace())
                        .count();
                    let mut s = String::with_capacity(
                        leading_space_count + 2 + raw_line.len() - (pos + raw_delimiter.len()),
                    );
                    for _ in 0..leading_space_count {
                        s.push(' ');
                    }
                    s.push_str("\"\"");
                    s.push_str(&raw_line[pos + raw_delimiter.len()..]);
                    line_without_raw = Cow::Owned(s);
                    raw_delimiter.clear();
                } else {
                    line_without_raw = Cow::Borrowed("\"\"");
                }
            }

            while raw_delimiter.is_empty() {
                let Some((prefix, delimiter_text, suffix)) =
                    find_raw_string_start(&line_without_raw)
                else {
                    break;
                };

                if prefix_is_in_comment_or_literal(prefix) {
                    break;
                }

                raw_delimiter.clear();
                raw_delimiter.push(')');
                raw_delimiter.push_str(delimiter_text);
                raw_delimiter.push('"');

                if let Some(end) = suffix.find(&raw_delimiter) {
                    let mut s = String::with_capacity(
                        prefix.len() + 2 + suffix.len() - (end + raw_delimiter.len()),
                    );
                    s.push_str(prefix);
                    s.push_str("\"\"");
                    s.push_str(&suffix[end + raw_delimiter.len()..]);
                    line_without_raw = Cow::Owned(s);
                    raw_delimiter.clear();
                } else {
                    let mut s = String::with_capacity(prefix.len() + 2);
                    s.push_str(prefix);
                    s.push_str("\"\"");
                    line_without_raw = Cow::Owned(s);
                }
            }

            let line_without_raw_owned = line_without_raw.into_owned();
            lines_without_raw_strings.push(line_without_raw_owned.clone());

            // 2. Cleanse comments
            let (comment_removed, is_comment, still_in_block) =
                cleanse_comments_line(&line_without_raw_owned, in_block_comment);
            lines.push(comment_removed.to_string());
            has_comment.push(is_comment);
            in_block_comment = still_in_block;

            // 3. Collapse strings
            let collapsed_line = collapse_strings(&comment_removed);
            if let Some(lines_without_alt_tokens) = &mut elided_without_alternate_tokens {
                let elided_line = replace_alternate_tokens(&collapsed_line);
                lines_without_alt_tokens.push(collapsed_line.to_string());
                elided.push(elided_line.into_owned());
            } else {
                elided.push(collapsed_line.into_owned());
            }

            keywords.push(MatchedKeywords::from_line(&elided[elided.len() - 1]));
        }

        CleansedLines {
            elided,
            elided_without_alternate_tokens,
            lines,
            raw_lines,
            lines_without_raw_strings,
            has_comment,
            keywords,
        }
    }

    pub fn line_without_alternate_tokens(&self, linenum: usize) -> &str {
        self.elided_without_alternate_tokens
            .as_ref()
            .and_then(|lines| lines.get(linenum))
            .map_or_else(
                || self.elided[linenum].as_str(),
                std::string::String::as_str,
            )
    }
}

pub fn cleanse_raw_strings(raw_lines: &[String]) -> Vec<String> {
    let mut result = Vec::with_capacity(raw_lines.len());
    let mut delimiter = String::new();

    for line in raw_lines {
        let mut new_line = line.clone();

        if !delimiter.is_empty() {
            if let Some(pos) = line.find(&delimiter) {
                // End of raw string
                // Match leading space
                let leading_space = line
                    .chars()
                    .take_while(|ch| ch.is_whitespace())
                    .collect::<String>();
                new_line = format!("{}\"\"{}", leading_space, &line[pos + delimiter.len()..]);
                delimiter.clear();
            } else {
                new_line = "\"\"".to_string();
            }
        }

        while delimiter.is_empty() {
            let Some((prefix, raw_delimiter, suffix)) = find_raw_string_start(&new_line) else {
                break;
            };

            if prefix_is_in_comment_or_literal(prefix) {
                break;
            }

            delimiter = format!("){}\"", raw_delimiter);
            if let Some(end) = suffix.find(&delimiter) {
                new_line = format!("{}\"\"{}", prefix, &suffix[end + delimiter.len()..]);
                delimiter.clear();
            } else {
                new_line = format!("{}\"\"", prefix);
            }
        }
        result.push(new_line);
    }
    result
}

fn find_raw_string_start(line: &str) -> Option<(&str, &str, &str)> {
    for mat in RAW_STRING_PREFIXES_AC.find_iter(line) {
        let start = mat.start();
        let prefix = RAW_STRING_PREFIXES[mat.pattern()];
        if !has_raw_string_word_boundary(line, start) {
            continue;
        }

        let before = &line[..start];
        let after_prefix = &line[start + prefix.len()..];
        let Some(open_paren) = after_prefix.find('(') else {
            continue;
        };
        let raw_delimiter = &after_prefix[..open_paren];
        if raw_delimiter
            .chars()
            .any(|ch| ch.is_whitespace() || ch == '\\' || ch == '(' || ch == ')')
        {
            continue;
        }
        let suffix = &after_prefix[open_paren + 1..];
        return Some((before, raw_delimiter, suffix));
    }
    None
}

fn has_raw_string_word_boundary(line: &str, start: usize) -> bool {
    if start == 0 {
        return true;
    }
    line[..start]
        .chars()
        .last()
        .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'))
}

fn prefix_is_in_comment_or_literal(prefix: &str) -> bool {
    let mut escaped = false;
    let mut in_single = false;
    let mut in_double = false;
    let mut it = prefix.chars().peekable();
    while let Some(ch) = it.next() {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if ch == '/' && it.peek() == Some(&'/') && !in_single && !in_double {
            return true;
        }
    }
    in_single || in_double
}

pub fn cleanse_comments(line: &str) -> (String, bool) {
    let (mut lines, has_comment) = cleanse_comments_from_lines(&[line.to_string()]);
    (lines.remove(0), has_comment[0])
}

fn cleanse_comments_from_lines(lines: &[String]) -> (Vec<String>, Vec<bool>) {
    let mut result = Vec::with_capacity(lines.len());
    let mut has_comment = Vec::with_capacity(lines.len());
    let mut in_block_comment = false;

    for line in lines {
        let (comment_removed, is_comment, still_in_block) =
            cleanse_comments_line(line, in_block_comment);
        result.push(comment_removed.into_owned());
        has_comment.push(is_comment);
        in_block_comment = still_in_block;
    }

    (result, has_comment)
}

fn cleanse_comments_line<'a>(
    line: &'a str,
    mut in_block_comment: bool,
) -> (Cow<'a, str>, bool, bool) {
    if line.is_empty() {
        return (Cow::Borrowed(""), false, in_block_comment);
    }

    // Quick check if we need to do anything.
    // If we're not in a block comment and the line has no interesting characters, return as-is (possibly trimmed)
    if !in_block_comment {
        let bytes = line.as_bytes();
        let has_special = bytes
            .iter()
            .any(|&b| matches!(b, b'/' | b'*' | b'"' | b'\'' | b'\\'));
        if !has_special {
            let trimmed = line.trim_end();
            return (Cow::Borrowed(trimmed), false, false);
        }
    }

    let mut result = String::with_capacity(line.len());
    let mut is_comment = false;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut just_closed_block_comment = false;

    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if in_block_comment {
            is_comment = true;
            if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                i += 2;
                in_block_comment = false;
                just_closed_block_comment = true;
                continue;
            }
            i += 1;
            continue;
        }

        if escaped {
            result.push(b as char);
            escaped = false;
            just_closed_block_comment = false;
            i += 1;
            continue;
        }

        if just_closed_block_comment
            && b.is_ascii_whitespace()
            && (result.is_empty()
                || result
                    .as_bytes()
                    .last()
                    .is_some_and(|&last| last.is_ascii_whitespace()))
        {
            i += 1;
            continue;
        }

        if b == b'\\' && (in_string || in_char) {
            result.push('\\');
            escaped = true;
            just_closed_block_comment = false;
            i += 1;
            continue;
        }

        if b == b'"' && !in_char {
            in_string = !in_string;
            result.push('"');
            just_closed_block_comment = false;
            i += 1;
            continue;
        }

        if b == b'\'' && !in_string {
            in_char = !in_char;
            result.push('\'');
            just_closed_block_comment = false;
            i += 1;
            continue;
        }

        if !in_string && !in_char && b == b'/' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'/' {
                is_comment = true;
                break;
            }
            if bytes[i + 1] == b'*' {
                in_block_comment = true;
                is_comment = true;
                i += 2;
                continue;
            }
        }

        result.push(b as char);
        just_closed_block_comment = false;
        i += 1;
    }

    if !is_comment
        && !in_block_comment
        && !escaped
        && !in_string
        && !in_char
        && result.len() == line.trim_end().len()
    {
        return (Cow::Borrowed(line.trim_end()), false, false);
    }

    (
        Cow::Owned(result.trim_end().to_string()),
        is_comment,
        in_block_comment,
    )
}

pub fn is_cpp_string(line: &str) -> bool {
    let mut escaped = false;
    let mut in_string = false;
    for c in line.chars() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            in_string = !in_string;
        }
    }
    in_string
}

pub fn collapse_strings<'a>(elided: &'a str) -> Cow<'a, str> {
    if INCLUDE_RE.is_match(elided) {
        return Cow::Borrowed(elided);
    }

    if !elided.contains('\\') && !elided.contains('"') && !elided.contains('\'') {
        return Cow::Borrowed(elided);
    }

    // Remove escapes
    let result = if elided.contains('\\') {
        Cow::Owned(ESCAPE_RE.replace_all(elided, "").to_string())
    } else {
        Cow::Borrowed(elided)
    };

    let collapsed = collapse_quotes_and_separators(&result);
    if collapsed.len() == result.len() {
        result
    } else {
        Cow::Owned(collapsed)
    }
}

pub fn replace_alternate_tokens<'a>(line: &'a str) -> Cow<'a, str> {
    let bytes = line.as_bytes();
    let mut last = 0usize;
    let mut result = String::new();

    for mat in ALT_TOKEN_AC.find_iter(line) {
        let start = mat.start();
        let end = mat.end();
        if !is_valid_alt_token_match(bytes, start, end) {
            continue;
        }

        if result.is_empty() {
            result.reserve(line.len());
        }

        let (token, replacement) = ALT_TOKEN_REPLACEMENT[mat.pattern().as_usize()];
        result.push_str(&line[last..start]);
        result.push_str(replacement);
        last = if end < bytes.len() && matches!(token, "not" | "compl") {
            end + 1
        } else {
            end
        };
    }

    if result.is_empty() {
        return Cow::Borrowed(line);
    }

    result.push_str(&line[last..]);
    Cow::Owned(result)
}

fn collapse_quotes_and_separators(elided: &str) -> String {
    let mut result = String::with_capacity(elided.len());
    let bytes = elided.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            let mut found = false;
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'"' {
                    result.push_str("\"\"");
                    i = j + 1;
                    found = true;
                    break;
                }
                j += 1;
            }
            if !found {
                result.push('"');
                if i + 1 < bytes.len() {
                    result.push_str(&elided[i + 1..]);
                }
                return result;
            }
            continue;
        }

        if b == b'\'' {
            // Check for digit separator
            if i > 0 && i + 1 < bytes.len() {
                let prev = bytes[i - 1];
                let next = bytes[i + 1];
                if prev.is_ascii_hexdigit() && (next.is_ascii_alphanumeric() || next == b'_') {
                    i += 1;
                    continue;
                }
            }

            let mut found = false;
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'\'' {
                    result.push_str("''");
                    i = j + 1;
                    found = true;
                    break;
                }
                j += 1;
            }
            if !found {
                result.push('\'');
                if i + 1 < bytes.len() {
                    result.push_str(&elided[i + 1..]);
                }
                return result;
            }
            continue;
        }

        result.push(b as char);
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanse_raw_strings_handles_single_and_multiline_forms() {
        let lines = vec![
            r#"auto a = R"(hello)";"#.to_string(),
            r#"auto b = R"tag(line1"#.to_string(),
            r#"line2)tag";"#.to_string(),
            r#"// R"(comment raw string should stay)"#.to_string(),
        ];

        let actual = cleanse_raw_strings(&lines);

        assert_eq!(actual[0], r#"auto a = "";"#);
        assert_eq!(actual[1], "auto b = \"\"");
        assert_eq!(actual[2], "\"\";");
        assert_eq!(actual[3], r#"// R"(comment raw string should stay)"#);
    }

    #[test]
    fn cleanse_raw_strings_requires_word_boundary_before_prefix() {
        let lines = vec![
            r#"auto x = fooR"(hello)";"#.to_string(),
            r#"auto y = (R"(hello)");"#.to_string(),
        ];

        let actual = cleanse_raw_strings(&lines);

        assert_eq!(actual[0], r#"auto x = fooR"(hello)";"#);
        assert_eq!(actual[1], r#"auto y = ("");"#);
    }

    #[test]
    fn cleanse_raw_strings_ignores_prefixes_inside_string_and_char_literals() {
        let lines = vec![
            r#"const char* s = "R\"(not raw)\"";"#.to_string(),
            r#"char c = 'R'; auto x = R"(raw)";"#.to_string(),
        ];

        let actual = cleanse_raw_strings(&lines);

        assert_eq!(actual[0], r#"const char* s = "R\"(not raw)\"";"#);
        assert_eq!(actual[1], r#"char c = 'R'; auto x = "";"#);
    }

    #[test]
    fn collapse_strings_keeps_digit_separators_out_of_char_collapse() {
        assert_eq!(collapse_strings("int x = 1'000'000;"), "int x = 1000000;");
        assert_eq!(collapse_strings("char c = 'x';"), "char c = '';");
        assert_eq!(collapse_strings("auto s = \"abc\";"), "auto s = \"\";");
    }

    #[test]
    fn collapse_strings_processes_quotes_in_order_and_keeps_unmatched_tail() {
        assert_eq!(collapse_strings("'x' \"abc\""), "'' \"\"");
        assert_eq!(collapse_strings("\"unterminated"), "\"unterminated");
        assert_eq!(collapse_strings("'x' \"unterminated"), "'' \"unterminated");
    }

    #[test]
    fn replace_alternate_tokens_matches_cpp_rules() {
        assert_eq!(
            replace_alternate_tokens("if (true or false)"),
            "if (true || false)"
        );
        assert_eq!(replace_alternate_tokens("if (not ready)"), "if (!ready)");
        assert_eq!(replace_alternate_tokens("x = compl y;"), "x = ~y;");
        assert_eq!(
            replace_alternate_tokens("if (true and(foo))"),
            "if (true &&(foo))"
        );
        assert_eq!(replace_alternate_tokens("android"), "android");
    }

    #[test]
    fn cleansed_lines_normalizes_alternate_tokens_but_preserves_detection_view() {
        let mut options = Options::new();
        options.add_filter("-readability/alt_tokens");
        let lines = vec![
            "// Copyright 2026".to_string(),
            "if (true or false) return;".to_string(),
            "if (not ready) return;".to_string(),
        ];

        let cleansed = CleansedLines::new_with_options(lines, &options, "test.cpp");

        assert_eq!(
            cleansed.line_without_alternate_tokens(1),
            "if (true or false) return;"
        );
        assert_eq!(cleansed.elided[1], "if (true || false) return;");
        assert_eq!(
            cleansed.line_without_alternate_tokens(2),
            "if (not ready) return;"
        );
        assert_eq!(cleansed.elided[2], "if (!ready) return;");
    }

    #[test]
    fn cleansed_lines_preserves_alternate_tokens_when_check_is_enabled() {
        let lines = vec![
            "// Copyright 2026".to_string(),
            "if (true or false) return;".to_string(),
        ];

        let cleansed = CleansedLines::new(lines);

        assert_eq!(
            cleansed.line_without_alternate_tokens(1),
            "if (true or false) return;"
        );
        assert_eq!(cleansed.elided[1], "if (true or false) return;");
    }

    #[test]
    fn cleanse_comments_handles_multiline_block_comments() {
        let lines = vec![
            "int a = 0; /* start".to_string(),
            "still comment".to_string(),
            "end */ int b = 1;".to_string(),
            "const char* s = \"// not a comment\";".to_string(),
        ];

        let (actual, has_comment) = cleanse_comments_from_lines(&lines);

        assert_eq!(actual[0], "int a = 0;");
        assert_eq!(actual[1], "");
        assert_eq!(actual[2], "int b = 1;");
        assert_eq!(actual[3], "const char* s = \"// not a comment\";");
        assert_eq!(has_comment, vec![true, true, true, false]);
    }

    #[test]
    fn cleanse_comments_preserves_comment_markers_inside_strings_and_chars() {
        let lines = vec![
            r#"const char* slash = "/* not a comment */";"#.to_string(),
            r#"char c = '/';  // real comment"#.to_string(),
            r#"const char* line = "// not a comment";"#.to_string(),
        ];

        let (actual, has_comment) = cleanse_comments_from_lines(&lines);

        assert_eq!(actual[0], r#"const char* slash = "/* not a comment */";"#);
        assert_eq!(actual[1], r#"char c = '/';"#);
        assert_eq!(actual[2], r#"const char* line = "// not a comment";"#);
        assert_eq!(has_comment, vec![false, true, false]);
    }

    #[test]
    fn cleanse_comments_handles_multiple_block_comments_on_one_line() {
        let lines = vec![r#"int value = /* one */ 1 + /* two */ 2;"#.to_string()];

        let (actual, has_comment) = cleanse_comments_from_lines(&lines);

        assert_eq!(actual[0], "int value = 1 + 2;");
        assert_eq!(has_comment, vec![true]);
    }

    #[test]
    fn find_alternate_tokens_reports_multiple_matches() {
        let actual = find_alternate_tokens("if (true or true and (not true)) return;");
        assert_eq!(actual.len(), 3);
        assert_eq!(actual[0], ("or", "||"));
        assert_eq!(actual[1], ("and", "&&"));
        assert_eq!(actual[2], ("not", "!"));
    }
}
