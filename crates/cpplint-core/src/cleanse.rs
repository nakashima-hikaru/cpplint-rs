use crate::options::Options;
use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
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
}

impl CleansedLines {
    pub fn new(raw_lines: Vec<String>) -> Self {
        let options = Options::new();
        Self::new_with_options(raw_lines, &options, "")
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn new_with_options(raw_lines: Vec<String>, options: &Options, filename: &str) -> Self {
        let lines_without_raw_strings = cleanse_raw_strings(&raw_lines);
        let (lines, has_comment) = cleanse_comments_from_lines(&lines_without_raw_strings);
        let mut elided = Vec::with_capacity(raw_lines.len());
        let replace_alt_tokens = !options.should_print_error("readability/alt_tokens", filename, 0);
        let mut elided_without_alternate_tokens =
            replace_alt_tokens.then(|| Vec::with_capacity(raw_lines.len()));

        for comment_removed in &lines {
            let collapsed_line = collapse_strings(comment_removed);
            if let Some(lines_without_alt_tokens) = &mut elided_without_alternate_tokens {
                let elided_line = replace_alternate_tokens(&collapsed_line);
                lines_without_alt_tokens.push(collapsed_line);
                elided.push(elided_line);
            } else {
                elided.push(collapsed_line);
            }
        }

        CleansedLines {
            elided,
            elided_without_alternate_tokens,
            lines,
            raw_lines,
            lines_without_raw_strings,
            has_comment,
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
    let chars: Vec<char> = prefix.chars().collect();
    let mut idx = 0usize;
    while idx + 1 < chars.len() {
        let ch = chars[idx];
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if ch == '/' && chars[idx + 1] == '/' && !in_single && !in_double {
            return true;
        }
        idx += 1;
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
        result.push(comment_removed);
        has_comment.push(is_comment);
        in_block_comment = still_in_block;
    }

    (result, has_comment)
}

fn cleanse_comments_line(line: &str, mut in_block_comment: bool) -> (String, bool, bool) {
    let mut result = String::with_capacity(line.len());
    let mut is_comment = false;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut just_closed_block_comment = false;

    let mut it = line.chars().peekable();
    while let Some(ch) = it.next() {
        if in_block_comment {
            is_comment = true;
            if ch == '*' && it.peek() == Some(&'/') {
                it.next(); // consume '/'
                in_block_comment = false;
                just_closed_block_comment = true;
            }
            continue;
        }

        if escaped {
            result.push(ch);
            escaped = false;
            just_closed_block_comment = false;
            continue;
        }

        if just_closed_block_comment
            && ch.is_whitespace()
            && (result.is_empty() || result.ends_with(|prev: char| prev.is_whitespace()))
        {
            continue;
        }

        if ch == '\\' && (in_string || in_char) {
            result.push(ch);
            escaped = true;
            just_closed_block_comment = false;
            continue;
        }

        if ch == '"' && !in_char {
            in_string = !in_string;
            result.push(ch);
            just_closed_block_comment = false;
            continue;
        }

        if ch == '\'' && !in_string {
            in_char = !in_char;
            result.push(ch);
            just_closed_block_comment = false;
            continue;
        }

        if !in_string && !in_char && ch == '/' {
            match it.peek() {
                Some(&'/') => {
                    is_comment = true;
                    break;
                }
                Some(&'*') => {
                    it.next(); // consume '*'
                    is_comment = true;
                    in_block_comment = true;
                    continue;
                }
                _ => {}
            }
        }

        result.push(ch);
        just_closed_block_comment = false;
    }

    (result.trim_end().to_string(), is_comment, in_block_comment)
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

pub fn collapse_strings(elided: &str) -> String {
    if INCLUDE_RE.is_match(elided) {
        return elided.to_string();
    }

    if !elided.contains('\\') && !elided.contains('"') && !elided.contains('\'') {
        return elided.to_string();
    }

    // Remove escapes
    let result = if elided.contains('\\') {
        ESCAPE_RE.replace_all(elided, "").to_string()
    } else {
        elided.to_string()
    };
    collapse_quotes_and_separators(&result)
}

pub fn replace_alternate_tokens(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut result = String::with_capacity(line.len());
    let mut last = 0usize;
    let mut replaced = false;

    for mat in ALT_TOKEN_AC.find_iter(line) {
        let start = mat.start();
        let end = mat.end();
        if !is_valid_alt_token_match(bytes, start, end) {
            continue;
        }

        let (token, replacement) = ALT_TOKEN_REPLACEMENT[mat.pattern().as_usize()];
        result.push_str(&line[last..start]);
        result.push_str(replacement);
        last = if end < bytes.len() && matches!(token, "not" | "compl") {
            end + 1
        } else {
            end
        };
        replaced = true;
    }

    if !replaced {
        return line.to_string();
    }

    result.push_str(&line[last..]);
    result
}

fn collapse_quotes_and_separators(elided: &str) -> String {
    let mut result = String::with_capacity(elided.len());
    let mut it = elided.chars().enumerate().peekable();

    while let Some((i, ch)) = it.next() {
        if ch == '"' {
            let mut found = false;
            let mut buffer = String::new();
            for (_, next_ch) in it.by_ref() {
                if next_ch == '"' {
                    result.push_str("\"\"");
                    found = true;
                    break;
                }
                buffer.push(next_ch);
            }
            if !found {
                result.push('"');
                result.push_str(&buffer);
                return result;
            }
            continue;
        }

        if ch == '\'' {
            // Check for digit separator
            if i > 0 && i + 1 < elided.len() {
                let prev = elided.as_bytes()[i - 1];
                let next = elided.as_bytes()[i + 1];
                if prev.is_ascii_hexdigit() && (next.is_ascii_alphanumeric() || next == b'_') {
                    continue;
                }
            }

            let mut found = false;
            let mut buffer = String::new();
            for (_, next_ch) in it.by_ref() {
                if next_ch == '\'' {
                    result.push_str("''");
                    found = true;
                    break;
                }
                buffer.push(next_ch);
            }
            if !found {
                result.push('\'');
                result.push_str(&buffer);
                return result;
            }
            continue;
        }

        result.push(ch);
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
