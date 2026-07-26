use crate::categories;
use crate::categories::Category;
use crate::cleanse::{
    CleansedLines, find_next_multiline_comment_end, find_next_multiline_comment_start,
    remove_multiline_comments_from_range,
};
use crate::diagnostics::FileId;
use crate::errors::Result;
use crate::facts::FileFacts;
use crate::options::Options;
use crate::registry::{ActiveRulePlan, RulePhase, RuleRegistry, rule_registry};
use crate::source::{DecodedSource, SourceFile};
use crate::state::CppLintState;
use crate::string_utils;
use crate::suppressions::{ErrorSuppressions, SuppressionKey};
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use regex::{Regex, RegexSet};
use std::iter::ExactSizeIterator;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

static NOLINT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\bNOLINT(NEXTLINE|BEGIN|END)?\b(\([^)]+\))?"#).unwrap());
static FILE_TYPE_RE_SET: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r#"\b(?:LINT_C_FILE|vim?:\s*.*(\s*|:)filetype=c(\s*|:|$))"#,
        r#"\b(?:LINT_KERNEL_FILE)"#,
    ])
    .unwrap()
});

pub struct FileLinter<'a> {
    session: &'a CppLintState,
    options: Arc<Options>,
    active_rules: ActiveRulePlan,
    error_suppressions: ErrorSuppressions,
    file_id: FileId,
    source_file: SourceFile,
    registry: &'static RuleRegistry,
    has_error: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProcessMode {
    Full,
    LanguageRules,
}

impl<'a> FileLinter<'a> {
    pub fn new(
        file_path: PathBuf,
        state: &'a CppLintState,
        options: impl Into<Arc<Options>>,
    ) -> Self {
        let source_file = SourceFile::new(file_path);
        let file_id = state.register_file(source_file.display_name());
        Self::with_source_file(source_file, state, options, file_id)
    }

    pub(crate) fn with_file_id(
        file_path: PathBuf,
        state: &'a CppLintState,
        options: impl Into<Arc<Options>>,
        file_id: FileId,
    ) -> Self {
        Self::with_source_file(SourceFile::new(file_path), state, options, file_id)
    }

    fn with_source_file(
        source_file: SourceFile,
        state: &'a CppLintState,
        options: impl Into<Arc<Options>>,
        file_id: FileId,
    ) -> Self {
        let options = options.into();
        let registry = rule_registry();
        Self {
            session: state,
            active_rules: registry.active_rule_plan(options.as_ref(), source_file.display_name()),
            options,
            error_suppressions: ErrorSuppressions::new(),
            file_id,
            source_file,
            registry,
            has_error: false,
        }
    }

    #[inline]
    pub fn options(&self) -> &Options {
        self.options.as_ref()
    }

    #[inline]
    pub fn filename(&self) -> &str {
        self.source_file.display_name()
    }

    #[inline]
    pub fn file_path(&self) -> &Path {
        self.source_file.path()
    }

    #[inline]
    pub fn file_id(&self) -> FileId {
        self.file_id
    }

