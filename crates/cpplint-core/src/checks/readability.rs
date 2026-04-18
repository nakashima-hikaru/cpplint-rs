use crate::categories::Category;
use crate::cleanse::{CleansedLines, LineFeatures, MatchedKeywords, collapse_strings};
use crate::file_linter::FileLinter;
use crate::line_utils;
use crate::regex_utils;
use crate::string_utils;
use aho_corasick::AhoCorasick;
use regex::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;

fn is_control_statement_start(s: &str) -> bool {
    ["if", "for", "while", "switch", "catch", "else"]
        .iter()
        .any(|&kw| string_utils::trimmed_starts_with_word(s, kw))
}
static FUNCTION_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"([A-Za-z_~][\w:]*(?:::[A-Za-z_~][\w:]*)*)\s*\([^;{}]*\)\s*$"#).unwrap()
});

static IF_ELSE_AC: LazyLock<AhoCorasick> =
    LazyLock::new(|| AhoCorasick::new(["if", "else"]).unwrap());
static MULTILINE_IF_OPEN_BRACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*(?:\[\[(?:un)?likely\]\]\s*)?\{"#).unwrap());
static MULTILINE_IF_MULTI_COMMAND_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^;[\s}]*(\\?)$"#).unwrap());

static MULTILINE_IF_LAMBDA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^[^{};]*\[[^\[\]]*\][^{}]*\{[^{}]*\}\s*\)*[;,]\s*$"#).unwrap());
static NAMESPACE_START_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*namespace\b\s*([:\w]+)?(.*)$"#).unwrap());
fn is_check_const(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 2 {
        return false;
    }
    let first = bytes[0];
    let last = bytes[bytes.len() - 1];
    (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'')
}
const INHERITANCE_KEYWORDS: [&str; 3] = ["virtual", "override", "final"];
static INHERITANCE_KEYWORDS_AC: LazyLock<AhoCorasick> =
    LazyLock::new(|| AhoCorasick::new(INHERITANCE_KEYWORDS).unwrap());

const CHECK_MACROS: [&str; 6] = [
    "DCHECK",
    "CHECK",
    "EXPECT_TRUE",
    "ASSERT_TRUE",
    "EXPECT_FALSE",
    "ASSERT_FALSE",
];
static CHECK_MACROS_AC: LazyLock<AhoCorasick> =
    LazyLock::new(|| AhoCorasick::new(CHECK_MACROS).unwrap());
static NAMESPACE_TERMINATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*};*\s*(//|/\*).*\bnamespace\b"#).unwrap());
static ANONYMOUS_NAMESPACE_TERMINATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*};*\s*(//|/\*).*\bnamespace[\*/\.\\\s]*$"#).unwrap());
static ANONYMOUS_NAMESPACE_TERM_MSG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*}.*\b(namespace anonymous|anonymous namespace)\b"#).unwrap()
});

