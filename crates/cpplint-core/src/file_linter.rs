use crate::categories;
use crate::categories::Category;
use crate::cleanse::CleansedLines;
use crate::errors::Result;
use crate::facts::FileFacts;
use crate::options::Options;
use crate::registry::{RuleRegistry, rule_registry};
use crate::source::{DecodedSource, SourceFile};
use crate::state::CppLintState;
use crate::string_utils;
use crate::suppressions::ErrorSuppressions;
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
    error_suppressions: ErrorSuppressions,
    file_index: usize,
    source_file: SourceFile,
    registry: &'static RuleRegistry,
    facts: Option<FileFacts>,
    has_error: bool,
}

impl<'a> FileLinter<'a> {
    pub fn new(
        file_path: PathBuf,
        state: &'a CppLintState,
        options: impl Into<Arc<Options>>,
    ) -> Self {
        Self::with_index(file_path, state, options, 0)
    }

    pub fn with_index(
        file_path: PathBuf,
        state: &'a CppLintState,
        options: impl Into<Arc<Options>>,
        file_index: usize,
    ) -> Self {
        Self {
            session: state,
            options: options.into(),
            error_suppressions: ErrorSuppressions::new(),
            file_index,
            source_file: SourceFile::new(file_path),
            registry: rule_registry(),
            facts: None,
            has_error: false,
        }
    }

    pub fn options(&self) -> &Options {
        self.options.as_ref()
    }

    pub fn filename(&self) -> &str {
        self.source_file.display_name()
    }

    pub fn file_path(&self) -> &Path {
        self.source_file.path()
    }

    pub fn file_index(&self) -> usize {
        self.file_index
    }

    pub fn has_error(&self) -> bool {
        self.has_error
    }