    #[inline]
    pub fn has_error(&self) -> bool {
        self.has_error
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn process_file(&mut self) -> Result<()> {
        let arena = Bump::new();
        self.process_file_with_arena(&arena)
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn process_file_with_arena(&mut self, arena: &Bump) -> Result<()> {
        let source_file = self.source_file.clone();
        source_file.with_decoded_source(arena, |decoded| {
            self.process_decoded_source(decoded, arena, ProcessMode::Full);
            Ok(())
        })
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn process_file_data<I, S>(&mut self, lines: I)
    where
        I: IntoIterator<Item = S>,
        I::IntoIter: ExactSizeIterator,
        S: AsRef<str>,
    {
        let arena = Bump::new();
        let decoded = DecodedSource::from_lines(&arena, self.source_file.clone(), lines);
        self.process_decoded_source(decoded, &arena, ProcessMode::Full);
    }

    pub fn process_language_rules_data<I, S>(&mut self, lines: I)
    where
        I: IntoIterator<Item = S>,
        I::IntoIter: ExactSizeIterator,
        S: AsRef<str>,
    {
        let arena = Bump::new();
        let decoded = DecodedSource::from_lines(&arena, self.source_file.clone(), lines);
        self.process_decoded_source(decoded, &arena, ProcessMode::LanguageRules);
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    fn process_decoded_source(&mut self, decoded: DecodedSource, arena: &Bump, mode: ProcessMode) {
        if self.active_rules.has_readability() {
            for &linenum in decoded.invalid_utf8_lines() {
                self.error(
                    linenum,
                    Category::ReadabilityUtf8,
                    5,
                    crate::messages::LintMessage::InvalidUtf8,
                );
            }
            for &linenum in decoded.null_lines() {
                self.error(
                    linenum,
                    Category::ReadabilityNul,
                    5,
                    crate::messages::LintMessage::NulByte,
                );
            }
        }

        let report_mixed_line_endings = self.active_rules.has_whitespace();
        let mixed_line_endings = report_mixed_line_endings && decoded.has_mixed_line_endings();
        let crlf_lines = if report_mixed_line_endings {
            decoded.crlf_lines().to_vec()
        } else {
            Vec::new()
        };
        self.process_source_lines(decoded.into_lines(), arena, mode);

        if mixed_line_endings {
            for linenum in crlf_lines {
                self.error(
                    linenum,
                    Category::WhitespaceNewline,
                    1,
                    crate::messages::LintMessage::MixedLineEndings,
                );
            }
        }
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    fn process_source_lines<'b>(
        &mut self,
        mut lines: BumpVec<'b, &'b str>,
        arena: &'b Bump,
        mode: ProcessMode,
    ) {
        let registry = self.registry;
        let active_rules = self.active_rules;
        if matches!(mode, ProcessMode::Full) {
            registry.run_raw_source(self, &lines, active_rules);

            if active_rules.needs_global_suppressions() {
                for &line in &lines {
                    if line.contains("LINT") || line.contains("filetype") {
                        self.process_global_suppressions(line);
                    }
                }
            }
        }

        let phase = match mode {
            ProcessMode::Full => {
                if active_rules.has_line_checks() {
                    RulePhase::Line
                } else if active_rules.has_headers() {
                    RulePhase::FileStructure
                } else if active_rules.has_whitespace() {
                    RulePhase::Finalize
                } else {
                    RulePhase::RawSource
                }
            }
            ProcessMode::LanguageRules => RulePhase::FileStructure,
        };
        if !active_rules.needs_cleansed_lines(phase) {
            return;
        }

        self.remove_multiline_comments(lines.as_mut_slice());

        let clean_lines = CleansedLines::new_with_options(
            arena,
            lines.as_slice(),
            self.options.as_ref(),
            self.filename(),
        );
        match mode {
            ProcessMode::Full => registry.run_file_structure(self, &clean_lines, active_rules),
            ProcessMode::LanguageRules => {
                registry.run_language_rules(self, &clean_lines, active_rules);
            }
        }

        if active_rules.has_line_checks() {
            let facts = FileFacts::new(&clean_lines, arena);
            for linenum in 0..clean_lines.raw_lines.len() {
                self.process_line(&clean_lines, &facts, linenum);
            }
        }

        if matches!(mode, ProcessMode::Full) {
            if active_rules.has_line_checks()
                && let Some(begin) = self.error_suppressions.get_open_block_start()
            {
                self.error(
                    begin,
                    Category::ReadabilityNolint,
                    5,
                    crate::messages::LintMessage::NolintBlockNeverEnded,
                );
            }

            registry.run_finalize(self, &clean_lines.raw_lines, active_rules);
        }
    }

    pub fn relative_from_repository(&self) -> PathBuf {
        relative_from_repository(self.file_path(), &self.options.repository)
    }

    pub fn relative_from_root(&self) -> PathBuf {
        relative_from_subdir(&self.relative_from_repository(), &self.options.root)
    }

    pub fn header_guard_path(&self) -> PathBuf {
        let normalized = self
            .relative_from_root()
            .to_string_lossy()
            .replace("C++", "cpp")
            .replace("c++", "cpp");
        PathBuf::from(normalized)
    }

    fn process_line(&mut self, clean_lines: &CleansedLines, facts: &FileFacts<'_>, linenum: usize) {
        let raw_line = &clean_lines.raw_lines[linenum];
        if clean_lines
            .has_comment
            .get(linenum)
            .copied()
            .unwrap_or(false)
        {
            self.parse_nolint_suppressions(raw_line, linenum);
        }
        let registry = self.registry;
        registry.run_line(self, facts, clean_lines, linenum, self.active_rules);
    }

    fn parse_nolint_suppressions(&mut self, raw_line: &str, linenum: usize) {
        if !raw_line.contains("NOLINT") {
            return;
        }
        let Some(captures) = NOLINT_RE.captures(raw_line) else {
            return;
        };
        let no_lint_type = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        let categories = captures.get(2).map(|m| m.as_str()).unwrap_or("");

        let process_category =
            |this: &mut FileLinter<'a>, category: SuppressionKey| match no_lint_type {
                "NEXTLINE" => this
                    .error_suppressions
                    .add_line_suppression(category, linenum + 1),
                "BEGIN" => this
                    .error_suppressions
                    .start_block_suppression(category, linenum),
                "END" => {
                    if let SuppressionKey::Category(category) = category {
                        this.error(
                            linenum,
                            Category::ReadabilityNolint,
                            5,
                            crate::messages::LintMessage::NolintCategoriesNotSupportedInEnd(
                                category.as_str().into(),
                            ),
                        );
                    }
                    this.error_suppressions.end_block_suppression(linenum);
                }
                _ => this
                    .error_suppressions
                    .add_line_suppression(category, linenum),
            };

        if no_lint_type == "BEGIN" && self.error_suppressions.has_open_block() {
            if let Some(begin) = self.error_suppressions.peek_open_block_start() {
                self.error(
                    linenum,
                    Category::ReadabilityNolint,
                    5,
                    crate::messages::LintMessage::NolintBlockAlreadyDefined(begin + 1),
                );
            }
        } else if no_lint_type == "END" && !self.error_suppressions.has_open_block() {
            self.error(
                linenum,
                Category::ReadabilityNolint,
                5,
                crate::messages::LintMessage::NotInNolintBlock,
            );
        }

        if categories.is_empty() || categories == "(*)" {
            process_category(self, SuppressionKey::All);
            return;
        }
        if !(categories.starts_with('(') && categories.ends_with(')')) {
            return;
        }

        let inner = &categories[1..categories.len() - 1];
        for category in string_utils::parse_comma_separated_list(inner) {
            if let Ok(category) = category.parse::<Category>() {
                process_category(self, SuppressionKey::Category(category));
            } else if !categories::is_other_nolint_category(&category)
                && !categories::is_legacy_error_category(&category)
            {
                self.error(
                    linenum,
                    Category::ReadabilityNolint,
                    5,
                    crate::messages::LintMessage::UnknownNolintCategory(
                        category.to_string().into(),
                    ),
                );
            }
        }
    }

    fn process_global_suppressions(&mut self, line: &str) {
        let matches = FILE_TYPE_RE_SET.matches(line);
        if matches.matched(0) {
            self.error_suppressions.add_default_c_suppressions();
        }
        if matches.matched(1) {
            self.error_suppressions.add_default_kernel_suppressions();
        }
    }

    fn remove_multiline_comments(&mut self, lines: &mut [&str]) {
        let mut lineix = 0usize;
        loop {
            let begin = find_next_multiline_comment_start(lines, lineix);
            if begin >= lines.len() {
                return;
            }
            let end = find_next_multiline_comment_end(lines, begin);
            if end >= lines.len() {
                self.error(
                    begin,
                    Category::ReadabilityMultilineComment,
                    5,
                    crate::messages::LintMessage::UnterminatedMultilineComment,
                );
                return;
            }

            remove_multiline_comments_from_range(lines, begin, end + 1);
            lineix = end + 1;
        }
    }

    pub fn error(
        &mut self,
        linenum: usize,
        category: Category,
        confidence: i32,
        message: crate::messages::LintMessage,
    ) {
        if self.error_suppressions.is_suppressed(category, linenum)
            || !self
                .options
                .should_print_error(category, self.filename(), linenum)
            || confidence < self.session.verbose_level()
        {
            return;
        }

        self.has_error = true;
        self.session
            .record_diagnostic(self.file_id, linenum, category, confidence, message);
    }

    pub fn error_display_line(
        &mut self,
        display_linenum: usize,
        category: Category,
        confidence: i32,
        message: crate::messages::LintMessage,
    ) {
        let filter_linenum = display_linenum.saturating_sub(1);
        if self
            .error_suppressions
            .is_suppressed(category, filter_linenum)
            || !self
                .options
                .should_print_error(category, self.filename(), filter_linenum)
            || confidence < self.session.verbose_level()
        {
            return;
        }

        self.has_error = true;
        self.session.record_diagnostic_display_line(
            self.file_id,
            display_linenum,
            category,
            confidence,
            message,
        );
    }
}

use rustc_hash::FxHashMap;
use std::cell::RefCell;

thread_local! {
    static VCS_ROOT_CACHE: RefCell<FxHashMap<PathBuf, PathBuf>> = RefCell::new(FxHashMap::default());
}

fn find_vcs_root(dir: &Path) -> PathBuf {
    VCS_ROOT_CACHE.with(|cache_cell| {
        let mut cache = cache_cell.borrow_mut();
        if let Some(root) = cache.get(dir) {
            return root.clone();
        }

        let mut current = dir;
        let mut project_root = current.to_path_buf();
        loop {
            if current.join(".git").exists()
                || current.join(".hg").exists()
                || current.join(".svn").exists()
            {
                project_root = current.to_path_buf();
                break;
            }
            let Some(parent) = current.parent() else {
                break;
            };
            if parent == current {
                break;
            }
            current = parent;
        }

        cache.insert(dir.to_path_buf(), project_root.clone());
        project_root
    })
}

fn relative_from_repository(file: &Path, repository: &Path) -> PathBuf {
    if file == Path::new("-") {
        return PathBuf::from("-");
    }

    if !repository.as_os_str().is_empty()
        && let (Ok(file_abs), Ok(repo_abs)) = (
            std::fs::canonicalize(file),
            std::fs::canonicalize(repository),
        )
        && let Ok(relative) = file_abs.strip_prefix(repo_abs)
    {
        return relative.to_path_buf();
    }

    let Ok(file_abs) = std::fs::canonicalize(file) else {
        return file.to_path_buf();
    };

    let parent_dir = file_abs.parent().unwrap_or(&file_abs);
    let project_root = find_vcs_root(parent_dir);

    file_abs
        .strip_prefix(project_root)
        .map(Path::to_path_buf)
        .unwrap_or(file_abs)
}

fn relative_from_subdir(file: &Path, subdir: &Path) -> PathBuf {
    if subdir.as_os_str().is_empty() {
        return file.to_path_buf();
    }

    if let Ok(relative) = file.strip_prefix(subdir) {
        return relative.to_path_buf();
    }

    if let (Ok(file_abs), Ok(subdir_abs)) =
        (std::fs::canonicalize(file), std::fs::canonicalize(subdir))
        && let Ok(relative) = file_abs.strip_prefix(subdir_abs)
    {
        return relative.to_path_buf();
    }

    file.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::Options;
    use crate::state::CppLintState;

    #[test]
    fn test_linter_integration() {
        let state = CppLintState::new();
        let options = Options::new();
        let mut linter = FileLinter::new(PathBuf::from("test.cpp"), &state, options);

        let lines = vec![
            "// Copyright 2026 Test".to_string(),
            "#include <iostream>".to_string(),
            "".to_string(),
            "int main()".to_string(),
            "{".to_string(),
            "  int x = (int)1.0;  // C-style cast".to_string(),
            "\treturn 0; // Tab character".to_string(),
            "}  ".to_string(),
        ];

        linter.process_file_data(lines);
        assert_eq!(state.error_count(), 6);
    }

    #[test]
    fn test_process_file_reports_crlf() {
        let state = CppLintState::new();
        let mut options = Options::new();
        options.add_filter("-legal/copyright");
        options.add_filter("-whitespace/ending_newline");

        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("cpplint_test_crlf.c");
        std::fs::write(&file_path, b"line1\r\nline2\n").unwrap();

        let mut linter = FileLinter::new(file_path.clone(), &state, options);
        linter.process_file().unwrap();

        let _ = std::fs::remove_file(file_path);

        assert_eq!(state.error_count(), 1);
        assert!(state.has_error(Category::WhitespaceNewline));
    }

    #[test]
    fn test_invalid_utf8() {
        let state = CppLintState::new();
        let mut options = Options::new();
        options.add_filter("-legal/copyright");
        options.add_filter("-whitespace/ending_newline");

        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("cpplint_test_invalid_utf8.c");
        // FF 0A FF 0A is invalid UTF-8 (two lines)
        std::fs::write(&file_path, b"\xff\n\xff\n").unwrap();

        let mut linter = FileLinter::new(file_path.clone(), &state, options);
        linter.process_file().unwrap();

        let _ = std::fs::remove_file(file_path);

        assert_eq!(state.error_count(), 2);
        assert!(state.has_error(Category::ReadabilityUtf8));
    }

    #[test]
    fn test_header_guard_path_honors_repository_and_root() {
        let state = CppLintState::new();
        let temp_dir = std::env::temp_dir().join(format!(
            "cpplint_test_header_guard_path_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let repo_dir = temp_dir.join("trunk");
        let header_dir = repo_dir.join("cpplint");
        std::fs::create_dir_all(temp_dir.join(".git")).unwrap();
        std::fs::create_dir_all(&header_dir).unwrap();
        let file_path = header_dir.join("cpplint_test_header.h");
        std::fs::write(&file_path, "").unwrap();

        let linter = FileLinter::new(file_path.clone(), &state, Options::new());
        assert_eq!(
            linter.header_guard_path(),
            PathBuf::from("trunk/cpplint/cpplint_test_header.h")
        );

        let mut options = Options::new();
        options.repository = repo_dir.clone();
        let linter = FileLinter::new(file_path.clone(), &state, options.clone());
        assert_eq!(
            linter.header_guard_path(),
            PathBuf::from("cpplint/cpplint_test_header.h")
        );

        options.root = PathBuf::from("cpplint");
        let linter = FileLinter::new(file_path, &state, options);
        assert_eq!(
            linter.header_guard_path(),
            PathBuf::from("cpplint_test_header.h")
        );

        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_bad_characters() {
        let state = CppLintState::new();
        let mut options = Options::new();
        options.add_filter("-legal/copyright");
        options.add_filter("-whitespace/ending_newline");

        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("cpplint_test_bad_characters.c");
        std::fs::write(&file_path, b"// Copyright 2026 Your Company.\n\xe9x\0\n").unwrap();

        let mut linter = FileLinter::new(file_path.clone(), &state, options);
        linter.process_file().unwrap();

        let _ = std::fs::remove_file(file_path);

        assert_eq!(state.error_count(), 2);
        assert!(state.has_error(Category::ReadabilityUtf8));
        assert!(state.has_error(Category::ReadabilityNul));
    }

    #[test]
    fn test_remove_multiline_comments_replaces_full_comment_blocks() {
        let state = CppLintState::new();
        let mut options = Options::new();
        options.add_filter("-legal/copyright");
        options.add_filter("-whitespace/ending_newline");
        let mut linter = FileLinter::new(PathBuf::from("test.cpp"), &state, options);
        let mut lines = vec!["/* This should be removed", "", "*/"];

        linter.remove_multiline_comments(&mut lines);

        assert_eq!(lines, vec!["/**/", "/**/", "/**/"]);
        assert_eq!(state.error_count(), 0);
    }

    #[test]
    fn test_remove_multiline_comments_reports_unterminated_comment() {
        let state = CppLintState::new();
        let mut options = Options::new();
        options.add_filter("-legal/copyright");
        options.add_filter("-whitespace/ending_newline");
        let mut linter = FileLinter::new(PathBuf::from("test.cpp"), &state, options);
        let mut lines = vec!["/* This should be removed", ""];

        linter.remove_multiline_comments(&mut lines);

        assert_eq!(state.error_count(), 1);
        assert!(state.has_error(Category::ReadabilityMultilineComment));
    }

    #[test]
    fn test_process_language_rules_data_reports_unnamed_namespace() {
        let state = CppLintState::new();
        let options = Options::new();
        let mut linter = FileLinter::new(PathBuf::from("foo.h"), &state, options);
        let lines = vec![
            "// Copyright 2026".to_string(),
            "namespace {".to_string(),
            "".to_string(),
        ];

        linter.process_language_rules_data(lines);

        assert!(state.has_error(Category::BuildNamespacesHeaders));
    }

    #[test]
    fn test_process_language_rules_data_ignores_unnamed_namespace_in_non_headers() {
        let state = CppLintState::new();
        let options = Options::new();
        let mut linter = FileLinter::new(PathBuf::from("foo.cc"), &state, options);
        let lines = vec![
            "// Copyright 2026".to_string(),
            "namespace {".to_string(),
            "".to_string(),
        ];

        linter.process_language_rules_data(lines);

        assert_eq!(state.error_count(), 0);
    }

    #[test]
    fn test_process_language_rules_data_handles_include_regression_cases() {
        let state = CppLintState::new();
        let options = Options::new();
        let mut linter = FileLinter::new(PathBuf::from("foo/foo.cc"), &state, options);

        let format_includes = |includes: &[&str]| -> Vec<String> {
            let mut lines = vec!["// Copyright 2026".to_string()];
            let mut include_block = String::new();
            for item in includes {
                if item.starts_with('"') || item.starts_with('<') {
                    include_block.push_str(&format!("#include {item}\n"));
                } else {
                    include_block.push_str(item);
                    include_block.push('\n');
                }
            }
            lines.push(include_block);
            lines.push(String::new());
            lines
        };

        linter.process_language_rules_data(format_includes(&["\"foo/foo.h\""]));
        assert_eq!(state.error_count(), 0);

        linter.process_language_rules_data(format_includes(&[
            "\"foo/foo.h\"",
            "\"foo/foo-inl.h\"",
            "<stdio.h>",
            "<string>",
            "<unordered_map>",
            "\"bar/bar-inl.h\"",
            "\"bar/bar.h\"",
        ]));
        assert_eq!(state.error_count(), 0);
    }
}