fn is_test_like_function(name: &str) -> bool {
    name.starts_with("TEST") || name.starts_with("Test")
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn check(linter: &mut FileLinter, clean_lines: &CleansedLines<'_>, linenum: usize) {
    let elided_line = &clean_lines.elided[linenum];
    let raw_line = &clean_lines.raw_lines[linenum];
    let line_features = clean_lines.line_features[linenum];
    let has_brace = line_features.contains(LineFeatures::BRACE);
    let has_semicolon = line_features.contains(LineFeatures::SEMI);

    let has_slash = raw_line.contains('/');
    let keywords = clean_lines.keywords(linenum);

    if keywords.has_alt_token() {
        check_alt_tokens(linter, clean_lines, linenum);
    }

    if keywords.has_using() || keywords.has_namespace() {
        check_namespace_using(linter, elided_line, linenum);
        check_unnamed_namespace_in_header(linter, elided_line, linenum);
    }

    // Always check namespace indentation as it depends on context facts, not just keywords
    check_namespace_indentation(linter, clean_lines, elided_line, linenum);

    if has_brace || has_slash || has_semicolon {
        check_namespace_termination_comment(linter, clean_lines, linenum);
    }

    if elided_line.contains("CHECK")
        || elided_line.contains("ASSERT_")
        || elided_line.contains("EXPECT_")
    {
        check_check_macro(linter, clean_lines, elided_line, linenum);
    }

    if has_slash {
        check_multiline_comments(linter, clean_lines, linenum);
    }
    if raw_line.contains('"') {
        check_multiline_strings(linter, clean_lines, linenum);
    }

    let has_control = keywords.intersects(
        MatchedKeywords::IF
            | MatchedKeywords::ELSE
            | MatchedKeywords::FOR
            | MatchedKeywords::WHILE
            | MatchedKeywords::DO,
    );
    if has_brace
        || has_control
        || keywords.has_virtual()
        || keywords.has_override()
        || keywords.has_final()
    {
        check_redundant_virtuals(linter, clean_lines, elided_line, linenum);
    }

    if has_brace || has_control {
        check_braces(linter, clean_lines, elided_line, linenum);
        check_single_line_control_bodies(linter, clean_lines, elided_line, linenum);
        check_multiline_if_else_bodies(linter, clean_lines, elided_line, linenum);
        check_function_size(linter, clean_lines, elided_line, linenum);
    }

    if has_brace || has_semicolon || has_control {
        check_empty_bodies(linter, clean_lines, elided_line, linenum);
        check_trailing_semicolon(linter, clean_lines, elided_line, linenum);
    }

    if line_features.contains(LineFeatures::PAREN) {
        check_missing_function_body(linter, clean_lines, linenum);
    }
}

fn check_alt_tokens(linter: &mut FileLinter, clean_lines: &CleansedLines<'_>, linenum: usize) {
    let use_raw_block_comment = clean_lines.has_comment[linenum]
        && clean_lines.elided[linenum].trim().is_empty()
        && is_interior_block_comment_line(clean_lines.raw_lines[linenum]);
    let line = if use_raw_block_comment {
        clean_lines.raw_lines[linenum]
    } else {
        clean_lines.line_without_alternate_tokens(linenum)
    };
    if line.trim_start().starts_with('#')
        || (!use_raw_block_comment
            && clean_lines.has_comment[linenum]
            && clean_lines.lines[linenum].trim().is_empty())
        || line.trim().is_empty()
    {
        return;
    }

    for (key, token) in crate::cleanse::find_alternate_tokens(line) {
        linter.error(
            linenum,
            Category::ReadabilityAltTokens,
            2,
            &format!("Use operator {} instead of {}", token, key),
        );
    }
}

fn is_interior_block_comment_line(raw_line: &str) -> bool {
    let trimmed = raw_line.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with("//")
        && !trimmed.starts_with("/*")
        && !trimmed.starts_with('*')
        && !trimmed.contains("*/")
}

fn check_namespace_using(linter: &mut FileLinter, elided_line: &str, linenum: usize) {
    let trimmed = elided_line.trim();
    if !trimmed.starts_with("using namespace ") {
        return;
    }

    let category = if trimmed.starts_with("using namespace std::literals") {
        Category::BuildNamespacesLiterals
    } else {
        Category::BuildNamespaces
    };
    linter.error(
        linenum,
        category,
        5,
        "Do not use namespace using-directives.  Use using-declarations instead.",
    );
}

fn check_unnamed_namespace_in_header(linter: &mut FileLinter, elided_line: &str, linenum: usize) {
    if !linter.filename().ends_with(".h")
        && !linter.filename().ends_with(".hpp")
        && !linter.filename().ends_with(".hxx")
    {
        return;
    }

    let trimmed = elided_line.trim();
    if trimmed != "namespace {" && trimmed != "namespace {}" {
        return;
    }

    linter.error(
        linenum,
        Category::BuildNamespacesHeaders,
        4,
        "Do not use unnamed namespaces in header files.  See https://google-styleguide.googlecode.com/svn/trunk/cppguide.xml#Namespaces for more information.",
    );
}

fn check_check_macro(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
) {
    let Some((check_macro, open_paren_pos)) = find_check_macro(elided_line) else {
        return;
    };
    let Some((end_line, end_pos)) =
        line_utils::close_expression(clean_lines, linenum, open_paren_pos)
    else {
        return;
    };
    if end_line != linenum || end_pos <= open_paren_pos + 1 {
        return;
    }

    let expression = &elided_line[open_paren_pos + 1..end_pos - 1];
    if expression.contains("&&") || expression.contains("||") {
        return;
    }

    let Some((lhs, op, rhs)) = split_comparison_expression(expression) else {
        return;
    };
    let lhs = lhs.trim();
    let rhs = rhs.trim();
    if !(is_check_const(lhs) || is_check_const(rhs)) {
        return;
    }

    let Some(replacement) = replacement_check_macro(check_macro, op) else {
        return;
    };

    linter.error(
        linenum,
        Category::ReadabilityCheck,
        2,
        &format!(
            "Consider using {} instead of {}(a {} b)",
            replacement, check_macro, op
        ),
    );
}

fn find_check_macro(line: &str) -> Option<(&'static str, usize)> {
    CHECK_MACROS_AC.find_iter(line).find_map(|mat| {
        let check_macro = CHECK_MACROS[mat.pattern()];
        let start = mat.start();
        let suffix = &line[start + check_macro.len()..];
        let open_offset = suffix.find('(')?;
        suffix[..open_offset]
            .trim()
            .is_empty()
            .then_some((check_macro, start + check_macro.len() + open_offset))
    })
}

fn split_comparison_expression(expression: &str) -> Option<(&str, &'static str, &str)> {
    let mut depth = 0usize;
    let bytes = expression.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 {
            for op in ["==", "!=", ">=", "<=", ">", "<"] {
                if expression[i..].starts_with(op) {
                    return Some((&expression[..i], op, &expression[i + op.len()..]));
                }
            }
        }
        i += 1;
    }

    None
}