    pub(crate) fn facts(&self) -> &FileFacts {
        self.facts
            .as_ref()
            .expect("file facts should be initialized before running checks")
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn process_file(&mut self) -> Result<()> {
        let arena = Bump::new();
        let decoded = self.source_file.read_into(&arena)?;
        self.process_decoded_source(decoded, &arena);
        Ok(())
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
        self.process_decoded_source(decoded, &arena);
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    fn process_decoded_source(&mut self, decoded: DecodedSource, arena: &Bump) {
        for &linenum in decoded.invalid_utf8_lines() {
            self.error(
                linenum,
                Category::ReadabilityUtf8,
                5,
                "Line contains invalid UTF-8 (or Unicode replacement character).",
            );
        }
        for &linenum in decoded.null_lines() {
            self.error(
                linenum,
                Category::ReadabilityNul,
                5,
                "Line contains NUL byte.",
            );
        }

        let mixed_line_endings = decoded.has_mixed_line_endings();
        let crlf_lines = decoded.crlf_lines().to_vec();
        self.process_source_lines(decoded.into_lines(), arena);

        if mixed_line_endings {
            for linenum in crlf_lines {
                self.error(
                    linenum,
                    Category::WhitespaceNewline,
                    1,
                    "Unexpected \\r (^M) found; better to use only \\n",
                );
            }
        }
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    fn process_source_lines<'b>(&mut self, mut lines: BumpVec<'b, &'b str>, arena: &'b Bump) {
        let registry = self.registry;
        registry.run_raw_source(self, &lines);

        for &line in &lines {
            self.process_global_suppressions(line);
        }

        self.remove_multiline_comments(lines.as_mut_slice());

        let clean_lines = CleansedLines::new_with_options(
            arena,
            lines.as_slice(),
            self.options.as_ref(),
            self.filename(),
        );
        self.facts = Some(FileFacts::new(&clean_lines));
        registry.run_file_structure(self, &clean_lines);

        for linenum in 0..clean_lines.raw_lines.len() {
            self.process_line(&clean_lines, linenum);
        }

        if let Some(begin) = self.error_suppressions.get_open_block_start() {
            self.error(
                begin,
                Category::ReadabilityNolint,
                5,
                "NOLINT block never ended",
            );
        }

        registry.run_finalize(self, &clean_lines.raw_lines);
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

    fn process_line(&mut self, clean_lines: &CleansedLines, linenum: usize) {
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
        registry.run_line(self, clean_lines, linenum);
    }

    fn parse_nolint_suppressions(&mut self, raw_line: &str, linenum: usize) {
        let Some(captures) = NOLINT_RE.captures(raw_line) else {
            return;
        };
        let no_lint_type = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        let categories = captures.get(2).map(|m| m.as_str()).unwrap_or("");

        let process_category = |this: &mut FileLinter<'a>, category: &str| match no_lint_type {
            "NEXTLINE" => this
                .error_suppressions
                .add_line_suppression(category, linenum + 1),
            "BEGIN" => this
                .error_suppressions
                .start_block_suppression(category, linenum),
            "END" => {
                if !category.is_empty() {
                    this.error(
                        linenum,
                        Category::ReadabilityNolint,
                        5,
                        &format!("NOLINT categories not supported in block END: {}", category),
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
                    &format!("NOLINT block already defined on line {}", begin + 1),
                );
            }
        } else if no_lint_type == "END" && !self.error_suppressions.has_open_block() {
            self.error(
                linenum,
                Category::ReadabilityNolint,
                5,
                "Not in a NOLINT block",
            );
        }

        if categories.is_empty() || categories == "(*)" {
            process_category(self, "");
            return;
        }
        if !(categories.starts_with('(') && categories.ends_with(')')) {
            return;
        }

        let inner = &categories[1..categories.len() - 1];
        for category in string_utils::parse_comma_separated_list(inner) {
            if categories::is_error_category(&category) {
                process_category(self, &category);
            } else if !categories::is_other_nolint_category(&category)
                && !categories::is_legacy_error_category(&category)
            {
                self.error(
                    linenum,
                    Category::ReadabilityNolint,
                    5,
                    &format!("Unknown NOLINT error category: {}", category),
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
        while let Some(begin) = find_next_multiline_comment_start(lines, lineix) {
            let Some(end) = find_next_multiline_comment_end(lines, begin) else {
                self.error(
                    begin,
                    Category::ReadabilityMultilineComment,
                    5,
                    "Could not find end of multi-line comment",
                );
                return;
            };
            if !lines[end].trim_end().ends_with("*/") {
                self.error(
                    end,
                    Category::ReadabilityMultilineComment,
                    5,
                    "Could not find end of multi-line comment",
                );
                return;
            }

            for line in lines.iter_mut().take(end + 1).skip(begin) {
                *line = "/**/";
            }
            lineix = end + 1;
        }
    }

    pub fn error(&mut self, linenum: usize, category: Category, confidence: i32, message: &str) {
        if self.error_suppressions.is_suppressed(category, linenum)
            || !self
                .options
                .should_print_error(category, self.filename(), linenum)
            || confidence < self.session.verbose_level()
        {
            return;
        }

        self.has_error = true;
        self.session.record_diagnostic(
            self.file_index,
            self.filename(),
            linenum,
            category,
            confidence,
            message,
        );
    }

    pub fn error_display_line(
        &mut self,
        display_linenum: usize,
        category: Category,
        confidence: i32,
        message: &str,
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
            self.file_index,
            self.filename(),
            display_linenum,
            category,
            confidence,
            message,
        );
    }
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

    let mut current = file_abs.parent().unwrap_or(file_abs.as_path());
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

fn find_next_multiline_comment_start(lines: &[&str], start: usize) -> Option<usize> {
    for (idx, line) in lines.iter().enumerate().skip(start) {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("/*") {
            continue;
        }
        if trimmed[2..].contains("*/") {
            continue;
        }
        return Some(idx);
    }
    None
}

fn find_next_multiline_comment_end(lines: &[&str], start: usize) -> Option<usize> {
    for (idx, line) in lines.iter().enumerate().skip(start) {
        if line.contains("*/") {
            return Some(idx);
        }
    }
    None
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
    fn test_process_file_reports_invalid_utf8() {
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
}
