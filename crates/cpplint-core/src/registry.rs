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

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct RuleRequirements: u8 {
        const RAW_SOURCE     = 1 << 0;
        const CLEANSED_LINES = 1 << 1;
        const FILE_FACTS     = 1 << 2;
        const HEADER_PATH    = 1 << 3;
        const SYNTAX_TREE    = 1 << 4;
    }
}

fn category_requirements(cat: Category) -> RuleRequirements {
    match cat {
        Category::LegalCopyright => RuleRequirements::RAW_SOURCE,
        Category::WhitespaceLineLength
        | Category::WhitespaceEndingNewline
        | Category::WhitespaceTab
        | Category::WhitespaceEndOfLine => RuleRequirements::RAW_SOURCE,

        Category::BuildHeaderGuard
        | Category::BuildInclude
        | Category::BuildIncludeSubdir
        | Category::BuildIncludeAlpha
        | Category::BuildIncludeOrder
        | Category::BuildIncludeWhatYouUse => {
            RuleRequirements::RAW_SOURCE
                | RuleRequirements::CLEANSED_LINES
                | RuleRequirements::HEADER_PATH
        }

        Category::WhitespaceBlankLine
        | Category::WhitespaceBraces
        | Category::WhitespaceIndent
        | Category::WhitespaceIndentNamespace
        | Category::ReadabilityNamespace
        | Category::ReadabilityFnSize
        | Category::ReadabilityConstructors
        | Category::ReadabilityBraces
        | Category::RuntimeExplicit
        | Category::RuntimeInit
        | Category::BuildNamespaces
        | Category::BuildNamespacesHeaders => {
            RuleRequirements::RAW_SOURCE
                | RuleRequirements::CLEANSED_LINES
                | RuleRequirements::FILE_FACTS
        }

        _ => RuleRequirements::RAW_SOURCE | RuleRequirements::CLEANSED_LINES,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActiveRulePlan {
    pub categories: categories::CategorySet,
    pub requirements: RuleRequirements,
}

impl ActiveRulePlan {
    pub fn enable_category(&mut self, cat: Category) {
        self.categories.insert(cat);
        self.requirements |= category_requirements(cat);
    }

    #[inline]
    pub fn is_enabled(&self, cat: Category) -> bool {
        self.categories.contains(cat)
    }

    #[inline]
    pub fn has_any(&self) -> bool {
        !self.categories.is_empty()
    }

    #[inline]
    pub fn has_copyright(&self) -> bool {
        self.is_enabled(Category::LegalCopyright)
    }

    pub fn has_headers(&self) -> bool {
        self.is_enabled(Category::BuildHeaderGuard)
            || self.is_enabled(Category::BuildInclude)
            || self.is_enabled(Category::BuildIncludeSubdir)
            || self.is_enabled(Category::BuildIncludeAlpha)
            || self.is_enabled(Category::BuildIncludeOrder)
            || self.is_enabled(Category::BuildIncludeWhatYouUse)
    }

    pub fn has_whitespace(&self) -> bool {
        self.categories.contains(Category::WhitespaceBlankLine)
            || self.categories.contains(Category::WhitespaceBraces)
            || self.categories.contains(Category::WhitespaceComma)
            || self.categories.contains(Category::WhitespaceComments)
            || self.categories.contains(Category::WhitespaceEmptyConditionalBody)
            || self.categories.contains(Category::WhitespaceEmptyIfBody)
            || self.categories.contains(Category::WhitespaceEmptyLoopBody)
            || self.categories.contains(Category::WhitespaceEndOfLine)
            || self.categories.contains(Category::WhitespaceEndingNewline)
            || self.categories.contains(Category::WhitespaceForcolon)
            || self.categories.contains(Category::WhitespaceIndent)
            || self.categories.contains(Category::WhitespaceIndentNamespace)
            || self.categories.contains(Category::WhitespaceLineLength)
            || self.categories.contains(Category::WhitespaceNewline)
            || self.categories.contains(Category::WhitespaceOperators)
            || self.categories.contains(Category::WhitespaceParens)
            || self.categories.contains(Category::WhitespaceSemicolon)
            || self.categories.contains(Category::WhitespaceTab)
            || self.categories.contains(Category::WhitespaceTodo)
    }

    pub fn has_runtime(&self) -> bool {
        self.categories.contains(Category::RuntimeArrays)
            || self.categories.contains(Category::RuntimeCasting)
            || self.categories.contains(Category::RuntimeExplicit)
            || self.categories.contains(Category::RuntimeInt)
            || self.categories.contains(Category::RuntimeInit)
            || self.categories.contains(Category::RuntimeInvalidIncrement)
            || self.categories.contains(Category::RuntimeMemberStringReferences)
            || self.categories.contains(Category::RuntimeMemset)
            || self.categories.contains(Category::RuntimeOperator)
            || self.categories.contains(Category::RuntimePrintf)
            || self.categories.contains(Category::RuntimePrintfFormat)
            || self.categories.contains(Category::RuntimeReferences)
            || self.categories.contains(Category::RuntimeString)
            || self.categories.contains(Category::RuntimeThreadsafeFn)
            || self.categories.contains(Category::RuntimeVlog)
    }

    pub fn has_readability(&self) -> bool {
        self.categories.contains(Category::ReadabilityAltTokens)
            || self.categories.contains(Category::ReadabilityBraces)
            || self.categories.contains(Category::ReadabilityCasting)
            || self.categories.contains(Category::ReadabilityCheck)
            || self.categories.contains(Category::ReadabilityConstructors)
            || self.categories.contains(Category::ReadabilityFnSize)
            || self.categories.contains(Category::ReadabilityInheritance)
            || self.categories.contains(Category::ReadabilityMultilineComment)
            || self.categories.contains(Category::ReadabilityMultilineString)
            || self.categories.contains(Category::ReadabilityNamespace)
            || self.categories.contains(Category::ReadabilityNolint)
            || self.categories.contains(Category::ReadabilityNul)
            || self.categories.contains(Category::ReadabilityStrings)
            || self.categories.contains(Category::ReadabilityTodo)
            || self.categories.contains(Category::ReadabilityUtf8)
    }

    pub fn has_line_checks(&self) -> bool {
        self.has_any()
    }

    pub fn needs_cleansed_lines(&self, _phase: RulePhase) -> bool {
        self.requirements.contains(RuleRequirements::CLEANSED_LINES)
    }

    pub fn needs_file_facts(&self) -> bool {
        self.requirements.contains(RuleRequirements::FILE_FACTS)
    }

    pub fn needs_global_suppressions(&self) -> bool {
        self.has_any()
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
            "build/include_alpha",
            "build/include_order",
            "build/include_subdir",
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
            "runtime/init",
            "runtime/int",
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
            "build/namespaces",
            "build/namespaces_headers",
            "build/namespaces_literals",
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

impl Default for RuleRegistry {
    fn default() -> Self {
        Self {
            families: RULE_FAMILIES,
        }
    }
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
            .find(|family| family.categories.binary_search(&category).is_ok())
    }

    pub fn select(&self, query: &str) -> Option<RuleSelection> {
        if let Some(family) = self.family_by_name(query) {
            return Some(RuleSelection::Family(family));
        }

        self.family_for_category(query).and_then(|family| {
            family
                .categories
                .binary_search(&query)
                .ok()
                .map(|idx| RuleSelection::Category {
                    category: family.categories[idx],
                    family,
                })
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
        for &category in Category::ALL {
            if options.can_print_error_for_some_line(category, filename) {
                plan.enable_category(category);
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
            whitespace::check(linter, facts, clean_lines, linenum, active_rules);
        }
        if active_rules.has_runtime() {
            runtime::check(linter, facts, clean_lines, linenum, active_rules);
        }
        if active_rules.has_readability() {
            readability::check(linter, facts, clean_lines, linenum, active_rules);
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