fn replacement_check_macro(check_macro: &str, op: &str) -> Option<&'static str> {
    match check_macro {
        "DCHECK" => match op {
            "==" => Some("DCHECK_EQ"),
            "!=" => Some("DCHECK_NE"),
            ">=" => Some("DCHECK_GE"),
            ">" => Some("DCHECK_GT"),
            "<=" => Some("DCHECK_LE"),
            "<" => Some("DCHECK_LT"),
            _ => None,
        },
        "CHECK" => match op {
            "==" => Some("CHECK_EQ"),
            "!=" => Some("CHECK_NE"),
            ">=" => Some("CHECK_GE"),
            ">" => Some("CHECK_GT"),
            "<=" => Some("CHECK_LE"),
            "<" => Some("CHECK_LT"),
            _ => None,
        },
        "EXPECT_TRUE" => match op {
            "==" => Some("EXPECT_EQ"),
            "!=" => Some("EXPECT_NE"),
            ">=" => Some("EXPECT_GE"),
            ">" => Some("EXPECT_GT"),
            "<=" => Some("EXPECT_LE"),
            "<" => Some("EXPECT_LT"),
            _ => None,
        },
        "ASSERT_TRUE" => match op {
            "==" => Some("ASSERT_EQ"),
            "!=" => Some("ASSERT_NE"),
            ">=" => Some("ASSERT_GE"),
            ">" => Some("ASSERT_GT"),
            "<=" => Some("ASSERT_LE"),
            "<" => Some("ASSERT_LT"),
            _ => None,
        },
        "EXPECT_FALSE" => match op {
            "==" => Some("EXPECT_NE"),
            "!=" => Some("EXPECT_EQ"),
            ">=" => Some("EXPECT_LT"),
            ">" => Some("EXPECT_LE"),
            "<=" => Some("EXPECT_GT"),
            "<" => Some("EXPECT_GE"),
            _ => None,
        },
        "ASSERT_FALSE" => match op {
            "==" => Some("ASSERT_NE"),
            "!=" => Some("ASSERT_EQ"),
            ">=" => Some("ASSERT_LT"),
            ">" => Some("ASSERT_LE"),
            "<=" => Some("ASSERT_GT"),
            "<" => Some("ASSERT_GE"),
            _ => None,
        },
        _ => None,
    }
}

fn check_function_size(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
) {
    if elided_line.trim() != "}" {
        return;
    }

    let Some(start_line) = linter.facts().matching_block_start(linenum) else {
        return;
    };

    let signature_line = collect_function_signature(clean_lines, start_line);
    if signature_line.is_empty() || is_control_statement_start(&signature_line) {
        return;
    }

    let Some(function_name) = parse_function_name(&signature_line) else {
        return;
    };

    let line_count = linter
        .facts()
        .non_blank_elided_lines_between(start_line, linenum);
    let limit = if is_test_like_function(function_name) {
        400
    } else {
        250
    };
    if line_count > limit {
        let display_name = if is_test_like_function(function_name) {
            Cow::Owned(format!("{}()", function_name))
        } else {
            Cow::Borrowed(function_name)
        };
        linter.error(
            start_line,
            Category::ReadabilityFnSize,
            0,
            &format!(
                "Small and focused functions are preferred: {} has {} non-blank lines (error triggered by exceeding {} lines).",
                display_name, line_count, limit
            ),
        );
    }
}

fn check_missing_function_body(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    linenum: usize,
) {
    if let Some(function_name) = find_function_without_body(clean_lines, linenum) {
        linter.error(
            linenum,
            Category::ReadabilityFnSize,
            5,
            &format!(
                "Lint failed to find start of function body for {}.",
                function_name.as_ref()
            ),
        );
    }
}

fn find_function_without_body<'a>(
    clean_lines: &'a CleansedLines,
    linenum: usize,
) -> Option<Cow<'a, str>> {
    let first_line = clean_lines.elided[linenum].trim();
    if first_line.is_empty()
        || !first_line.contains('(')
        || first_line.contains('{')
        || first_line.contains(';')
        || first_line.contains('}')
        || is_control_statement_start(first_line)
    {
        return None;
    }

    let mut signature = Cow::Borrowed(first_line);
    for idx in linenum + 1..clean_lines.elided.len() {
        let line = clean_lines.elided[idx].trim();
        if line.is_empty() {
            break;
        }
        if line.contains('{') || line.contains(';') || line.contains('}') {
            return None;
        }

        if let Cow::Borrowed(current) = &signature {
            let mut owned = String::with_capacity(current.len() + 1 + line.len());
            owned.push_str(current);
            owned.push(' ');
            owned.push_str(line);
            signature = Cow::Owned(owned);
        } else if let Cow::Owned(current) = &mut signature {
            current.push(' ');
            current.push_str(line);
        }
    }
    match signature {
        Cow::Borrowed(signature) => {
            let function_name = parse_function_name(signature)?;
            if is_test_like_function(function_name) {
                Some(Cow::Owned(format!("{}()", function_name)))
            } else {
                Some(Cow::Borrowed(function_name))
            }
        }
        Cow::Owned(signature) => {
            let function_name = parse_function_name(&signature)?;
            if is_test_like_function(function_name) {
                Some(Cow::Owned(format!("{}()", function_name)))
            } else {
                Some(Cow::Owned(function_name.to_owned()))
            }
        }
    }
}

