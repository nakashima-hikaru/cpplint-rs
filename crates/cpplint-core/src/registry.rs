use crate::categories;
use crate::categories::Category;
use crate::checks::{copyright, headers, readability, runtime, whitespace};
use crate::cleanse::CleansedLines;
use crate::facts::FileFacts;
use crate::file_linter::FileLinter;
use crate::options::Options;
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

const FAMILY_COPYRIGHT: u8 = 1 << 0;
const FAMILY_HEADERS: u8 = 1 << 1;
const FAMILY_WHITESPACE: u8 = 1 << 2;
const FAMILY_RUNTIME: u8 = 1 << 3;
const FAMILY_READABILITY: u8 = 1 << 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActiveRulePlan {
    enabled_families: u8,
}

impl ActiveRulePlan {
    fn enable(&mut self, family: RuleFamily) {
        self.enabled_families |= family_mask(family.name);
    }

    pub fn has_any(self) -> bool {
        self.enabled_families != 0
    }

    pub fn has_copyright(self) -> bool {
        self.enabled_families & FAMILY_COPYRIGHT != 0
    }

    pub fn has_headers(self) -> bool {
        self.enabled_families & FAMILY_HEADERS != 0
    }

    pub fn has_whitespace(self) -> bool {
        self.enabled_families & FAMILY_WHITESPACE != 0
    }

    pub fn has_runtime(self) -> bool {
        self.enabled_families & FAMILY_RUNTIME != 0
    }

    pub fn has_readability(self) -> bool {
        self.enabled_families & FAMILY_READABILITY != 0
    }

    pub fn has_line_checks(self) -> bool {
        self.has_whitespace() || self.has_runtime() || self.has_readability()
    }

    pub fn needs_cleansed_lines(self, phase: RulePhase) -> bool {
        match phase {
            RulePhase::RawSource => false,
            RulePhase::FileStructure => self.has_headers(),
            RulePhase::Line => self.has_line_checks(),
            RulePhase::Finalize => self.has_whitespace(),
        }
    }

    pub fn needs_global_suppressions(self) -> bool {
        self.has_headers() || self.has_line_checks() || self.has_whitespace()
    }
}

fn family_mask(name: &str) -> u8 {
    match name {
        "copyright" => FAMILY_COPYRIGHT,
        "headers" => FAMILY_HEADERS,
        "whitespace" => FAMILY_WHITESPACE,
        "runtime" => FAMILY_RUNTIME,
        "readability" => FAMILY_READABILITY,
        _ => 0,
    }
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

    pub fn active_rule_plan(&self, options: &Options, filename: &str) -> ActiveRulePlan {
        let mut plan = ActiveRulePlan::default();
        for &family in self.families {
            let family_active = family.categories.iter().any(|category| {
                category.parse::<Category>().ok().is_some_and(|category| {
                    options.can_print_error_for_some_line(category, filename)
                })
            });
            if family_active {
                plan.enable(family);
            }
        }
        plan
    }

    pub fn run_raw_source<S: AsRef<str>>(
        &self,
        linter: &mut FileLinter<'_>,
        raw_lines: &[S],
        active_rules: ActiveRulePlan,
    ) {
        if active_rules.has_copyright() {
            copyright::check(linter, raw_lines);
        }
    }

    pub fn run_file_structure(
        &self,
        linter: &mut FileLinter<'_>,
        clean_lines: &CleansedLines<'_>,
        active_rules: ActiveRulePlan,
    ) {
        if !active_rules.has_headers() {
            return;
        }
        headers::check_header_guard(linter, clean_lines);
        headers::check_includes(linter, clean_lines);
    }

    pub fn run_language_rules(
        &self,
        linter: &mut FileLinter<'_>,
        clean_lines: &CleansedLines<'_>,
        active_rules: ActiveRulePlan,
    ) {
        if active_rules.has_headers() {
            headers::check_includes(linter, clean_lines);
        }
    }

    pub fn run_line(
        &self,
        linter: &mut FileLinter<'_>,
        facts: &FileFacts<'_>,
        clean_lines: &CleansedLines<'_>,
        linenum: usize,
        active_rules: ActiveRulePlan,
    ) {
        if active_rules.has_whitespace() {
            whitespace::check(linter, facts, clean_lines, linenum);
        }
        if active_rules.has_runtime() {
            runtime::check(linter, facts, clean_lines, linenum);
        }
        if active_rules.has_readability() {
            readability::check(linter, facts, clean_lines, linenum);
        }
    }

    pub fn run_finalize<S: AsRef<str>>(
        &self,
        linter: &mut FileLinter<'_>,
        raw_lines: &[S],
        active_rules: ActiveRulePlan,
    ) {
        if active_rules.has_whitespace() {
            whitespace::check_eof_newline(linter, raw_lines);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::Options;

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

    #[test]
    fn active_rule_plan_tracks_filtered_families() {
        let registry = rule_registry();
        let mut options = Options::new();
        options.add_filter("-");
        options.add_filter("+whitespace");
        options.add_filter("+runtime/printf:test.cc:14");

        let test_file_plan = registry.active_rule_plan(&options, "test.cc");
        assert!(test_file_plan.has_whitespace());
        assert!(test_file_plan.has_runtime());
        assert!(!test_file_plan.has_headers());
        assert!(!test_file_plan.has_readability());
        assert!(!test_file_plan.has_copyright());

        let other_file_plan = registry.active_rule_plan(&options, "other.cc");
        assert!(other_file_plan.has_whitespace());
        assert!(!other_file_plan.has_runtime());
    }
}
