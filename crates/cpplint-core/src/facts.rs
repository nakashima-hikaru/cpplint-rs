use crate::cleanse::CleansedLines;
use crate::line_utils;
use regex::Regex;
use std::simd::prelude::*;
use std::sync::LazyLock;

static CLASS_DECL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^(\s*(?:template\s*<.*?>\s*)?(class|struct|union)\s+(?:(?:[A-Za-z0-9_]+\s+|\[\[.*?\]\]\s+)*)(\w+(?:::\w+)*(?:<[^;{]*?>)?))(?:\s*[:{]|(?:\s+\[\[.*?\]\])*\s*[:{]|\s*$)?"#,
    )
    .unwrap()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClassFact {
    range: ClassRange,
    name: String,
    is_struct: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts {
    in_namespace_or_extern_block: Vec<bool>,
    class_facts: Vec<ClassFact>,
    class_fact_by_line: Vec<Option<usize>>,
    namespace_top_level_depth: Vec<Option<usize>>,
    closing_brace_starts: Vec<Option<usize>>,
    macro_lines: Vec<bool>,
    matching_block_starts: Vec<Option<usize>>,
    non_blank_elided_prefix: Vec<usize>,
    block_kind: Vec<Option<ScopeKind>>,
    namespace_decl_line: Vec<Option<usize>>,
    non_namespace_indent_depth_before: Vec<usize>,
    non_namespace_indent_depth: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Namespace,
    Extern,
    Block,
}

impl FileFacts {
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn new(clean_lines: &CleansedLines<'_>) -> Self {
        let n = clean_lines.elided.len();
        let mut in_namespace_or_extern_block = Vec::with_capacity(n);
        let mut namespace_top_level_depth = Vec::with_capacity(n);
        let mut closing_brace_starts = Vec::with_capacity(n);
        let mut macro_lines = Vec::with_capacity(n);
        let mut matching_block_starts = vec![None; n];
        let mut non_blank_elided_prefix = Vec::with_capacity(n + 1);
        non_blank_elided_prefix.push(0);

        // State for various trackers
        let mut ns_ext_stack = Vec::new();
        let mut ns_ext_depth = 0usize;
        let mut pending_ns_ext_scope = None;
        let mut last_namespace_decl = None;

        let mut top_ns_stack = Vec::new();
        let mut top_ns_depth = 0usize;
        let mut block_kind = vec![None; n];
        let mut namespace_decl_line = vec![None; n];
        let mut non_namespace_indent_depth_before = Vec::with_capacity(n);
        let mut non_namespace_indent_depth = Vec::with_capacity(n);

        let mut brace_stack = Vec::new();
        let mut matching_stack = Vec::new();
        let mut in_macro_continuation = false;
        let mut non_blank_count = 0usize;

        // 1. Initial brace scan without joining the whole file into a temporary string.
        let line_braces = clean_lines
            .elided
            .iter()
            .map(|line| (brace_count(line, '{') as u32, brace_count(line, '}') as u32))
            .collect::<Vec<_>>();

        for (linenum, (elided, line_no_raw)) in clean_lines
            .elided
            .iter()
            .zip(&clean_lines.lines_without_raw_strings)
            .enumerate()
        {
            // 1. Non-blank prefix
            if !elided.trim().is_empty() {
                non_blank_count += 1;
            }
            non_blank_elided_prefix.push(non_blank_count);

            // 2. Macro lines
            let trimmed_start = line_no_raw.trim_start();
            let is_macro = if trimmed_start.starts_with('#') {
                in_macro_continuation = line_no_raw.trim_end().ends_with('\\');
                true
            } else {
                let current = in_macro_continuation;
                in_macro_continuation =
                    in_macro_continuation && line_no_raw.trim_end().ends_with('\\');
                current
            };
            macro_lines.push(is_macro);

            let (l_braces, r_braces) = (
                line_braces[linenum].0 as usize,
                line_braces[linenum].1 as usize,
            );

            // 3a. in_namespace_or_extern_block
            in_namespace_or_extern_block.push(ns_ext_depth > 0);
            let trimmed_elided = elided.trim();

            if trimmed_elided.starts_with("namespace") {
                last_namespace_decl = Some(linenum);
            }

            if let Some(scope) = pending_ns_ext_scope {
                if trimmed_elided.starts_with('{') {
                    ns_ext_stack.push(scope);
                    if scope == ScopeKind::Namespace {
                        block_kind[linenum] = Some(ScopeKind::Namespace);
                        namespace_decl_line[linenum] = last_namespace_decl;
                    }
                    if matches!(scope, ScopeKind::Namespace | ScopeKind::Extern) {
                        ns_ext_depth += 1;
                    }
                    pending_ns_ext_scope = None;
                    for _ in 1..l_braces {
                        ns_ext_stack.push(ScopeKind::Block);
                    }
                    for _ in 0..r_braces {
                        if let Some(popped) = ns_ext_stack.pop()
                            && matches!(popped, ScopeKind::Namespace | ScopeKind::Extern)
                        {
                            ns_ext_depth = ns_ext_depth.saturating_sub(1);
                        }
                    }
                } else if !trimmed_elided.is_empty() {
                    pending_ns_ext_scope = None;
                }
            }
            if pending_ns_ext_scope.is_none() {
                if l_braces > 0 && last_namespace_decl.is_some() {
                    // Try to confirm if this brace belongs to the namespace
                    // For now, if we have a recent namespace decl and a brace, we assume it's linked
                    ns_ext_stack.push(ScopeKind::Namespace);
                    block_kind[linenum] = Some(ScopeKind::Namespace);
                    namespace_decl_line[linenum] = last_namespace_decl;
                    ns_ext_depth += 1;
                    for _ in 1..l_braces {
                        ns_ext_stack.push(ScopeKind::Block);
                    }
                    for _ in 0..r_braces {
                        if let Some(popped) = ns_ext_stack.pop()
                            && matches!(popped, ScopeKind::Namespace | ScopeKind::Extern)
                        {
                            ns_ext_depth = ns_ext_depth.saturating_sub(1);
                        }
                    }
                    last_namespace_decl = None; // consumed
                } else if trimmed_elided.starts_with("namespace") {
                    if l_braces > 0 {
                        ns_ext_stack.push(ScopeKind::Namespace);
                        block_kind[linenum] = Some(ScopeKind::Namespace);
                        namespace_decl_line[linenum] = Some(linenum);
                        ns_ext_depth += 1;
                        for _ in 1..l_braces {
                            ns_ext_stack.push(ScopeKind::Block);
                        }
                    } else {
                        pending_ns_ext_scope = Some(ScopeKind::Namespace);
                    }
                } else if trimmed_elided.starts_with("extern ") {
                    if l_braces > 0 {
                        ns_ext_stack.push(ScopeKind::Extern);
                        ns_ext_depth += 1;
                    } else {
                        pending_ns_ext_scope = Some(ScopeKind::Extern);
                    }
                } else {
                    for _ in 0..l_braces {
                        ns_ext_stack.push(ScopeKind::Block);
                    }
                    for _ in 0..r_braces {
                        if let Some(popped) = ns_ext_stack.pop()
                            && matches!(popped, ScopeKind::Namespace | ScopeKind::Extern)
                        {
                            ns_ext_depth = ns_ext_depth.saturating_sub(1);
                        }
                    }
                }
            }

            // 3b. namespace_top_level_depth
            let non_ns_before = top_ns_stack
                .iter()
                .filter(|&&k| k == ScopeKind::Block)
                .count();
            non_namespace_indent_depth_before.push(non_ns_before);

            namespace_top_level_depth.push((top_ns_depth > 0).then_some(top_ns_depth));
            if l_braces > 0 && block_kind[linenum] == Some(ScopeKind::Namespace) {
                top_ns_stack.push(ScopeKind::Namespace);
                top_ns_depth += 1;
                for _ in 1..l_braces {
                    top_ns_stack.push(ScopeKind::Block);
                }
                for _ in 0..r_braces {
                    if let Some(popped) = top_ns_stack.pop()
                        && popped == ScopeKind::Namespace
                    {
                        top_ns_depth = top_ns_depth.saturating_sub(1);
                    }
                }
            } else {
                for _ in 0..l_braces {
                    top_ns_stack.push(ScopeKind::Block);
                }
                for _ in 0..r_braces {
                    if let Some(popped) = top_ns_stack.pop()
                        && popped == ScopeKind::Namespace
                    {
                        top_ns_depth = top_ns_depth.saturating_sub(1);
                    }
                }
            }
            let non_ns_depth = top_ns_stack
                .iter()
                .filter(|&&k| k == ScopeKind::Block)
                .count();
            non_namespace_indent_depth.push(non_ns_depth);

            // 3c. closing_brace_starts
            let cbs = if r_braces == 0 {
                None
            } else {
                let mut depth = 0usize;
                let mut found = None;
                for byte in elided.bytes().rev() {
                    match byte {
                        b'}' => depth += 1,
                        b'{' => {
                            if depth == 0 {
                                found = Some(linenum);
                                break;
                            }
                            depth -= 1;
                            if depth == 0 {
                                found = Some(linenum);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if found.is_some() {
                    found
                } else {
                    brace_stack
                        .len()
                        .checked_sub(depth)
                        .and_then(|index| brace_stack.get(index).copied())
                }
            };
            closing_brace_starts.push(cbs);
            for byte in elided.bytes() {
                match byte {
                    b'{' => brace_stack.push(linenum),
                    b'}' => {
                        brace_stack.pop();
                    }
                    _ => {}
                }
            }

            // 3d. matching_block_starts
            for _ in 0..l_braces {
                matching_stack.push(linenum);
            }
            let mut last_popped = None;
            for _ in 0..r_braces {
                if let Some(start) = matching_stack.pop() {
                    last_popped = Some(start);
                }
            }
            matching_block_starts[linenum] = last_popped;
        }

        let (class_facts, class_fact_by_line) =
            build_class_facts(&clean_lines.elided, &line_braces);

        Self {
            in_namespace_or_extern_block,
            class_facts,
            class_fact_by_line,
            namespace_top_level_depth,
            closing_brace_starts,
            macro_lines,
            matching_block_starts,
            non_blank_elided_prefix,
            block_kind,
            namespace_decl_line,
            non_namespace_indent_depth_before,
            non_namespace_indent_depth,
        }
    }

    pub fn enclosing_class_range(&self, linenum: usize) -> Option<ClassRange> {
        self.class_fact_by_line
            .get(linenum)
            .and_then(|index| index.map(|index| self.class_facts[index].range))
    }

    pub fn nearest_class_name(&self, linenum: usize) -> Option<&str> {
        self.class_fact_by_line.get(linenum).and_then(|index| {
            index.and_then(|index| {
                let name = self.class_facts[index].name.as_str();
                (!name.is_empty()).then_some(name)
            })
        })
    }

    pub fn enclosing_class_is_struct(&self, linenum: usize) -> Option<bool> {
        self.class_fact_by_line
            .get(linenum)
            .and_then(|index| index.map(|index| self.class_facts[index].is_struct))
    }

    pub fn namespace_top_level_depth(&self, linenum: usize) -> Option<usize> {
        self.namespace_top_level_depth
            .get(linenum)
            .copied()
            .flatten()
    }

    pub fn non_namespace_indent_depth_before(&self, linenum: usize) -> usize {
        self.non_namespace_indent_depth_before
            .get(linenum)
            .copied()
            .unwrap_or(0)
    }

    pub fn non_namespace_indent_depth(&self, linenum: usize) -> usize {
        self.non_namespace_indent_depth
            .get(linenum)
            .copied()
            .unwrap_or(0)
    }

    pub fn block_kind(&self, linenum: usize) -> Option<ScopeKind> {
        self.block_kind.get(linenum).copied().flatten()
    }

    pub fn namespace_decl_line(&self, linenum: usize) -> Option<usize> {
        self.namespace_decl_line.get(linenum).copied().flatten()
    }

    pub fn matching_block_start(&self, linenum: usize) -> Option<usize> {
        self.matching_block_starts.get(linenum).copied().flatten()
    }

    pub fn non_blank_elided_lines_between(
        &self,
        start_exclusive: usize,
        end_exclusive: usize,
    ) -> usize {
        if end_exclusive <= start_exclusive.saturating_add(1)
            || end_exclusive >= self.non_blank_elided_prefix.len()
        {
            return 0;
        }

        self.non_blank_elided_prefix[end_exclusive]
            .saturating_sub(self.non_blank_elided_prefix[start_exclusive + 1])
    }
}

static CLASS_KEYWORDS_AC: LazyLock<aho_corasick::AhoCorasick> =
    LazyLock::new(|| aho_corasick::AhoCorasick::new(["class", "struct", "union"]).unwrap());

fn build_class_facts<S: AsRef<str>>(
    lines: &[S],
    line_braces: &[(u32, u32)],
) -> (Vec<ClassFact>, Vec<Option<usize>>) {
    let mut class_facts = Vec::new();
    let mut pending: Option<(usize, String, bool)> = None;

    for (linenum, line) in lines.iter().enumerate() {
        let line = line.as_ref();
        if !CLASS_KEYWORDS_AC.is_match(line) && pending.is_none() {
            continue;
        }
        let trimmed = line.trim();

        if pending.is_none()
            && let Some(captures) = CLASS_DECL_RE.captures(trimmed)
        {
            let end_declaration = captures.get(1).map(|m| m.end()).unwrap_or(0);
            if !in_template_argument_list(lines, linenum, end_declaration) {
                let name = captures
                    .get(3)
                    .map(|matched| matched.as_str().to_string())
                    .unwrap_or_default();
                let is_struct = captures
                    .get(2)
                    .is_some_and(|matched| matched.as_str() == "struct");
                pending = Some((linenum, name, is_struct));
            }
        }

        let Some(start) = pending.as_ref().map(|(start, _, _)| *start) else {
            continue;
        };
        if !trimmed.contains('{') {
            if trimmed.contains(';') || trimmed.contains('}') {
                pending = None;
            }
            continue;
        }

        let (l, r) = line_braces[linenum];
        let mut depth = l as isize - r as isize;
        if depth <= 0 {
            pending = None;
            continue;
        }

        let mut class_end = None;
        for (end, &(l, r)) in line_braces.iter().enumerate().skip(linenum + 1) {
            depth += l as isize;
            depth -= r as isize;
            if depth == 0 {
                class_end = Some(end);
                break;
            }
        }
        if let Some(end) = class_end {
            let (_, name, is_struct) = pending.take().unwrap();
            class_facts.push(ClassFact {
                range: ClassRange { start, end },
                name,
                is_struct,
            });
        } else {
            pending = None;
        }
    }

    let mut class_fact_by_line: Vec<Option<usize>> = vec![None; lines.len()];
    for (index, class_fact) in class_facts.iter().enumerate() {
        for existing_opt in class_fact_by_line
            .iter_mut()
            .take(class_fact.range.end + 1)
            .skip(class_fact.range.start + 1)
        {
            let should_replace = existing_opt
                .map(|existing| class_facts[existing].range.start <= class_fact.range.start)
                .unwrap_or(true);
            if should_replace {
                *existing_opt = Some(index);
            }
        }
    }

    (class_facts, class_fact_by_line)
}

fn in_template_argument_list<S: AsRef<str>>(
    lines: &[S],
    mut linenum: usize,
    mut pos: usize,
) -> bool {
    while linenum < lines.len() {
        let line = lines[linenum].as_ref();
        if pos >= line.len() {
            linenum += 1;
            pos = 0;
            continue;
        }

        let slice = &line[pos..];
        let Some((offset, ch)) = slice
            .char_indices()
            .find(|(_, c)| matches!(c, '{' | '}' | ';' | '=' | '[' | ']' | '.' | '<' | '>'))
        else {
            linenum += 1;
            pos = 0;
            continue;
        };

        pos += offset + ch.len_utf8();

        match ch {
            '{' | '}' | ';' => return false,
            '>' | '=' | '[' | ']' | '.' => return true,
            '<' => {
                let open_pos = pos.saturating_sub(1);
                let Some((end_line, end_pos)) =
                    line_utils::close_expression_in_lines(lines, linenum, open_pos)
                else {
                    return false;
                };
                linenum = end_line;
                pos = end_pos;
            }
            _ => {
                // Should not happen given the find criteria
                pos += 1;
                if pos >= line.len() {
                    linenum += 1;
                    pos = 0;
                }
            }
        }
    }

    false
}

fn brace_count(line: &str, brace: char) -> usize {
    let bytes = line.as_bytes();
    let b = brace as u8;
    let mut count = 0;
    let mut i = 0;
    while i + 32 <= bytes.len() {
        let chunk = u8x32::from_slice(&bytes[i..i + 32]);
        count += chunk.simd_eq(u8x32::splat(b)).to_bitmask().count_ones() as usize;
        i += 32;
    }
    for &byte in &bytes[i..] {
        if byte == b {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    #[test]
    fn file_facts_capture_namespace_macro_class_and_blocks() {
        let arena = Bump::new();
        let lines = [
            "namespace {",
            "  int value = 0;",
            "}",
            "#define FOO(x) \\",
            "  x",
            "class Foo {",
            " public:",
            "};",
        ];
        let clean_lines = CleansedLines::new(&arena, &lines);

        let facts = FileFacts::new(&clean_lines);

        assert_eq!(facts.namespace_top_level_depth(1), Some(1));
        assert_eq!(facts.matching_block_start(2), Some(0));
        assert_eq!(
            facts.enclosing_class_range(6),
            Some(ClassRange { start: 5, end: 7 })
        );
        assert_eq!(facts.nearest_class_name(6), Some("Foo"));
        assert_eq!(facts.non_blank_elided_lines_between(5, 7), 1);
    }

    #[test]
    fn file_facts_capture_split_namespace_blocks() {
        let arena = Bump::new();
        let lines = ["namespace", "Foo", "{", "  int value = 0;", "}"];
        let clean_lines = CleansedLines::new(&arena, &lines);

        let facts = FileFacts::new(&clean_lines);

        assert_eq!(facts.namespace_top_level_depth(3), Some(1));
        assert_eq!(facts.matching_block_start(4), Some(2));
    }

    #[test]
    fn file_facts_track_closing_brace_context_on_mixed_brace_lines() {
        let arena = Bump::new();
        let lines = [
            "namespace foo {",
            "  const int values[] = {",
            "    1,",
            "  }, make_pair({1, 2});",
            "  if (ready) {",
            "  } else {",
            "}",
        ];
        let clean_lines = CleansedLines::new(&arena, &lines);

        let facts = FileFacts::new(&clean_lines);

        assert_eq!(facts.matching_block_start(3), Some(1));
    }
}