fn collect_function_signature(clean_lines: &CleansedLines<'_>, start_line: usize) -> String {
    let mut parts = Vec::new();
    for idx in (0..=start_line).rev() {
        let line = clean_lines.elided[idx].trim();
        if line.is_empty() {
            break;
        }
        if line.ends_with(';') || line == "{" || line == "}" {
            break;
        }

        let before_brace = line.split('{').next().unwrap_or("").trim();
        if !before_brace.is_empty() {
            parts.push(before_brace);
        }

        if before_brace.contains('(') {
            break;
        }
    }
    if parts.is_empty() {
        return String::new();
    }

    let capacity =
        parts.iter().map(|part| part.len()).sum::<usize>() + parts.len().saturating_sub(1);
    let mut signature = String::with_capacity(capacity);
    for (index, part) in parts.iter().rev().enumerate() {
        if index > 0 {
            signature.push(' ');
        }
        signature.push_str(part);
    }
    signature
}

fn parse_function_name(signature_line: &str) -> Option<&str> {
    let captures = FUNCTION_NAME_RE.captures(signature_line)?;
    let function_name = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    if function_name.is_empty() {
        return None;
    }
    Some(function_name)
}

fn check_braces(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
) {
    // 1. Check for open brace at the start of a line
    if elided_line.trim() == "{" {
        if clean_lines.raw_lines[linenum].contains("*/ {") {
            return;
        }
        if let Some((_prev_idx, prev_line)) =
            line_utils::get_previous_non_blank_line(&clean_lines.elided, linenum)
        {
            // Exceptions: previous line ends with , ; : ( { } or starts with #
            let prev_trimmed = prev_line.trim();
            let last_byte = prev_trimmed.as_bytes().last();
            let safe_end =
                last_byte.is_some_and(|&b| matches!(b, b',' | b';' | b':' | b'(' | b'{' | b'}'));
            if !safe_end && !prev_trimmed.starts_with('#') {
                linter.error(
                    linenum,
                    Category::WhitespaceBraces,
                    4,
                    r#"{ should almost always be at the end of the previous line"#,
                );
            }
        }
    }

    // 2. Check for "else" placement
    if let Some(else_pos) = elided_line.find("else") {
        let _bytes = elided_line.as_bytes();

        // Check if it's a word match for "else"
        if !string_utils::is_word_match(elided_line, else_pos, else_pos + 4) {
            return;
        }

        let mut last_wrong = false;

        // Pattern 0: ^\s*else\b\s*(?:if\b|\{|$)
        let prefix = &elided_line[..else_pos];
        if prefix.trim().is_empty() {
            let suffix = elided_line[else_pos + 4..].trim_start();
            if (suffix.is_empty() || suffix.starts_with('{') || suffix.starts_with("if"))
                && let Some((_prev_idx, prev_line)) =
                    line_utils::get_previous_non_blank_line(&clean_lines.elided, linenum)
                && prev_line.trim() == "}"
            {
                linter.error(
                    linenum,
                    Category::WhitespaceNewline,
                    4,
                    "An else should appear on the same line as the preceding }",
                );
                last_wrong = true;
            }
        }

        // Pattern 1 & 2: else if / } else if
        if let Some(if_pos) = elided_line[else_pos + 4..].find("if") {
            let if_pos = else_pos + 4 + if_pos;
            if string_utils::is_word_match(elided_line, if_pos, if_pos + 2) {
                // Check for BRACED_ELSE_IF: } \s* else if
                let braced_else_if = prefix.trim() == "}";

                if let Some(open_paren_offset) = elided_line[if_pos + 2..].find('(') {
                    let open_paren_pos = if_pos + 2 + open_paren_offset;
                    if let Some((end_line, end_pos)) =
                        line_utils::close_expression(clean_lines, linenum, open_paren_pos)
                    {
                        let endline = &clean_lines.elided[end_line];
                        let brace_on_right = endline
                            .get(end_pos..)
                            .is_some_and(|suffix| suffix.contains('{'));
                        if braced_else_if != brace_on_right {
                            linter.error(
                                linenum,
                                Category::ReadabilityBraces,
                                5,
                                "If an else has a brace on one side, it should have it on both",
                            );
                        }
                    }
                }
                return;
            }
        }

        // Pattern 3 & 4: } \s* else [^{]*$ / ^[^}]* else \s* {
        let has_left_brace = prefix.trim() == "}";
        let suffix = elided_line[else_pos + 4..].trim_start();
        let has_right_brace = suffix.starts_with('{') && !last_wrong;

        if (has_left_brace || has_right_brace) && has_left_brace != has_right_brace {
            linter.error(
                linenum,
                Category::ReadabilityBraces,
                5,
                "If an else has a brace on one side, it should have it on both",
            );
        }
    }
}

fn count_unescaped_quotes(line: &str) -> usize {
    let mut count = 0;
    let mut escaped = false;
    for c in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == '"' {
            count += 1;
        }
    }
    count
}

