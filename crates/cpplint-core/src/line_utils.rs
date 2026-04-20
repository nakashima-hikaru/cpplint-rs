use crate::cleanse::CleansedLines;
use regex::RegexBuilder;
use std::simd::cmp::SimdPartialEq;
use std::simd::u8x32;

pub fn get_indent_level(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i + 32 <= bytes.len() {
        let chunk = u8x32::from_slice(&bytes[i..i + 32]);
        let mask = chunk.simd_eq(u8x32::splat(b' ')).to_bitmask();
        let ones = mask.trailing_ones() as usize;
        count += ones;
        if ones < 32 {
            return count;
        }
        i += 32;
    }
    for &b in &bytes[i..] {
        if b == b' ' {
            count += 1;
        } else {
            break;
        }
    }
    count
}

#[inline]
pub fn is_blank_line(line: &str) -> bool {
    let bytes = line.as_bytes();
    if bytes.is_empty() {
        return true;
    }
    // Fast path: most non-blank lines start with a non-whitespace character
    if !bytes[0].is_ascii_whitespace() {
        return false;
    }
    bytes.iter().all(|b| b.is_ascii_whitespace())
}

pub fn get_previous_non_blank_line<S: AsRef<str>>(
    lines: &[S],
    linenum: usize,
) -> Option<(usize, &str)> {
    if linenum == 0 {
        return None;
    }
    for i in (0..linenum).rev() {
        let line = lines[i].as_ref();
        if !is_blank_line(line) {
            return Some((i, line));
        }
    }
    None
}

pub fn namespace_decl_start_line<S: AsRef<str>>(lines: &[S], start: usize) -> Option<usize> {
    let trimmed = lines.get(start)?.as_ref().trim();
    if is_namespace_decl(trimmed) {
        return Some(start);
    }
    if trimmed != "{" {
        return None;
    }

    let (prev, prev_line) = get_previous_non_blank_line(lines, start)?;
    let prev_trimmed = prev_line.trim();
    if is_namespace_decl(prev_trimmed) {
        return Some(prev);
    }
    if !is_namespace_name_continuation(prev_trimmed) {
        return None;
    }

    get_previous_non_blank_line(lines, prev).and_then(|(namespace_line, namespace_decl)| {
        is_namespace_decl(namespace_decl.trim()).then_some(namespace_line)
    })
}

