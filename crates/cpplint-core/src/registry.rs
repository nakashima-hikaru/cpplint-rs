use crate::categories;
use crate::checks::{copyright, headers, readability, runtime, whitespace};
use crate::cleanse::CleansedLines;
use crate::file_linter::FileLinter;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulePhase {
    RawSource,
    FileStructure,
    Line,
    Finalize,
}

impl RulePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            RulePhase::RawSource => "raw-source",
            RulePhase::FileStructure => "file-structure",
            RulePhase::Line => "line",
            RulePhase::Finalize => "finalize",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleFamily {
    pub name: &'static str,
    pub summary: &'static str,
    pub phase: RulePhase,
    pub categories: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSelection {
    Family(RuleFamily),
    Category {
        category: &'static str,
        family: RuleFamily,
    },
}

pub struct RuleRegistry {
    families: &'static [RuleFamily],
}

const RULE_FAMILIES: &[RuleFamily] = &[
    RuleFamily {
        name: "copyright",
        summary: "Checks top-of-file copyright boilerplate before deeper analysis.",
        phase: RulePhase::RawSource,
        categories: &["legal/copyright"],
    },
    RuleFamily {
        name: "headers",
        summary: "Validates header guards, include ordering, and include hygiene.",
        phase: RulePhase::FileStructure,
        categories: &[
            "build/header_guard",
            "build/include",
            "build/include_subdir",
            "build/include_alpha",
            "build/include_order",
            "build/include_what_you_use",
        ],
    },
    RuleFamily {
        name: "whitespace",
        summary: "Applies token, indentation, newline, and formatting-oriented checks.",
        phase: RulePhase::Line,
        categories: &[
            "whitespace/blank_line",
            "whitespace/braces",
            "whitespace/comma",
            "whitespace/comments",
            "whitespace/empty_conditional_body",
            "whitespace/empty_if_body",
            "whitespace/empty_loop_body",
            "whitespace/end_of_line",
            "whitespace/ending_newline",
            "whitespace/forcolon",
            "whitespace/indent",
            "whitespace/indent_namespace",
            "whitespace/line_length",
            "whitespace/newline",
            "whitespace/operators",
            "whitespace/parens",
            "whitespace/semicolon",
            "whitespace/tab",
            "whitespace/todo",
        ],
    },
    RuleFamily {
        name: "runtime",
        summary: "Catches runtime hazards and discouraged C/C++ constructs.",
        phase: RulePhase::Line,
        categories: &[
            "runtime/arrays",
            "runtime/casting",
            "runtime/explicit",
            "runtime/int",
            "runtime/init",
            "runtime/invalid_increment",
            "runtime/member_string_references",
            "runtime/memset",
            "runtime/operator",
            "runtime/printf",
            "runtime/printf_format",
            "runtime/references",
            "runtime/string",
            "runtime/threadsafe_fn",
            "runtime/vlog",
        ],
    },
    RuleFamily {
        name: "readability",
        summary: "Enforces readability, maintainability, and style signal checks.",
        phase: RulePhase::Line,
        categories: &[
            "build/c++11",
            "build/c++17",
            "build/deprecated",
            "build/endif_comment",
            "build/explicit_make_pair",
            "build/forward_decl",
            "build/namespaces_headers",
            "build/namespaces_literals",
            "build/namespaces",
            "build/printf_format",
            "build/storage_class",
            "readability/alt_tokens",
            "readability/braces",
            "readability/casting",
            "readability/check",
            "readability/constructors",
            "readability/fn_size",
            "readability/inheritance",
            "readability/multiline_comment",
            "readability/multiline_string",
            "readability/namespace",
            "readability/nolint",
            "readability/nul",
            "readability/strings",
            "readability/todo",
            "readability/utf8",
        ],
    },
];

pub fn rule_registry() -> &'static RuleRegistry {
    static REGISTRY: LazyLock<RuleRegistry> = LazyLock::new(|| RuleRegistry {
        families: RULE_FAMILIES,
    });
    &REGISTRY
}

impl RuleRegistry {
    pub fn families(&self) -> &'static [RuleFamily] {
        self.families
    }

    pub fn family_by_name(&self, name: &str) -> Option<RuleFamily> {
        self.families
            .iter()
            .copied()
            .find(|family| family.name == name)
    }

    pub fn family_for_category(&self, category: &str) -> Option<RuleFamily> {
        self.families
            .iter()
            .copied()
            .find(|family| family.categories.contains(&category))
    }

    pub fn select(&self, query: &str) -> Option<RuleSelection> {
        if let Some(family) = self.family_by_name(query) {
            return Some(RuleSelection::Family(family));
        }

        self.family_for_category(query).and_then(|family| {
            family
                .categories
                .iter()
                .copied()
                .find(|candidate| *candidate == query)
                .map(|category| RuleSelection::Category { category, family })
        })
    }

    pub fn humanize_category(&self, category: &str) -> String {
        category
            .split('/')
            .nth(1)
            .unwrap_or(category)
            .replace('_', " ")
    }

    pub fn all_categories(&self) -> &'static [&'static str] {
        categories::ERROR_CATEGORIES
    }

    pub fn run_raw_source<S: AsRef<str>>(&self, linter: &mut FileLinter<'_>, raw_lines: &[S]) {
        copyright::check(linter, raw_lines);
    }

    pub fn run_file_structure(&self, linter: &mut FileLinter<'_>, clean_lines: &CleansedLines<'_>) {
        headers::check_header_guard(linter, clean_lines);
        headers::check_includes(linter, clean_lines);
    }

    pub fn run_line(
        &self,
        linter: &mut FileLinter<'_>,
        clean_lines: &CleansedLines<'_>,
        linenum: usize,
    ) {
        whitespace::check(linter, clean_lines, linenum);
        runtime::check(linter, clean_lines, linenum);
        readability::check(linter, clean_lines, linenum);
    }

    pub fn run_finalize<S: AsRef<str>>(&self, linter: &mut FileLinter<'_>, raw_lines: &[S]) {
        whitespace::check_eof_newline(linter, raw_lines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_resolves_families_and_categories() {
        let registry = rule_registry();

        let family = registry.family_by_name("whitespace").unwrap();
        assert_eq!(family.phase, RulePhase::Line);
        assert!(family.categories.contains(&"whitespace/operators"));

        let selection = registry.select("build/header_guard").unwrap();
        assert_eq!(
            selection,
            RuleSelection::Category {
                category: "build/header_guard",
                family: registry.family_by_name("headers").unwrap(),
            }
        );
        assert_eq!(
            registry.humanize_category("whitespace/empty_loop_body"),
            "empty loop body"
        );
        assert!(registry.all_categories().contains(&"runtime/casting"));
    }

    #[test]
    fn registry_covers_every_supported_category() {
        let registry = rule_registry();

        for &category in registry.all_categories() {
            assert!(
                registry.family_for_category(category).is_some(),
                "missing rule family for category {category}"
            );
        }
    }
}