fn check_multiline_strings(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    linenum: usize,
) {
    let line = clean_lines.line_without_alternate_tokens(linenum);
    if line.contains('"') && count_unescaped_quotes(line) % 2 == 1 {
        linter.error(
            linenum,
            Category::ReadabilityMultilineString,
            5,
            "Multi-line string (\"...\") found.  This lint script doesn't do well with such strings, and may give bogus warnings.  Use C++11 raw strings or concatenation instead.",
        );
    }
}

fn check_multiline_comments(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    linenum: usize,
) {
    let line = &clean_lines.lines_without_raw_strings[linenum];
    if !line.contains("/*") {
        return;
    }

    let line_without_cpp_comments = line.split("//").next().unwrap_or("");
    if !line_without_cpp_comments.contains("/*") {
        return;
    }

    let line = collapse_strings(line_without_cpp_comments);
    if !line.contains("/*") {
        return;
    }
    if line.matches("/*").count() <= line.matches("*/").count() {
        return;
    }

    let has_end = clean_lines
        .raw_lines
        .iter()
        .skip(linenum)
        .any(|line| line.contains("*/"));

    if has_end {
        linter.error(
            linenum,
            Category::ReadabilityMultilineComment,
            5,
            "Complex multi-line /*...*/-style comment found. Lint may give bogus warnings.  Consider replacing these with //-style comments, with #if 0...#endif, or with more clearly structured multi-line comments.",
        );
    } else {
        linter.error(
            linenum,
            Category::ReadabilityMultilineComment,
            5,
            "Could not find end of multi-line comment",
        );
    }
}

fn check_empty_bodies(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
) {
    if elided_line.is_empty() {
        return;
    }

    let trimmed_start = elided_line.trim_start();
    if trimmed_start.is_empty() {
        return;
    }

    // if/while/for empty bodies: if (cond) ;
    if (trimmed_start.starts_with("if")
        || trimmed_start.starts_with("while")
        || trimmed_start.starts_with("for"))
        && trimmed_start.contains('(')
    {
        if is_empty_control_statement(trimmed_start, "if") {
            linter.error(
                linenum,
                Category::WhitespaceEmptyConditionalBody,
                5,
                "Empty conditional bodies should use {}",
            );
            return;
        }

        if is_empty_control_statement(trimmed_start, "while")
            || is_empty_control_statement(trimmed_start, "for")
        {
            linter.error(
                linenum,
                Category::WhitespaceEmptyLoopBody,
                5,
                "Empty loop bodies should use {} or continue",
            );
            return;
        }
    }

    if unmatched_parentheses(elided_line) > 0 {
        return;
    }

    let trimmed = trimmed_start.trim_end();
    if trimmed != "}" || linenum == 0 {
        return;
    }

    let Some((opening_idx, opening_line)) =
        line_utils::get_previous_non_blank_line(&clean_lines.elided, linenum)
    else {
        return;
    };

    if !opening_line.trim_end().ends_with('{') {
        return;
    }

    let mut if_idx = opening_idx;
    let mut found_if = string_utils::contains_word(clean_lines.elided[if_idx], "if");
    while !found_if && if_idx > 0 {
        if_idx -= 1;
        if clean_lines.elided[if_idx].trim().is_empty() {
            continue;
        }
        if clean_lines.elided[if_idx].contains('}') {
            return;
        }
        found_if = string_utils::contains_word(clean_lines.elided[if_idx], "if");
        if clean_lines.elided[if_idx].contains(';') && !found_if {
            return;
        }
    }

    if !found_if {
        return;
    }

    let has_comment_only_body = clean_lines
        .raw_lines
        .iter()
        .zip(clean_lines.elided.iter())
        .skip(opening_idx + 1)
        .take(linenum.saturating_sub(opening_idx + 1))
        .any(|(raw, elided)| !raw.trim().is_empty() && elided.trim().is_empty());
    if has_comment_only_body {
        return;
    }

    let has_else_clause = clean_lines
        .elided
        .iter()
        .skip(linenum + 1)
        .find(|line| !line.trim().is_empty())
        .map(|line| string_utils::trimmed_starts_with_word(line, "else"))
        .unwrap_or(false);
    if has_else_clause {
        return;
    }

    linter.error(
        opening_idx,
        Category::WhitespaceEmptyIfBody,
        4,
        "If statement had no body and no else clause",
    );
}

fn unmatched_parentheses(line: &str) -> i32 {
    let mut depth = 0;
    for ch in line.chars() {
        match ch {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            _ => {}
        }
    }
    depth
}

fn is_empty_control_statement(line: &str, keyword: &str) -> bool {
    let Some(rest) = line.strip_prefix(keyword) else {
        return false;
    };
    let rest = rest.trim_start();
    let Some(rest) = rest.strip_prefix('(') else {
        return false;
    };

    let mut depth = 1;
    let mut split_index = None;
    for (idx, ch) in rest.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    split_index = Some(idx + ch.len_utf8());
                    break;
                }
            }
            _ => {}
        }
    }

    let Some(split_index) = split_index else {
        return false;
    };
    rest[split_index..].trim() == ";"
}