/// Returns the text inside the first balanced pair matched by `start_pattern`.
///
/// The start pattern is searched with multiline semantics, mirroring the
/// upstream helper used by `cpplint.py` tests.
pub fn get_text_inside(text: &str, start_pattern: &str) -> Option<String> {
    let regex = RegexBuilder::new(start_pattern)
        .multi_line(true)
        .build()
        .ok()?;
    let mat = regex.find(text)?;
    let open = text[..mat.end()].chars().next_back()?;
    let close = match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        '<' => '>',
        _ => return None,
    };

    let mut stack = vec![close];
    let start = mat.end();
    for (offset, ch) in text[start..].char_indices() {
        match ch {
            '(' => stack.push(')'),
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            '<' => stack.push('>'),
            ')' | ']' | '}' | '>' => {
                let expected = stack.pop()?;
                if ch != expected {
                    return None;
                }
                if stack.is_empty() {
                    return Some(text[start..start + offset].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn is_namespace_decl(s: &str) -> bool {
    let trimmed = s.trim_start();
    if let Some(rest) = trimmed.strip_prefix("namespace") {
        match rest.as_bytes().first() {
            None => true,
            Some(&c) => !c.is_ascii_alphanumeric() && c != b'_',
        }
    } else {
        false
    }
}

fn is_namespace_name_continuation(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let bytes = s.as_bytes();
    if !bytes[0].is_ascii_alphabetic() && bytes[0] != b'_' {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|&c| c.is_ascii_alphanumeric() || c == b'_' || c == b':')
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn close_expression(
    clean_lines: &CleansedLines,
    linenum: usize,
    pos: usize,
) -> Option<(usize, usize)> {
    close_expression_in_lines(&clean_lines.elided, linenum, pos)
}

pub fn close_expression_in_lines<S: AsRef<str>>(
    lines: &[S],
    mut linenum: usize,
    pos: usize,
) -> Option<(usize, usize)> {
    let line = lines.get(linenum)?.as_ref();
    let bytes = line.as_bytes();
    let start = *bytes.get(pos)?;
    let next = bytes.get(pos + 1).copied();
    if !matches!(start, b'(' | b'{' | b'[' | b'<')
        || (start == b'<' && matches!(next, Some(b'<' | b'=')))
    {
        return None;
    }

    let mut stack = Vec::new();
    if let Some(end_pos) = find_end_of_expression_in_line(line, pos, &mut stack) {
        return Some((linenum, end_pos));
    }

    while !stack.is_empty() && linenum + 1 < lines.len() {
        linenum += 1;
        let line = lines[linenum].as_ref();
        if let Some(end_pos) = find_end_of_expression_in_line(line, 0, &mut stack) {
            return Some((linenum, end_pos));
        }
    }

    None
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn reverse_close_expression(
    clean_lines: &CleansedLines,
    mut linenum: usize,
    pos: usize,
) -> Option<(usize, usize)> {
    let line = clean_lines.elided.get(linenum)?;
    let line_str: &str = line;
    if !matches!(
        line_str.as_bytes().get(pos),
        Some(b')' | b'}' | b']' | b'>')
    ) {
        return None;
    }

    let mut stack = Vec::new();
    if let Some(start_pos) = find_start_of_expression_in_line(line_str, pos, &mut stack) {
        return Some((linenum, start_pos));
    }

    while !stack.is_empty() && linenum > 0 {
        linenum -= 1;
        let line = clean_lines.elided[linenum];
        if let Some(start_pos) =
            find_start_of_expression_in_line(line, line.len().saturating_sub(1), &mut stack)
        {
            return Some((linenum, start_pos));
        }
    }

    None
}

fn find_end_of_expression_in_line(
    line: &str,
    startpos: usize,
    stack: &mut Vec<char>,
) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = startpos;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => stack.push(bytes[i] as char),
            b'<' => {
                if i > 0 && bytes[i - 1] == b'<' {
                    if stack.last() == Some(&'<') {
                        stack.pop();
                        if stack.is_empty() {
                            return None;
                        }
                    }
                } else if i > 0 && trailing_operator_match(&line[..i]) {
                    i += 1;
                    continue;
                } else {
                    stack.push('<');
                }
            }
            b')' | b']' | b'}' => {
                while stack.last() == Some(&'<') {
                    stack.pop();
                }
                if stack.is_empty() {
                    return None;
                }
                let expected = match bytes[i] {
                    b')' => '(',
                    b']' => '[',
                    _ => '{',
                };
                if stack.last() == Some(&expected) {
                    stack.pop();
                    if stack.is_empty() {
                        return Some(i + 1);
                    }
                } else {
                    stack.clear();
                    return None;
                }
            }
            b'>' => {
                if i > 0 && (bytes[i - 1] == b'-' || trailing_operator_match(&line[..i])) {
                    i += 1;
                    continue;
                }
                if stack.last() == Some(&'<') {
                    stack.pop();
                    if stack.is_empty() {
                        return Some(i + 1);
                    }
                }
            }
            b';' => {
                while stack.last() == Some(&'<') {
                    stack.pop();
                }
                if stack.is_empty() {
                    return None;
                }
            }
            _ => {}
        }
        i += 1;
    }

    None
}

fn find_start_of_expression_in_line(
    line: &str,
    endpos: usize,
    stack: &mut Vec<char>,
) -> Option<usize> {
    if line.is_empty() {
        return None;
    }

    let bytes = line.as_bytes();
    let mut i = endpos.min(bytes.len() - 1);
    loop {
        match bytes[i] {
            b')' | b']' | b'}' => stack.push(bytes[i] as char),
            b'>' => {
                if i > 0
                    && (bytes[i - 1] == b'-'
                        || (bytes[i - 1] == b' ' && bytes.get(i + 1) == Some(&b'='))
                        || trailing_operator_match(&line[..i]))
                {
                    if i == 0 {
                        break;
                    }
                    i -= 1;
                    continue;
                }
                stack.push('>');
            }
            b'<' => {
                if i > 0 && bytes[i - 1] == b'<' {
                    i = i.saturating_sub(1);
                    continue;
                }
                if stack.last() == Some(&'>') {
                    stack.pop();
                    if stack.is_empty() {
                        return Some(i);
                    }
                }
            }
            b'(' | b'[' | b'{' => {
                while stack.last() == Some(&'>') {
                    stack.pop();
                }
                if stack.is_empty() {
                    return None;
                }
                let expected = match bytes[i] {
                    b'(' => ')',
                    b'[' => ']',
                    _ => '}',
                };
                if stack.last() == Some(&expected) {
                    stack.pop();
                    if stack.is_empty() {
                        return Some(i);
                    }
                } else {
                    stack.clear();
                    return None;
                }
            }
            b';' => {
                while stack.last() == Some(&'>') {
                    stack.pop();
                }
                if stack.is_empty() {
                    return None;
                }
            }
            _ => {}
        }

        if i == 0 {
            break;
        }
        i -= 1;
    }

    None
}

fn trailing_operator_match(prefix: &str) -> bool {
    let trimmed = prefix.trim_end();
    if let Some(op_start) = trimmed.strip_suffix("operator") {
        match op_start.as_bytes().last() {
            None => true,
            Some(&c) => !c.is_ascii_alphanumeric() && c != b'_',
        }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    fn upstream_close_expression_lines() -> [&'static str; 22] {
        [
            "// Line 0",
            "inline RCULocked<X>::ReadPtr::ReadPtr(const RCULocked* rcu) {",
            "  DCHECK(!(data & kFlagMask)) << \"Error\";",
            "}",
            "// Line 4",
            "RCULocked<X>::WritePtr::WritePtr(RCULocked* rcu)",
            "    : lock_(&rcu_->mutex_) {",
            "}",
            "// Line 8",
            "template <typename T, typename... A>",
            "typename std::enable_if<",
            "    std::is_array<T>::value && (std::extent<T>::value > 0)>::type",
            "MakeUnique(A&&... a) = delete;",
            "// Line 13",
            "auto x = []() {};",
            "// Line 15",
            "template <typename U>",
            "friend bool operator==(const reffed_ptr& a,",
            "                       const reffed_ptr<U>& b) {",
            "  return a.get() == b.get();",
            "}",
            "// Line 21",
        ]
    }

    #[test]
    fn close_expression_handles_multiline_templates() {
        let arena = Bump::new();
        let lines = [
            "return BuiltInDefaultValueGetter<",
            "    T, ::std::is_default_constructible<T>::value>::Get();",
        ];
        let clean_lines = CleansedLines::new(&arena, &lines);

        assert_eq!(close_expression(&clean_lines, 0, 32), Some((1, 49)));
        assert_eq!(reverse_close_expression(&clean_lines, 1, 48), Some((0, 32)));
    }

    #[test]
    fn line_helpers_cover_blank_indent_and_namespace_detection() {
        assert_eq!(get_indent_level(""), 0);
        let indented = format!("{}x", " ".repeat(33));
        assert_eq!(get_indent_level(&indented), 33);

        assert!(is_blank_line(""));
        assert!(is_blank_line(" \t"));
        assert!(!is_blank_line("x"));

        let lines = ["alpha", "", "beta", "   "];
        assert_eq!(get_previous_non_blank_line(&lines, 2), Some((0, "alpha")));
        assert_eq!(get_previous_non_blank_line(&lines, 1), Some((0, "alpha")));
        assert_eq!(get_previous_non_blank_line(&lines, 0), None);

        let namespace_lines = ["namespace foo", "{", "int x;", "}"];
        assert_eq!(namespace_decl_start_line(&namespace_lines, 0), Some(0));
        assert_eq!(namespace_decl_start_line(&namespace_lines, 1), Some(0));
        assert_eq!(namespace_decl_start_line(&["class Foo", "{"], 1), None);
    }

    #[test]
    fn test_is_blank_line() {
        assert!(is_blank_line(""));
        assert!(is_blank_line(" "));
        assert!(is_blank_line(" \t\r\n"));
        assert!(!is_blank_line("int a;"));
        assert!(!is_blank_line("{"));
    }

    #[test]
    fn expression_helpers_cover_multiline_and_failure_paths() {
        let arena = Bump::new();
        let lines = ["call(foo)", "std::vector<", "int> value;", "x <= y"];
        let clean_lines = CleansedLines::new(&arena, &lines);

        assert_eq!(close_expression(&clean_lines, 0, 4), Some((0, 9)));
        assert_eq!(reverse_close_expression(&clean_lines, 0, 8), Some((0, 4)));
        assert_eq!(close_expression(&clean_lines, 1, 11), Some((2, 4)));
        assert_eq!(reverse_close_expression(&clean_lines, 2, 3), Some((1, 11)));
        assert_eq!(close_expression_in_lines(&lines, 3, 2), None);
        assert_eq!(reverse_close_expression(&clean_lines, 3, 2), None);
        assert_eq!(close_expression_in_lines(&lines, 0, 2), None);
    }

    #[test]
    fn test_close_expression() {
        let arena = Bump::new();
        let lines = upstream_close_expression_lines();
        let clean_lines = CleansedLines::new(&arena, &lines);

        let positions = [
            ((1, 16), Some((1, 19))),
            ((1, 37), Some((1, 59))),
            ((1, 60), Some((3, 1))),
            ((2, 8), Some((2, 29))),
            ((2, 30), None),
            ((9, 9), Some((9, 36))),
            ((10, 23), Some((11, 59))),
            ((11, 54), None),
            ((14, 9), Some((14, 11))),
            ((14, 11), Some((14, 13))),
            ((14, 14), Some((14, 16))),
            ((17, 22), Some((18, 46))),
            ((18, 47), Some((20, 1))),
        ];

        for &((line, pos), expected) in &positions {
            assert_eq!(close_expression(&clean_lines, line, pos), expected);
        }
    }

    #[test]
    fn test_reverse_close_expression() {
        let arena = Bump::new();
        let lines = upstream_close_expression_lines();
        let clean_lines = CleansedLines::new(&arena, &lines);

        let positions = [
            ((1, 18), Some((1, 16))),
            ((1, 58), Some((1, 37))),
            ((2, 27), Some((2, 10))),
            ((2, 28), Some((2, 8))),
            ((6, 18), None),
            ((9, 35), Some((9, 9))),
            ((11, 54), None),
            ((11, 57), Some((11, 31))),
            ((14, 10), Some((14, 9))),
            ((14, 12), Some((14, 11))),
            ((14, 15), Some((14, 14))),
            ((18, 45), Some((17, 22))),
            ((20, 0), Some((18, 47))),
        ];

        for &((line, pos), expected) in &positions {
            assert_eq!(reverse_close_expression(&clean_lines, line, pos), expected);
        }
    }

    #[test]
    fn test_get_text_inside() {
        assert_eq!(get_text_inside("fun()", r"fun\("), Some(String::new()));
        assert_eq!(get_text_inside("f(x, y)", r"f\("), Some("x, y".to_string()));
        assert_eq!(
            get_text_inside("printf(a(), b(c()))", r"printf\("),
            Some("a(), b(c())".to_string())
        );
        assert_eq!(
            get_text_inside("f[x, y{}]", r"f\["),
            Some("x, y{}".to_string())
        );
        assert_eq!(get_text_inside("f[a, b(}]", r"f\["), None);
        assert_eq!(get_text_inside("f[x, y]", r"f\("), None);
        assert_eq!(
            get_text_inside("f(x, g(y, h(z, (a + b))))", r"g\("),
            Some("y, h(z, (a + b))".to_string())
        );
        assert_eq!(
            get_text_inside("f(f(f(x)))", r"f\("),
            Some("f(f(x))".to_string())
        );
        assert_eq!(
            get_text_inside("int loop(int x) {\n  return loop(x);\n}\n", r"\{"),
            Some("\n  return loop(x);\n".to_string())
        );
        assert_eq!(
            get_text_inside(
                "#include \"inl.h\"  // skip #define\n#define A2(x, y) a_inl_(x, y, __LINE__)\n#define A(x) a_inl_(x, \"\", __LINE__)\n",
                r"^\s*#define\s*\w+\(",
            ),
            Some("x, y".to_string())
        );
    }
}
