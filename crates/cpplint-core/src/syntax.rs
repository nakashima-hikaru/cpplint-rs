use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;
use tree_sitter::{Node, Parser, Tree};

const PROTECTED_NODE_KINDS: &[&str] = &[
    "comment",
    "string_literal",
    "concatenated_string",
    "raw_string_literal",
    "char_literal",
    "system_lib_string",
];

const STRING_NODE_KINDS: &[&str] = &[
    "string_literal",
    "concatenated_string",
    "raw_string_literal",
];

thread_local! {
    static CPP_PARSER: RefCell<Parser> = RefCell::new({
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_cpp::LANGUAGE.into())
            .expect("tree-sitter-cpp language should initialize");
        parser
    });
    static LAST_PARSED_LINE: RefCell<Option<(Arc<str>, Tree)>> = const { RefCell::new(None) };
}

#[derive(Debug)]
pub(crate) struct ParsedLine {
    source: Arc<str>,
    tree: Tree,
}

#[derive(Debug, Clone)]
pub(crate) struct CallExpression<'tree> {
    pub node: Node<'tree>,
    pub function: Node<'tree>,
    pub arguments: Vec<Node<'tree>>,
}

#[derive(Debug, Clone)]
pub(crate) struct CastExpression<'tree> {
    pub node: Node<'tree>,
    pub value_node: Node<'tree>,
}

#[derive(Debug, Clone)]
pub(crate) struct InvalidIncrementExpression<'tree> {
    pub node: Node<'tree>,
    pub operand: Node<'tree>,
    pub operator: &'static str,
}

impl ParsedLine {
    pub(crate) fn parse(line: &str) -> Option<Self> {
        if let Some((source, tree)) = LAST_PARSED_LINE.with_borrow(|cached| {
            cached
                .as_ref()
                .filter(|(source, _)| source.as_ref() == line)
                .map(|(source, tree)| (source.clone(), tree.clone()))
        }) {
            return Some(Self { source, tree });
        }

        let tree = CPP_PARSER.with_borrow_mut(|parser| parser.parse(line, None))?;
        let source: Arc<str> = Arc::from(line);
        LAST_PARSED_LINE.with_borrow_mut(|cached| {
            *cached = Some((source.clone(), tree.clone()));
        });
        Some(Self { source, tree })
    }

    pub(crate) fn node_text<'a>(&'a self, node: Node<'a>) -> Option<&'a str> {
        node.utf8_text(self.source.as_bytes()).ok()
    }

    pub(crate) fn replace_range(&self, range: Range<usize>, replacement: &str) -> String {
        let mut fixed = self.source.to_string();
        fixed.replace_range(range, replacement);
        fixed
    }

    pub(crate) fn replace_node(&self, node: Node<'_>, replacement: &str) -> String {
        self.replace_range(node.byte_range(), replacement)
    }

    pub(crate) fn rewrite_code_segments(
        &self,
        mut transform: impl FnMut(&str) -> String,
    ) -> Option<String> {
        let protected = self.protected_ranges();
        let mut fixed = String::with_capacity(self.source.len());
        let mut cursor = 0usize;

        for range in protected {
            if cursor < range.start {
                fixed.push_str(&transform(&self.source[cursor..range.start]));
            }
            fixed.push_str(&self.source[range.clone()]);
            cursor = range.end;
        }

        if cursor < self.source.len() {
            fixed.push_str(&transform(&self.source[cursor..]));
        }

        (fixed != self.source.as_ref()).then_some(fixed)
    }

    pub(crate) fn rewrite_string_literals(
        &self,
        mut transform: impl FnMut(&str) -> Option<String>,
    ) -> Option<String> {
        self.rewrite_string_literals_in(0..self.source.len(), move |literal| transform(literal))
    }

    pub(crate) fn rewrite_string_literals_in(
        &self,
        within: Range<usize>,
        mut transform: impl FnMut(&str) -> Option<String>,
    ) -> Option<String> {
        let mut replacements = Vec::new();
        for range in self.string_ranges_in(within) {
            let original = &self.source[range.clone()];
            if let Some(replacement) = transform(original)
                && replacement != original
            {
                replacements.push((range, replacement));
            }
        }

        apply_replacements(&self.source, replacements)
    }

    pub(crate) fn first_comment_start(&self) -> Option<usize> {
        let mut comment_start = None;
        self.visit(self.tree.root_node(), &mut |node| {
            if node.kind() == "comment" {
                comment_start = Some(node.start_byte());
                return false;
            }
            true
        });
        comment_start
    }