fn check_redundant_virtuals(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
) {
    let trimmed = elided_line.trim();
    let mut keyword_flags = 0u8;
    for mat in INHERITANCE_KEYWORDS_AC.find_iter(trimmed) {
        if !string_utils::is_word_match(trimmed, mat.start(), mat.end()) {
            continue;
        }
        keyword_flags |= 1 << mat.pattern().as_usize();
        if keyword_flags == 0b111 {
            break;
        }
    }
    let has_virtual = keyword_flags & 0b001 != 0;
    let has_override = keyword_flags & 0b010 != 0;
    let has_final = keyword_flags & 0b100 != 0;

    if has_virtual && (has_override || has_final) {
        linter.error(
            linenum,
            Category::ReadabilityInheritance,
            4,
            "virtual is redundant since override/final already implies a virtual function",
        );
        return;
    }

    if has_override && has_final {
        linter.error(
            linenum,
            Category::ReadabilityInheritance,
            4,
            "override is redundant when final is present",
        );
        return;
    }

    if !(trimmed == "override;"
        || trimmed == "final;"
        || trimmed == "override final;"
        || trimmed == "final override;")
    {
        return;
    }

    let Some((_prev_idx, prev_line)) =
        line_utils::get_previous_non_blank_line(&clean_lines.elided, linenum)
    else {
        return;
    };
    let prev_has_virtual = INHERITANCE_KEYWORDS_AC.find_iter(prev_line).any(|mat| {
        mat.pattern().as_usize() == 0
            && string_utils::is_word_match(prev_line, mat.start(), mat.end())
    });
    if prev_has_virtual {
        linter.error(
            linenum,
            Category::ReadabilityInheritance,
            4,
            "virtual is redundant since override/final already implies a virtual function",
        );
    }
}

fn check_single_line_control_bodies(
    linter: &mut FileLinter,
    _clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
) {
    if elided_line.trim().is_empty() {
        return;
    }

    // Manual implementation of SINGLE_LINE_CONTROL_SET patterns:
    // r#"\b(if|else|while|for)\b.*\s*\{[^{}]+\}\s*$"#
    for kw in ["if", "else", "while", "for"] {
        if let Some(pos) = elided_line.find(kw) {
            if !string_utils::is_word_match(elided_line, pos, pos + kw.len()) {
                continue;
            }

            let rest = &elided_line[pos + kw.len()..];
            if let Some(open_brace) = rest.find('{') {
                let after_brace = &rest[open_brace + 1..];
                if let Some(close_brace) = after_brace.find('}') {
                    let inside = &after_brace[..close_brace];
                    let after_close = &after_brace[close_brace + 1..];

                    if !inside.trim().is_empty()
                        && !inside.contains('{')
                        && !inside.contains('}')
                        && after_close.trim().is_empty()
                    {
                        linter.error(
                            linenum,
                            Category::WhitespaceNewline,
                            5,
                            &format!(
                                "Controlled statements inside brackets of {} clause should be on a separate line",
                                kw
                            ),
                        );
                        return;
                    }
                }
            }
        }
    }
}

fn check_multiline_if_else_bodies(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
) {
    if elided_line.trim_start().starts_with('#') {
        return;
    }

    let mut if_else_match = None;
    for mat in IF_ELSE_AC.find_iter(elided_line) {
        if string_utils::is_word_match(elided_line, mat.start(), mat.end()) {
            let keyword = match mat.pattern().as_usize() {
                0 => "if",
                _ => "else",
            };

            if keyword == "else" {
                if_else_match = Some((mat.start(), mat.end(), false));
                break;
            } else {
                // Check for "if ... ("
                let rest = &elided_line[mat.end()..];
                let trimmed_rest = rest.trim_start();
                if trimmed_rest.starts_with('(') {
                    let open_paren_pos = mat.end() + rest.len() - trimmed_rest.len();
                    if_else_match = Some((mat.start(), open_paren_pos + 1, true));
                    break;
                } else if let Some(rest_after_constexpr) = trimmed_rest.strip_prefix("constexpr") {
                    let rest_after_constexpr = rest_after_constexpr.trim_start();
                    if rest_after_constexpr.starts_with('(') {
                        let open_paren_pos = elided_line.len() - rest_after_constexpr.len();
                        if_else_match = Some((mat.start(), open_paren_pos + 1, true));
                        break;
                    }
                }
            }
        }
    }

    let Some((_match_start, match_end, is_if_match)) = if_else_match else {
        return;
    };

    let if_indent = line_utils::get_indent_level(elided_line);
    let mut endlinenum = linenum;
    let mut endpos = match_end;
    if is_if_match {
        let open_paren_pos = match_end.saturating_sub(1);
        let Some((matched_line, matched_pos)) =
            line_utils::close_expression(clean_lines, linenum, open_paren_pos)
        else {
            return;
        };
        endlinenum = matched_line;
        endpos = matched_pos;
    }

    let endline = &clean_lines.elided[endlinenum];
    let endline_sub = endline.get(endpos..).unwrap_or("");
    let opens_brace = MULTILINE_IF_OPEN_BRACE_RE.is_match(endline_sub)
        || (endline_sub.trim().is_empty()
            && endlinenum + 1 < clean_lines.elided.len()
            && clean_lines.elided[endlinenum + 1]
                .trim_start()
                .starts_with('{'));
    if opens_brace {
        return;
    }

    let mut scan_line = endlinenum;
    let mut scan_pos = endpos;
    let mut semicolon = None;
    while scan_line < clean_lines.elided.len() {
        let line = &clean_lines.elided[scan_line];
        let start = scan_pos.min(line.len());
        if let Some(found) = line[start..].find(';') {
            semicolon = Some((scan_line, start + found));
            break;
        }
        scan_line += 1;
        scan_pos = 0;
    }

    let Some((semicolon_line, semicolon_pos)) = semicolon else {
        return;
    };
    let statement_line = &clean_lines.elided[semicolon_line];
    let statement_tail = statement_line
        .find(';')
        .and_then(|first_semicolon_pos| statement_line.get(first_semicolon_pos..))
        .unwrap_or_else(|| statement_line.get(semicolon_pos..).unwrap_or(""));
    if !MULTILINE_IF_MULTI_COMMAND_RE.is_match(statement_tail) {
        if !MULTILINE_IF_LAMBDA_RE.is_match(statement_line) {
            linter.error(
                linenum,
                Category::ReadabilityBraces,
                4,
                "If/else bodies with multiple statements require braces",
            );
        }
        return;
    }

    if semicolon_line + 1 < clean_lines.elided.len() {
        let next_line = &clean_lines.elided[semicolon_line + 1];
        let next_indent = line_utils::get_indent_level(next_line);
        if is_if_match
            && string_utils::trimmed_starts_with_word(next_line, "else")
            && next_indent != if_indent
        {
            linter.error(
                linenum,
                Category::ReadabilityBraces,
                4,
                "Else clause should be indented at the same level as if. Ambiguous nested if/else chains require braces.",
            );
        } else if next_indent > if_indent {
            linter.error(
                linenum,
                Category::ReadabilityBraces,
                4,
                "If/else bodies with multiple statements require braces",
            );
        }
    }
}

