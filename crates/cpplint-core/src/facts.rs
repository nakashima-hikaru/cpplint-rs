use crate::cleanse::CleansedLines;
use crate::line_utils;
use memchr::memchr3_iter;
use regex::Regex;
use std::num::{NonZeroU8, NonZeroU32};
use std::simd::cmp::SimdPartialEq;
use std::simd::u8x32;
use std::sync::LazyLock;

static CLASS_DECL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^(\s*(?:template\s*<.*?>\s*)?(class|struct|union)\s+(?:(?:[A-Za-z0-9_]+\s+|\[\[.*?\]\]\s+)*)(\w+(?:::\w+)*(?:<[^;{]*?>)?))(?:\s*[:{]|(?:\s+\[\[.*?\]\])*\s*[:{]|\s*$)?"#,
    )
    .unwrap()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    Class,
    Struct,
    Union,
}

impl ClassKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Union => "union",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClassFact<'a> {
    range: ClassRange,
    name: &'a str,
    kind: ClassKind,
}

use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts<'a> {
    class_facts: BumpVec<'a, ClassFact<'a>>,
    class_fact_indices: BumpVec<'a, Option<NonZeroU32>>,
    namespace_top_level_depths: BumpVec<'a, Option<NonZeroU8>>,
    matching_block_starts: BumpVec<'a, Option<NonZeroU32>>,
    block_kinds: BumpVec<'a, Option<ScopeKind>>,
    namespace_decl_lines: BumpVec<'a, Option<NonZeroU32>>,
    non_namespace_indent_depths_before: BumpVec<'a, u16>,
    non_namespace_indent_depths: BumpVec<'a, u16>,
    non_blank_elided_prefix: BumpVec<'a, u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Namespace,
    Extern,
    Block,
}

