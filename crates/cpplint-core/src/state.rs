use crate::diagnostics::{Diagnostic, Note, NoteStream, ProcessedFile};
use parking_lot::Mutex;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Emacs,
    Vs7,
    Eclipse,
    JUnit,
    Sed,
    Gsed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CountingStyle {
    #[default]
    Total,
    Toplevel,
    Detailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncludeKind {
    CSystem,
    CppSystem,
    OtherSystem,
    LikelyMyHeader,
    PossibleMyHeader,
    OtherHeader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum IncludeSection {
    Initial,
    MyHeader,
    CSystem,
    CppSystem,
    OtherSystem,
    OtherHeader,
}

#[derive(Debug)]
pub struct IncludeState {
    section: IncludeSection,
    last_header: String,
    include_list: Vec<Vec<(String, usize)>>,
}

impl Default for IncludeState {
    fn default() -> Self {
        Self::new()
    }
}

impl IncludeState {
    pub fn new() -> Self {
        let mut state = Self {
            section: IncludeSection::Initial,
            last_header: String::new(),
            include_list: vec![Vec::new()],
        };
        state.reset_section("");
        state
    }

    pub fn find_header(&self, header: &str) -> Option<usize> {
        self.include_list.iter().find_map(|section_list| {
            section_list
                .iter()
                .find_map(|(include, line)| (include == header).then_some(*line))
        })
    }

    pub fn reset_section(&mut self, directive: &str) {
        self.section = IncludeSection::Initial;
        self.last_header.clear();

        match directive {
            "if" | "ifdef" | "ifndef" => self.include_list.push(Vec::new()),
            "else" | "elif" => {
                if let Some(section_list) = self.include_list.last_mut() {
                    section_list.clear();
                }
            }
            _ => {}
        }
    }

    pub fn include_lists(&self) -> &[Vec<(String, usize)>] {
        &self.include_list
    }

    pub fn last_include_list_mut(&mut self) -> &mut Vec<(String, usize)> {
        self.include_list
            .last_mut()
            .expect("include list always has at least one section")
    }

    pub fn set_last_header(&mut self, header_path: &str) {
        self.last_header = header_path.to_string();
    }

    pub fn canonicalize_alphabetical_order(&self, header_path: &str) -> String {
        header_path
            .replace("-inl.h", ".h")
            .replace('-', "_")
            .to_ascii_lowercase()
    }

    pub fn is_in_alphabetical_order(
        &self,
        previous_line_is_include: bool,
        canonical_header: &str,
    ) -> bool {
        if self.last_header.as_str() > canonical_header && previous_line_is_include {
            return false;
        }
        true
    }

    pub fn check_next_include_order(&mut self, kind: IncludeKind) -> Option<String> {
        let type_name = match kind {
            IncludeKind::CSystem => "C system header",
            IncludeKind::CppSystem => "C++ system header",
            IncludeKind::OtherSystem => "other system header",
            IncludeKind::LikelyMyHeader => "header this file implements",
            IncludeKind::PossibleMyHeader => "header this file may implement",
            IncludeKind::OtherHeader => "other header",
        };
        let section_name = match self.section {
            IncludeSection::Initial => "... nothing. (This can't be an error.)",
            IncludeSection::MyHeader => "a header this file implements",
            IncludeSection::CSystem => "C system header",
            IncludeSection::CppSystem => "C++ system header",
            IncludeSection::OtherSystem => "other system header",
            IncludeSection::OtherHeader => "other header",
        };

        let error_message = format!("Found {} after {}", type_name, section_name);
        let last_section = self.section;
        self.section = match kind {
            IncludeKind::CSystem => {
                if self.section <= IncludeSection::CSystem {
                    IncludeSection::CSystem
                } else {
                    self.last_header.clear();
                    return Some(error_message);
                }
            }
            IncludeKind::CppSystem => {
                if self.section <= IncludeSection::CppSystem {
                    IncludeSection::CppSystem
                } else {
                    self.last_header.clear();
                    return Some(error_message);
                }
            }
            IncludeKind::OtherSystem => {
                if self.section <= IncludeSection::OtherSystem {
                    IncludeSection::OtherSystem
                } else {
                    self.last_header.clear();
                    return Some(error_message);
                }
            }
            IncludeKind::LikelyMyHeader => {
                if self.section <= IncludeSection::MyHeader {
                    IncludeSection::MyHeader
                } else {
                    IncludeSection::OtherHeader
                }
            }
            IncludeKind::PossibleMyHeader => {
                if self.section <= IncludeSection::MyHeader {
                    IncludeSection::MyHeader
                } else {
                    IncludeSection::OtherHeader
                }
            }
            IncludeKind::OtherHeader => IncludeSection::OtherHeader,
        };

        if last_section != self.section {
            self.last_header.clear();
        }
        None
    }
}

#[derive(Debug, Default)]
pub struct FunctionState {
    current_name: Option<String>,
    lines_in_function: usize,
}

impl FunctionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin(&mut self, name: &str) {
        self.current_name = Some(name.to_string());
        self.lines_in_function = 0;
    }

    pub fn count_line(&mut self) {
        if self.current_name.is_some() {
            self.lines_in_function += 1;
        }
    }

    pub fn end(&mut self) -> Option<(String, usize)> {
        let name = self.current_name.take()?;
        let lines = self.lines_in_function;
        self.lines_in_function = 0;
        Some((name, lines))
    }

    pub fn current_name(&self) -> Option<&str> {
        self.current_name.as_deref()
    }

    pub fn lines_in_function(&self) -> usize {
        self.lines_in_function
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionSettings {
    pub verbose_level: i32,
    pub counting_style: CountingStyle,
    pub quiet: bool,
    pub output_format: OutputFormat,
    pub num_threads: usize,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            verbose_level: 1,
            counting_style: CountingStyle::Total,
            quiet: false,
            output_format: OutputFormat::Emacs,
            num_threads: 1,
        }
    }
}

#[derive(Debug)]
pub struct LintSession {
    inner: Mutex<SessionInner>,
}

#[derive(Debug)]
struct SessionInner {
    settings: SessionSettings,
    error_count: usize,
    errors_by_category: BTreeMap<String, usize>,
    diagnostics: Vec<Diagnostic>,
    notes: Vec<Note>,
    processed_files: Vec<ProcessedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub error_count: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub notes: Vec<Note>,
    pub processed_files: Vec<ProcessedFile>,
}

impl LintSession {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(SessionInner {
                settings: SessionSettings::default(),
                error_count: 0,
                errors_by_category: BTreeMap::new(),
                diagnostics: Vec::new(),
                notes: Vec::new(),
                processed_files: Vec::new(),
            }),
        }
    }

    pub fn with_settings(settings: SessionSettings) -> Self {
        let session = Self::new();
        session.apply_settings(settings);
        session
    }

    pub fn apply_settings(&self, settings: SessionSettings) {
        self.inner.lock().settings = settings;
    }

    pub fn settings(&self) -> SessionSettings {
        self.inner.lock().settings
    }

    pub fn verbose_level(&self) -> i32 {
        self.settings().verbose_level
    }

    pub fn set_verbose_level(&self, level: i32) -> i32 {
        let mut inner = self.inner.lock();
        let last = inner.settings.verbose_level;
        inner.settings.verbose_level = level;
        last
    }

    pub fn quiet(&self) -> bool {
        self.settings().quiet
    }

    pub fn set_quiet(&self, quiet: bool) -> bool {
        let mut inner = self.inner.lock();
        let last = inner.settings.quiet;
        inner.settings.quiet = quiet;
        last
    }

    pub fn output_format(&self) -> OutputFormat {
        self.settings().output_format
    }

    pub fn set_output_format(&self, output_format: OutputFormat) -> OutputFormat {
        let mut inner = self.inner.lock();
        let last = inner.settings.output_format;
        inner.settings.output_format = output_format;
        last
    }

    pub fn counting_style(&self) -> CountingStyle {
        self.settings().counting_style
    }

    pub fn set_counting_style(&self, counting_style: CountingStyle) -> CountingStyle {
        let mut inner = self.inner.lock();
        let last = inner.settings.counting_style;
        inner.settings.counting_style = counting_style;
        last
    }

    pub fn num_threads(&self) -> usize {
        self.settings().num_threads
    }

    pub fn set_num_threads(&self, num_threads: usize) -> usize {
        let mut inner = self.inner.lock();
        let last = inner.settings.num_threads;
        inner.settings.num_threads = num_threads.max(1);
        last
    }

    pub fn error_count(&self) -> usize {
        self.inner.lock().error_count
    }

    pub fn increment_error_count(&self, category: crate::categories::Category) {
        let mut inner = self.inner.lock();
        inner.error_count += 1;
        *inner
            .errors_by_category
            .entry(category.to_string())
            .or_insert(0) += 1;
    }

    pub fn record_diagnostic(
        &self,
        file_index: usize,
        filename: &str,
        linenum: usize,
        category: crate::categories::Category,
        confidence: i32,
        message: &str,
    ) {
        let mut inner = self.inner.lock();
        inner.error_count += 1;
        *inner
            .errors_by_category
            .entry(category.to_string())
            .or_insert(0) += 1;
        inner.diagnostics.push(Diagnostic {
            file_index,
            filename: Arc::from(filename),
            linenum: linenum + 1,
            category,
            confidence,
            message: Arc::from(message),
        });
    }

    pub fn record_diagnostic_display_line(
        &self,
        file_index: usize,
        filename: &str,
        display_linenum: usize,
        category: crate::categories::Category,
        confidence: i32,
        message: &str,
    ) {
        let mut inner = self.inner.lock();
        inner.error_count += 1;
        *inner
            .errors_by_category
            .entry(category.to_string())
            .or_insert(0) += 1;
        inner.diagnostics.push(Diagnostic {
            file_index,
            filename: Arc::from(filename),
            linenum: display_linenum,
            category,
            confidence,
            message: Arc::from(message),
        });
    }

    pub fn record_info(&self, file_index: usize, order: usize, message: impl AsRef<str>) {
        self.record_note(
            file_index,
            order,
            NoteStream::Stdout,
            Arc::from(message.as_ref()),
        );
    }

    pub fn record_raw_error(&self, file_index: usize, order: usize, message: impl AsRef<str>) {
        self.record_note(
            file_index,
            order,
            NoteStream::Stderr,
            Arc::from(message.as_ref()),
        );
    }

    fn record_note(&self, file_index: usize, order: usize, stream: NoteStream, text: Arc<str>) {
        self.inner.lock().notes.push(Note {
            file_index,
            order,
            stream,
            text,
        });
    }

    pub fn record_processed_file(&self, file_index: usize, filename: &str, had_error: bool) {
        self.inner.lock().processed_files.push(ProcessedFile {
            file_index,
            filename: Arc::from(filename),
            had_error,
        });
    }

    pub fn reset_error_counts(&self) {
        let mut inner = self.inner.lock();
        inner.error_count = 0;
        inner.errors_by_category.clear();
        inner.diagnostics.clear();
        inner.notes.clear();
        inner.processed_files.clear();
    }

    pub fn has_error(&self, category: crate::categories::Category) -> bool {
        self.inner
            .lock()
            .errors_by_category
            .contains_key(category.as_str())
    }

    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = self.inner.lock().diagnostics.clone();
        diagnostics.sort_by(|lhs, rhs| {
            lhs.file_index
                .cmp(&rhs.file_index)
                .then_with(|| lhs.linenum.cmp(&rhs.linenum))
                .then_with(|| lhs.category.cmp(&rhs.category))
                .then_with(|| lhs.message.cmp(&rhs.message))
        });
        diagnostics
    }

    pub fn notes(&self) -> Vec<Note> {
        let mut notes = self.inner.lock().notes.clone();
        notes.sort_by(|lhs, rhs| {
            lhs.file_index
                .cmp(&rhs.file_index)
                .then_with(|| lhs.order.cmp(&rhs.order))
                .then_with(|| lhs.text.cmp(&rhs.text))
        });
        notes
    }

    pub fn processed_files(&self) -> Vec<ProcessedFile> {
        let mut processed_files = self.inner.lock().processed_files.clone();
        processed_files.sort_by(|lhs, rhs| {
            lhs.file_index
                .cmp(&rhs.file_index)
                .then_with(|| lhs.filename.cmp(&rhs.filename))
        });
        processed_files
    }

    pub fn into_snapshot(self) -> SessionSnapshot {
        let inner = self.inner.into_inner();
        SessionSnapshot {
            error_count: inner.error_count,
            diagnostics: inner.diagnostics,
            notes: inner.notes,
            processed_files: inner.processed_files,
        }
    }
}