fn check_namespace_termination_comment(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    linenum: usize,
) {
    let raw_line = &clean_lines.raw_lines[linenum];
    let Some(start_line) = linter.facts().matching_block_start(linenum) else {
        return;
    };
    let Some(namespace_line) =
        crate::line_utils::namespace_decl_start_line(&clean_lines.elided, start_line)
    else {
        return;
    };
    let start = clean_lines.elided[namespace_line].trim();
    let Some(captures) = NAMESPACE_START_RE.captures(start) else {
        return;
    };
    if linenum.saturating_sub(namespace_line) < 10 && !NAMESPACE_TERMINATION_RE.is_match(raw_line) {
        return;
    }

    let name = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    if name.is_empty() {
        if ANONYMOUS_NAMESPACE_TERMINATION_RE.is_match(raw_line) {
            return;
        }
        let message = if ANONYMOUS_NAMESPACE_TERM_MSG_RE.is_match(raw_line) {
            "Anonymous namespace should be terminated with \"// namespace\" or \"// anonymous namespace\""
        } else {
            "Anonymous namespace should be terminated with \"// namespace\""
        };
        linter.error(linenum, Category::ReadabilityNamespace, 5, message);
        return;
    }
    let pattern = format!(
        r#"^\s*}};*\s*(//|/\*).*\bnamespace\s+{}[\*/\.\\\s]*$"#,
        regex::escape(name)
    );
    if regex_utils::regex_search(&pattern, raw_line) {
        return;
    }
    linter.error(
        linenum,
        Category::ReadabilityNamespace,
        5,
        &format!(
            "Namespace should be terminated with \"// namespace {}\"",
            name
        ),
    );
}

fn check_namespace_indentation(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
) {
    if elided_line.is_empty() || !elided_line.starts_with(|c: char| c.is_ascii_whitespace()) {
        return;
    }

    if linter.facts().namespace_top_level_depth(linenum).is_none() {
        return;
    }

    if is_macro_definition(clean_lines, elided_line, linenum) {
        return;
    }

    if is_namespace_closing_brace(linter, clean_lines, linenum) {
        return;
    }

    let class_range = linter.facts().enclosing_class_range(linenum);
    if let Some(range) = class_range
        && linenum != range.end
    {
        return;
    }

    let non_ns_before = linter.facts().non_namespace_indent_depth_before(linenum);
    let non_ns = linter.facts().non_namespace_indent_depth(linenum);
    let has_close = elided_line.contains('}');

    if non_ns_before == 0 || (has_close && non_ns == 0) {
        linter.error(
            linenum,
            Category::WhitespaceIndentNamespace,
            4,
            "Do not indent within a namespace.",
        );
    }
}

fn is_macro_definition(clean_lines: &CleansedLines<'_>, elided_line: &str, linenum: usize) -> bool {
    elided_line.starts_with("#define")
        || (linenum > 0 && clean_lines.elided[linenum - 1].ends_with('\\'))
}