    pub(crate) fn string_ranges_in(&self, within: Range<usize>) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        self.visit(self.tree.root_node(), &mut |node| {
            if STRING_NODE_KINDS.contains(&node.kind()) {
                let range = node.byte_range();
                if range.start >= within.start && range.end <= within.end {
                    ranges.push(range);
                }
                return false;
            }
            true
        });
        normalize_ranges(ranges)
    }

    pub(crate) fn find_call_expression(&self, base_names: &[&str]) -> Option<CallExpression<'_>> {
        let mut found = None;
        self.visit(self.tree.root_node(), &mut |node| {
            if found.is_some() {
                return false;
            }
            if node.kind() != "call_expression" {
                return true;
            }

            let Some(function) = node.child_by_field_name("function") else {
                return true;
            };
            let Some(function_text) = self.node_text(function) else {
                return true;
            };
            if !base_names
                .iter()
                .any(|name| *name == base_name(function_text))
            {
                return true;
            }

            let arguments = node
                .child_by_field_name("arguments")
                .map(|argument_list| {
                    let mut cursor = argument_list.walk();
                    argument_list.named_children(&mut cursor).collect()
                })
                .unwrap_or_default();
            found = Some(CallExpression {
                node,
                function,
                arguments,
            });
            false
        });
        found
    }

    pub(crate) fn find_call_expression_matching(
        &self,
        mut predicate: impl FnMut(&str, &[Node<'_>]) -> bool,
    ) -> Option<CallExpression<'_>> {
        let mut found = None;
        self.visit(self.tree.root_node(), &mut |node| {
            if found.is_some() {
                return false;
            }
            if node.kind() != "call_expression" {
                return true;
            }

            let Some(function) = node.child_by_field_name("function") else {
                return true;
            };
            let arguments = node
                .child_by_field_name("arguments")
                .map(|argument_list| {
                    let mut cursor = argument_list.walk();
                    argument_list
                        .named_children(&mut cursor)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let Some(function_text) = self.node_text(function) else {
                return true;
            };
            if !predicate(function_text, &arguments) {
                return true;
            }
            found = Some(CallExpression {
                node,
                function,
                arguments,
            });
            false
        });
        found
    }

    pub(crate) fn find_cast_expression_matching(
        &self,
        mut predicate: impl FnMut(&str, Node<'_>) -> bool,
    ) -> Option<CastExpression<'_>> {
        let mut found = None;
        self.visit(self.tree.root_node(), &mut |node| {
            if found.is_some() {
                return false;
            }
            if node.kind() != "cast_expression" {
                return true;
            }

            let Some(type_node) = node.child_by_field_name("type") else {
                return true;
            };
            let Some(value_node) = node.child_by_field_name("value") else {
                return true;
            };
            let Some(type_text) = self.node_text(type_node) else {
                return true;
            };
            if !predicate(type_text, value_node) {
                return true;
            }
            found = Some(CastExpression { node, value_node });
            false
        });
        found
    }

    pub(crate) fn find_invalid_increment_expression(
        &self,
    ) -> Option<InvalidIncrementExpression<'_>> {
        let mut found = None;
        self.visit(self.tree.root_node(), &mut |node| {
            if found.is_some() {
                return false;
            }
            if node.kind() != "pointer_expression" {
                return true;
            }

            let Some(update_node) = node.child_by_field_name("argument") else {
                return true;
            };
            if update_node.kind() != "update_expression" {
                return true;
            }

            let Some(operand) = update_node.child_by_field_name("argument") else {
                return true;
            };
            let Some(update_text) = self.node_text(update_node) else {
                return true;
            };
            let Some(operand_text) = self.node_text(operand) else {
                return true;
            };
            let suffix = update_text.get(operand_text.len()..).unwrap_or("").trim();
            let operator = match suffix {
                "++" => "++",
                "--" => "--",
                _ => return true,
            };

            found = Some(InvalidIncrementExpression {
                node,
                operand,
                operator,
            });
            false
        });
        found
    }

    pub(crate) fn binary_expression_parts<'a>(
        &'a self,
        node: Node<'a>,
    ) -> Option<(Node<'a>, &'a str, Node<'a>)> {
        if node.kind() != "binary_expression" {
            return None;
        }
        let left = node.child_by_field_name("left")?;
        let right = node.child_by_field_name("right")?;
        let operator = self.source[left.end_byte()..right.start_byte()].trim();
        Some((left, operator, right))
    }

    pub(crate) fn rhs_contains_only_string_literals(&self, start: usize, end: usize) -> bool {
        let mut cursor = start;
        let ranges = self.string_ranges_in(start..end);
        if ranges.is_empty() {
            return false;
        }

        for range in ranges {
            if self.source[cursor..range.start]
                .chars()
                .any(|ch| !ch.is_ascii_whitespace())
            {
                return false;
            }
            cursor = range.end;
        }

        !self.source[cursor..end]
            .chars()
            .any(|ch| !ch.is_ascii_whitespace())
    }

    fn protected_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        self.visit(self.tree.root_node(), &mut |node| {
            if PROTECTED_NODE_KINDS.contains(&node.kind()) {
                ranges.push(node.byte_range());
                return false;
            }
            true
        });
        normalize_ranges(ranges)
    }

    fn visit<'tree>(&'tree self, node: Node<'tree>, f: &mut impl FnMut(Node<'tree>) -> bool) {
        if !f(node) {
            return;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit(child, f);
        }
    }
}

fn normalize_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        if let Some(last) = merged.last_mut()
            && range.start <= last.end
        {
            last.end = last.end.max(range.end);
            continue;
        }
        merged.push(range);
    }
    merged
}

fn apply_replacements(
    original: &str,
    mut replacements: Vec<(Range<usize>, String)>,
) -> Option<String> {
    if replacements.is_empty() {
        return None;
    }
    replacements.sort_by(|lhs, rhs| rhs.0.start.cmp(&lhs.0.start));
    let mut fixed = original.to_string();
    for (range, replacement) in replacements {
        fixed.replace_range(range, &replacement);
    }
    (fixed != original).then_some(fixed)
}

pub(crate) fn base_name(function_text: &str) -> &str {
    let without_template = function_text.split('<').next().unwrap_or(function_text);
    without_template
        .rsplit("::")
        .next()
        .unwrap_or(without_template)
        .trim()
}