impl Default for LintSession {
    fn default() -> Self {
        Self::new()
    }
}

pub type CppLintState = LintSession;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let state = CppLintState::new();
        assert_eq!(state.error_count(), 0);
        assert_eq!(state.verbose_level(), 1);
        assert!(!state.quiet());
        assert_eq!(state.output_format(), OutputFormat::Emacs);
        assert_eq!(state.counting_style(), CountingStyle::Total);
    }

    #[test]
    fn test_increment_error() {
        let state = CppLintState::new();
        state.increment_error_count(crate::categories::Category::BuildInclude);
        state.increment_error_count(crate::categories::Category::BuildInclude);
        state.increment_error_count(crate::categories::Category::ReadabilityBraces);
        assert_eq!(state.error_count(), 3);
    }

    #[test]
    fn test_include_state_tracks_duplicates_and_order() {
        let mut include_state = IncludeState::new();
        include_state
            .last_include_list_mut()
            .push(("vector".to_string(), 2));
        include_state
            .last_include_list_mut()
            .push(("vector".to_string(), 5));
        assert_eq!(include_state.find_header("vector"), Some(2));

        assert_eq!(
            include_state.check_next_include_order(IncludeKind::OtherHeader),
            None
        );
        assert_eq!(
            include_state.check_next_include_order(IncludeKind::CppSystem),
            Some("Found C++ system header after other header".to_string())
        );

        let canonical_z = include_state.canonicalize_alphabetical_order("foo/z-inl.h");
        include_state.set_last_header(&canonical_z);
        assert!(!include_state.is_in_alphabetical_order(
            true,
            &include_state.canonicalize_alphabetical_order("foo/a.h")
        ));

        include_state.reset_section("if");
        assert_eq!(include_state.include_lists().len(), 2);
        include_state
            .last_include_list_mut()
            .push(("foo/z.h".to_string(), 7));
        include_state.reset_section("else");
        assert!(
            include_state
                .include_lists()
                .last()
                .is_some_and(|section| section.is_empty())
        );
    }

    #[test]
    fn test_function_state_tracks_current_function_lines() {
        let mut function_state = FunctionState::new();
        assert_eq!(function_state.current_name(), None);
        assert_eq!(function_state.lines_in_function(), 0);

        function_state.begin("Foo");
        function_state.count_line();
        function_state.count_line();
        assert_eq!(function_state.current_name(), Some("Foo"));
        assert_eq!(function_state.lines_in_function(), 2);

        assert_eq!(function_state.end(), Some(("Foo".to_string(), 2)));
        assert_eq!(function_state.current_name(), None);
        assert_eq!(function_state.lines_in_function(), 0);
    }

    #[test]
    fn test_record_diagnostic_tracks_messages() {
        let state = CppLintState::new();
        state.record_diagnostic(
            1,
            "foo.cc",
            4,
            crate::categories::Category::WhitespaceTab,
            1,
            "Tab found",
        );
        state.record_info(1, 0, "Done processing foo.cc\n");
        state.record_processed_file(1, "foo.cc", true);

        assert_eq!(state.error_count(), 1);
        assert!(state.has_error(crate::categories::Category::WhitespaceTab));
        assert_eq!(state.diagnostics()[0].linenum, 5);
        assert_eq!(state.notes().len(), 1);
        assert_eq!(state.processed_files().len(), 1);
    }

    #[test]
    fn test_with_settings_and_snapshot_keep_recorded_data() {
        let state = CppLintState::with_settings(SessionSettings {
            verbose_level: 3,
            counting_style: CountingStyle::Detailed,
            quiet: true,
            output_format: OutputFormat::JUnit,
            num_threads: 8,
        });
        assert_eq!(state.verbose_level(), 3);
        assert_eq!(state.counting_style(), CountingStyle::Detailed);
        assert!(state.quiet());
        assert_eq!(state.output_format(), OutputFormat::JUnit);
        assert_eq!(state.num_threads(), 8);

        state.record_diagnostic(
            0,
            "demo.cc",
            0,
            crate::categories::Category::WhitespaceTab,
            1,
            "Tab found",
        );
        state.record_info(0, 0, "Done processing demo.cc\n");
        let snapshot = state.into_snapshot();

        assert_eq!(snapshot.error_count, 1);
        assert_eq!(snapshot.diagnostics.len(), 1);
        assert_eq!(snapshot.notes.len(), 1);
    }
}