fn is_namespace_closing_brace(
    linter: &FileLinter,
    clean_lines: &CleansedLines<'_>,
    linenum: usize,
) -> bool {
    clean_lines.elided[linenum].contains('}')
        && linter
            .facts()
            .matching_block_start(linenum)
            .and_then(|start| {
                if linter.facts().block_kind(start) == Some(crate::facts::ScopeKind::Namespace) {
                    linter.facts().namespace_decl_line(start)
                } else {
                    None
                }
            })
            .is_some_and(|namespace_line| {
                linter
                    .facts()
                    .namespace_top_level_depth(namespace_line)
                    .is_none()
            })
}

fn is_assign_match(line: &str) -> bool {
    let trimmed = line.trim_end();
    if !trimmed.ends_with('=') {
        return false;
    }
    let before = &trimmed[..trimmed.len() - 1];
    !before.is_empty()
        && before
            .chars()
            .next_back()
            .is_some_and(|c| c.is_whitespace())
}

fn is_alignas_match(line: &str) -> bool {
    let trimmed = line.trim_end();
    if !trimmed.ends_with("alignas") {
        return false;
    }
    let before = trimmed[..trimmed.len() - 7].trim_end();
    string_utils::ends_with_word(before, "struct") || string_utils::ends_with_word(before, "union")
}

fn get_trailing_macro(s: &str) -> Option<&str> {
    let trimmed = s.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    let mut i = trimmed.len();
    while i > 0 && string_utils::is_word_char(trimmed.as_bytes()[i - 1]) {
        i -= 1;
    }
    let word = &trimmed[i..];
    if !word.is_empty()
        && (word.as_bytes()[0].is_ascii_uppercase() || word.as_bytes()[0] == b'_')
        && word
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        Some(word)
    } else {
        None
    }
}

fn is_operator_index_match(s: &str) -> bool {
    let trimmed = s.trim_end();
    if !trimmed.ends_with(']') {
        return false;
    }
    let mut i = trimmed.len() - 1;
    while i > 0 && trimmed.as_bytes()[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    if i == 0 || trimmed.as_bytes()[i - 1] != b'[' {
        return false;
    }
    i -= 1;
    while i > 0 && trimmed.as_bytes()[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    string_utils::ends_with_word(&trimmed[..i], "operator")
}

fn check_trailing_semicolon(
    linter: &mut FileLinter,
    clean_lines: &CleansedLines<'_>,
    elided_line: &str,
    linenum: usize,
) {
    let Some(brace_pos) = elided_line.find('{') else {
        return;
    };

    let prefix = elided_line[..brace_pos].trim_end();

    let match_start = if prefix.ends_with(')') {
        let close_paren_pos = prefix.rfind(')').unwrap();

        let skip = line_utils::reverse_close_expression(clean_lines, linenum, close_paren_pos)
            .is_some_and(|(open_line, open_pos)| {
                let line_prefix = &clean_lines.elided[open_line][..open_pos];
                let macro_name = get_trailing_macro(line_prefix);
                let unsafe_macro = macro_name.is_some_and(|name| {
                    !matches!(
                        name,
                        "TEST"
                            | "TEST_F"
                            | "MATCHER"
                            | "MATCHER_P"
                            | "TYPED_TEST"
                            | "EXCLUSIVE_LOCKS_REQUIRED"
                            | "SHARED_LOCKS_REQUIRED"
                            | "LOCKS_EXCLUDED"
                            | "INTERFACE_DEF"
                    )
                });
                let lambda_capture =
                    line_prefix.trim_end().ends_with(']') && !is_operator_index_match(line_prefix);
                unsafe_macro
                    || lambda_capture
                    || is_alignas_match(line_prefix)
                    || string_utils::ends_with_word(line_prefix, "decltype")
                    || string_utils::contains_word_start(line_prefix, "requires")
                    || is_assign_match(line_prefix)
                    || (open_line > 0
                        && string_utils::get_last_non_space(clean_lines.elided[open_line - 1])
                            == ']')
            });
        if skip {
            None
        } else {
            Some((linenum, brace_pos))
        }
    } else if string_utils::ends_with_word(prefix, "else")
        || (prefix.ends_with("const") && prefix[..prefix.len() - 5].trim_end().ends_with(')'))
        || (prefix.is_empty()
            && line_utils::get_previous_non_blank_line(&clean_lines.elided, linenum).is_some_and(
                |(_idx, prev_line)| {
                    let last_char = string_utils::get_last_non_space(prev_line);
                    last_char == ';' || last_char == '{' || last_char == '}'
                },
            ))
    {
        Some((linenum, brace_pos))
    } else {
        None
    };

    let Some((start_line, start_pos)) = match_start else {
        return;
    };
    let Some((end_line, end_pos)) =
        line_utils::close_expression(clean_lines, start_line, start_pos)
    else {
        return;
    };
    let has_trailing_semicolon = clean_lines.elided[end_line]
        .get(end_pos..)
        .and_then(|suffix| suffix.chars().find(|ch| !ch.is_whitespace()))
        .is_some_and(|ch| ch == ';');
    if has_trailing_semicolon {
        linter.error(
            end_line,
            Category::ReadabilityBraces,
            4,
            "You don't need a ; after a }",
        );
    }
}
