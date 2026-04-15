use crate::cleanse::CleansedLines;

pub fn get_indent_level(line: &str) -> usize {
    line.as_bytes().iter().take_while(|&&b| b == b' ').count()
}

pub fn is_blank_line(line: &str) -> bool {
    line.trim().is_empty()
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
}