impl<'a> FileFacts<'a> {
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn new(clean_lines: &CleansedLines<'a>, arena: &'a Bump) -> Self {
        let n = clean_lines.elided.len();
        let mut namespace_top_level_depths = bumpalo::vec![in arena; None; n];
        let mut matching_block_starts = bumpalo::vec![in arena; None; n];
        let mut block_kinds = bumpalo::vec![in arena; None; n];
        let mut namespace_decl_lines = bumpalo::vec![in arena; None; n];
        let mut non_namespace_indent_depths_before = bumpalo::vec![in arena; 0u16; n];
        let mut non_namespace_indent_depths = bumpalo::vec![in arena; 0u16; n];
        let mut matching_block_ends: BumpVec<'a, Option<NonZeroU32>> =
            bumpalo::vec![in arena; None; n];
        let mut non_blank_elided_prefix = BumpVec::with_capacity_in(n + 1, arena);
        non_blank_elided_prefix.push(0);

        // State for various trackers
        let mut ns_ext_stack = BumpVec::new_in(arena);
        let mut ns_ext_depth = 0usize;
        let mut pending_ns_ext_scope = None;
        let mut last_namespace_decl = None;

        let mut top_ns_stack = BumpVec::new_in(arena);
        let mut top_ns_depth = 0usize;
        let mut top_non_namespace_depth = 0usize;

        let mut matching_stack = BumpVec::new_in(arena);
        let mut non_blank_count = 0u32;

        // 1. We will compute line_braces on the fly.
        let mut line_braces = BumpVec::with_capacity_in(n, arena);

        for (linenum, elided) in clean_lines.elided.iter().enumerate() {
            // 1. Non-blank prefix
            if !elided.trim().is_empty() {
                non_blank_count += 1;
            }
            non_blank_elided_prefix.push(non_blank_count);

            let (l_braces_count, r_braces_count) = brace_counts(elided);
            line_braces.push((l_braces_count as u32, r_braces_count as u32));
            let l_braces = l_braces_count;
            let r_braces = r_braces_count;

            let trimmed_elided = elided.trim();

            if trimmed_elided.starts_with("namespace") {
                last_namespace_decl = u32::try_from(linenum + 1).ok().and_then(NonZeroU32::new);
            }

            if let Some(scope) = pending_ns_ext_scope {
                if trimmed_elided.starts_with('{') {
                    ns_ext_stack.push(scope);
                    if scope == ScopeKind::Namespace {
                        block_kinds[linenum] = Some(ScopeKind::Namespace);
                        namespace_decl_lines[linenum] = last_namespace_decl;
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
                    ns_ext_stack.push(ScopeKind::Namespace);
                    block_kinds[linenum] = Some(ScopeKind::Namespace);
                    namespace_decl_lines[linenum] = last_namespace_decl;
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
                        block_kinds[linenum] = Some(ScopeKind::Namespace);
                        namespace_decl_lines[linenum] =
                            u32::try_from(linenum + 1).ok().and_then(NonZeroU32::new);
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
            non_namespace_indent_depths_before[linenum] = top_non_namespace_depth as u16;

            namespace_top_level_depths[linenum] =
                u8::try_from(top_ns_depth).ok().and_then(NonZeroU8::new);
            if l_braces > 0 && block_kinds[linenum] == Some(ScopeKind::Namespace) {
                top_ns_stack.push(ScopeKind::Namespace);
                top_ns_depth += 1;
                for _ in 1..l_braces {
                    top_ns_stack.push(ScopeKind::Block);
                    top_non_namespace_depth += 1;
                }
                for _ in 0..r_braces {
                    if let Some(popped) = top_ns_stack.pop() {
                        match popped {
                            ScopeKind::Namespace => {
                                top_ns_depth = top_ns_depth.saturating_sub(1);
                            }
                            ScopeKind::Block => {
                                top_non_namespace_depth = top_non_namespace_depth.saturating_sub(1);
                            }
                            ScopeKind::Extern => {}
                        }
                    }
                }
            } else {
                for _ in 0..l_braces {
                    top_ns_stack.push(ScopeKind::Block);
                    top_non_namespace_depth += 1;
                }
                for _ in 0..r_braces {
                    if let Some(popped) = top_ns_stack.pop() {
                        match popped {
                            ScopeKind::Namespace => {
                                top_ns_depth = top_ns_depth.saturating_sub(1);
                            }
                            ScopeKind::Block => {
                                top_non_namespace_depth = top_non_namespace_depth.saturating_sub(1);
                            }
                            ScopeKind::Extern => {}
                        }
                    }
                }
            }
            non_namespace_indent_depths[linenum] = top_non_namespace_depth as u16;

            // 3d. matching_block_starts
            for _ in 0..l_braces {
                matching_stack.push(linenum);
            }
            let mut last_popped = None;
            for _ in 0..r_braces {
                if let Some(start) = matching_stack.pop() {
                    matching_block_ends[start] =
                        u32::try_from(linenum + 1).ok().and_then(NonZeroU32::new);
                    last_popped = Some(start);
                }
            }
            matching_block_starts[linenum] =
                last_popped.and_then(|line| u32::try_from(line + 1).ok().and_then(NonZeroU32::new));
        }

        let (class_facts, class_fact_indices) = build_class_facts(
            clean_lines.elided.as_slice(),
            &line_braces,
            &matching_block_ends,
            arena,
        );

        Self {
            class_facts,
            class_fact_indices,
            namespace_top_level_depths,
            matching_block_starts,
            block_kinds,
            namespace_decl_lines,
            non_namespace_indent_depths_before,
            non_namespace_indent_depths,
            non_blank_elided_prefix,
        }
    }

    pub fn enclosing_class_range(&self, linenum: usize) -> Option<ClassRange> {
        self.class_fact_indices
            .get(linenum)
            .copied()
            .flatten()
            .map(|nz| self.class_facts[nz.get() as usize - 1].range)
    }

    pub fn nearest_class_name(&self, linenum: usize) -> Option<&str> {
        self.class_fact_indices
            .get(linenum)
            .copied()
            .flatten()
            .and_then(|nz| {
                let name = self.class_facts[nz.get() as usize - 1].name;
                (!name.is_empty()).then_some(name)
            })
    }

    pub fn enclosing_class_kind(&self, linenum: usize) -> Option<ClassKind> {
        self.class_fact_indices
            .get(linenum)
            .copied()
            .flatten()
            .map(|nz| self.class_facts[nz.get() as usize - 1].kind)
    }

    pub fn namespace_top_level_depth(&self, linenum: usize) -> Option<NonZeroU8> {
        self.namespace_top_level_depths
            .get(linenum)
            .copied()
            .flatten()
    }

    pub fn non_namespace_indent_depth_before(&self, linenum: usize) -> usize {
        self.non_namespace_indent_depths_before
            .get(linenum)
            .copied()
            .unwrap_or(0) as usize
    }

    pub fn non_namespace_indent_depth(&self, linenum: usize) -> usize {
        self.non_namespace_indent_depths
            .get(linenum)
            .copied()
            .unwrap_or(0) as usize
    }

    pub fn block_kind(&self, linenum: usize) -> Option<ScopeKind> {
        self.block_kinds.get(linenum).copied().flatten()
    }

    pub fn namespace_decl_line(&self, linenum: usize) -> Option<NonZeroU32> {
        self.namespace_decl_lines.get(linenum).copied().flatten()
    }

    pub fn matching_block_start(&self, linenum: usize) -> Option<NonZeroU32> {
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
            .saturating_sub(self.non_blank_elided_prefix[start_exclusive + 1]) as usize
    }
}

fn class_keywords_is_match(line: &str) -> bool {
    let bytes = line.as_bytes();
    for pos in memchr3_iter(b'c', b's', b'u', bytes) {
        let tail = &line[pos..];
        if tail.starts_with("class") || tail.starts_with("struct") || tail.starts_with("union") {
            return true;
        }
    }
    false
}

fn build_class_facts<'a>(
    lines: &[&'a str],
    line_braces: &[(u32, u32)],
    matching_block_ends: &[Option<NonZeroU32>],
    arena: &'a Bump,
) -> (BumpVec<'a, ClassFact<'a>>, BumpVec<'a, Option<NonZeroU32>>) {
    let mut class_facts = BumpVec::new_in(arena);
    let mut pending: Option<(usize, &'a str, ClassKind)> = None;

    for (linenum, line) in lines.iter().enumerate() {
        let line = *line;
        if !class_keywords_is_match(line) && pending.is_none() {
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
                    .map(|matched| matched.as_str())
                    .unwrap_or("");
                let kind = match captures.get(2).map(|matched| matched.as_str()) {
                    Some("struct") => ClassKind::Struct,
                    Some("union") => ClassKind::Union,
                    _ => ClassKind::Class,
                };
                pending = Some((linenum, name, kind));
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
        if l <= r {
            pending = None;
            continue;
        }

        if let Some(end_nz) = matching_block_ends[linenum] {
            let end = end_nz.get() as usize - 1;
            let (_, name, kind) = pending.take().unwrap();
            class_facts.push(ClassFact {
                range: ClassRange { start, end },
                name,
                kind,
            });
        } else {
            pending = None;
        }
    }

    let mut class_fact_by_line: BumpVec<'a, Option<NonZeroU32>> =
        bumpalo::vec![in arena; None; lines.len()];

    if !class_facts.is_empty() {
        let mut order: BumpVec<'a, usize> = BumpVec::with_capacity_in(class_facts.len(), arena);
        order.extend(0..class_facts.len());
        order.sort_unstable_by_key(|&i| {
            let f = &class_facts[i];
            (f.range.start, std::cmp::Reverse(f.range.end))
        });

        for &index in &order {
            let class_fact = &class_facts[index];
            let nz = u32::try_from(index + 1).ok().and_then(NonZeroU32::new);
            let start = class_fact.range.start + 1;
            let end = (class_fact.range.end + 1).min(class_fact_by_line.len());
            if start < end {
                class_fact_by_line[start..end].fill(nz);
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

fn brace_counts(line: &str) -> (usize, usize) {
    let bytes = line.as_bytes();
    let mut open = 0usize;
    let mut close = 0usize;
    let mut i = 0usize;

    while i + 32 <= bytes.len() {
        let chunk = u8x32::from_slice(&bytes[i..i + 32]);
        open += chunk.simd_eq(u8x32::splat(b'{')).to_bitmask().count_ones() as usize;
        close += chunk.simd_eq(u8x32::splat(b'}')).to_bitmask().count_ones() as usize;
        i += 32;
    }

    for &byte in &bytes[i..] {
        match byte {
            b'{' => open += 1,
            b'}' => close += 1,
            _ => {}
        }
    }

    (open, close)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    macro_rules! with_facts {
        ($lines:expr, |$facts:ident| $body:block) => {{
            let arena = Bump::new();
            let clean_lines = CleansedLines::new(&arena, &$lines);
            let $facts = FileFacts::new(&clean_lines, &arena);
            $body
        }};
    }

    #[test]
    fn test_empty() {
        with_facts!([], |facts| {
            assert_eq!(facts.namespace_top_level_depth(0), None);
            assert_eq!(facts.enclosing_class_range(0), None);
            assert_eq!(facts.block_kind(0), None);
        });
    }

    #[test]
    fn test_namespace() {
        with_facts!(["namespace {", "  int value = 0;", "}"], |facts| {
            assert_eq!(facts.block_kind(0), Some(ScopeKind::Namespace));
            assert_eq!(facts.namespace_decl_line(0), NonZeroU32::new(1));
            assert_eq!(facts.namespace_top_level_depth(1), NonZeroU8::new(1));
            assert_eq!(facts.matching_block_start(2), NonZeroU32::new(1));
        });

        with_facts!(
            ["namespace", "Foo", "{", "  int value = 0;", "}"],
            |facts| {
                assert_eq!(facts.namespace_top_level_depth(3), NonZeroU8::new(1));
                assert_eq!(facts.non_namespace_indent_depth_before(3), 0);
                assert_eq!(facts.non_namespace_indent_depth(3), 0);
                assert_eq!(facts.matching_block_start(4), NonZeroU32::new(3));
            }
        );
    }

    #[test]
    fn test_decorated_class() {
        with_facts!(
            ["class Decorated_123 API A {", "  int value;", "};"],
            |facts| {
                assert_eq!(
                    facts.enclosing_class_range(1),
                    Some(ClassRange { start: 0, end: 2 })
                );
                assert_eq!(facts.nearest_class_name(1), Some("A"));
                assert_eq!(facts.enclosing_class_kind(1), Some(ClassKind::Class));
            }
        );
    }

    #[test]
    fn test_inner_class() {
        with_facts!(["class A::B::C {", "  int value;", "};"], |facts| {
            assert_eq!(
                facts.enclosing_class_range(1),
                Some(ClassRange { start: 0, end: 2 })
            );
            assert_eq!(facts.nearest_class_name(1), Some("A::B::C"));
            assert_eq!(facts.enclosing_class_kind(1), Some(ClassKind::Class));
        });
    }

    #[test]
    fn test_class() {
        with_facts!(["class A {", "  int value;", "};"], |facts| {
            assert_eq!(
                facts.enclosing_class_range(1),
                Some(ClassRange { start: 0, end: 2 })
            );
            assert_eq!(facts.nearest_class_name(1), Some("A"));
            assert_eq!(facts.enclosing_class_kind(1), Some(ClassKind::Class));
        });

        with_facts!(["struct B : public A {", "  int value;", "};"], |facts| {
            assert_eq!(
                facts.enclosing_class_range(1),
                Some(ClassRange { start: 0, end: 2 })
            );
            assert_eq!(facts.nearest_class_name(1), Some("B"));
            assert_eq!(facts.enclosing_class_kind(1), Some(ClassKind::Struct));
        });

        with_facts!(["class C", ": public A {", "  int value;", "};"], |facts| {
            assert_eq!(
                facts.enclosing_class_range(2),
                Some(ClassRange { start: 0, end: 3 })
            );
            assert_eq!(facts.nearest_class_name(2), Some("C"));
            assert_eq!(facts.enclosing_class_kind(2), Some(ClassKind::Class));
        });

        with_facts!(
            ["class D {", "  class E {", "    int value;", "  };", "};"],
            |facts| {
                assert_eq!(
                    facts.enclosing_class_range(2),
                    Some(ClassRange { start: 1, end: 3 })
                );
                assert_eq!(facts.nearest_class_name(2), Some("E"));
                assert_eq!(facts.enclosing_class_kind(2), Some(ClassKind::Class));
            }
        );
    }

    #[test]
    fn test_struct() {
        with_facts!(["struct A {", "  int value;", "};"], |facts| {
            assert_eq!(
                facts.enclosing_class_range(1),
                Some(ClassRange { start: 0, end: 2 })
            );
            assert_eq!(facts.nearest_class_name(1), Some("A"));
            assert_eq!(facts.enclosing_class_kind(1), Some(ClassKind::Struct));
        });
    }

    #[test]
    fn test_template() {
        with_facts!(
            [
                "template <T,",
                "          class Arg1 = tmpl<T> >",
                "class A {",
                "  int value;",
                "};",
            ],
            |facts| {
                assert_eq!(
                    facts.enclosing_class_range(3),
                    Some(ClassRange { start: 2, end: 4 })
                );
                assert_eq!(facts.nearest_class_name(3), Some("A"));
                assert_eq!(facts.enclosing_class_kind(3), Some(ClassKind::Class));
            }
        );
    }

    #[test]
    fn test_template_default_arg() {
        with_facts!(
            [
                "template <class T, class D = default_delete<T>> class unique_ptr {",
                "  T* ptr;",
                "};",
            ],
            |facts| {
                assert_eq!(
                    facts.enclosing_class_range(1),
                    Some(ClassRange { start: 0, end: 2 })
                );
                assert_eq!(facts.nearest_class_name(1), Some("unique_ptr"));
                assert_eq!(facts.enclosing_class_kind(1), Some(ClassKind::Class));
            }
        );
    }

    #[test]
    fn test_template_inner_class() {
        with_facts!(
            [
                "class A {",
                " public:",
                "  template <class B>",
                "  class C<alloc<B> >",
                "      : public A {",
                "    B value;",
                "  };",
                "};",
            ],
            |facts| {
                assert_eq!(
                    facts.enclosing_class_range(5),
                    Some(ClassRange { start: 0, end: 7 })
                );
                assert_eq!(facts.nearest_class_name(5), Some("A"));
                assert_eq!(facts.enclosing_class_kind(5), Some(ClassKind::Class));
            }
        );
    }

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

        let facts = FileFacts::new(&clean_lines, &arena);

        assert_eq!(facts.namespace_top_level_depth(1), NonZeroU8::new(1));
        assert_eq!(facts.non_namespace_indent_depth_before(6), 1);
        assert_eq!(facts.non_namespace_indent_depth(6), 1);
        assert_eq!(facts.matching_block_start(2), NonZeroU32::new(1));
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

        let facts = FileFacts::new(&clean_lines, &arena);

        assert_eq!(facts.namespace_top_level_depth(3), NonZeroU8::new(1));
        assert_eq!(facts.non_namespace_indent_depth_before(3), 0);
        assert_eq!(facts.non_namespace_indent_depth(3), 0);
        assert_eq!(facts.matching_block_start(4), NonZeroU32::new(3));
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

        let facts = FileFacts::new(&clean_lines, &arena);

        assert_eq!(facts.non_namespace_indent_depth_before(4), 0);
        assert_eq!(facts.non_namespace_indent_depth(4), 1);
        assert_eq!(facts.matching_block_start(3), NonZeroU32::new(2));
    }

    #[test]
    fn brace_counts_tracks_both_brace_kinds() {
        assert_eq!(brace_counts("{{ foo } bar }"), (2, 2));
        assert_eq!(brace_counts("no braces"), (0, 0));
    }
}
