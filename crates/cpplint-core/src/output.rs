use crate::diagnostics::{Diagnostic, FileId, FileTable, Note, NoteStream, ProcessedFile};
use crate::state::{CountingStyle, OutputFormat};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderedOutput {
    pub stdout: String,
    pub stderr: String,
}

pub fn format_diagnostic(
    output_format: OutputFormat,
    file_names: &FileTable,
    diagnostic: &Diagnostic,
) -> String {
    let filename = file_name_for(file_names, diagnostic.file_id);
    format_diagnostic_with_name(output_format, filename, diagnostic)
}

pub fn format_diagnostic_with_name(
    output_format: OutputFormat,
    filename: &str,
    diagnostic: &Diagnostic,
) -> String {
    match output_format {
        OutputFormat::Vs7 => format!(
            "{}({}): error cpplint: [{}] {} [{}]\n",
            filename,
            diagnostic.linenum,
            diagnostic.category,
            diagnostic.message,
            diagnostic.confidence
        ),
        OutputFormat::Eclipse => format!(
            "{}:{}: warning: {}  [{}] [{}]\n",
            filename,
            diagnostic.linenum,
            diagnostic.message,
            diagnostic.category,
            diagnostic.confidence
        ),
        OutputFormat::Emacs | OutputFormat::JUnit | OutputFormat::Sed | OutputFormat::Gsed => {
            format!(
                "{}:{}:  {}  [{}] [{}]\n",
                filename,
                diagnostic.linenum,
                diagnostic.message,
                diagnostic.category,
                diagnostic.confidence
            )
        }
    }
}

pub fn format_note(note: &Note) -> String {
    note.text.to_string()
}

pub fn format_sed_diagnostic(
    output_format: OutputFormat,
    file_names: &FileTable,
    diagnostic: &Diagnostic,
) -> (bool, String) {
    let filename = file_name_for(file_names, diagnostic.file_id);
    format_sed_diagnostic_with_name(output_format, filename, diagnostic)
}

pub fn format_sed_diagnostic_with_name(
    output_format: OutputFormat,
    filename: &str,
    diagnostic: &Diagnostic,
) -> (bool, String) {
    let command = match output_format {
        OutputFormat::Sed => "sed",
        OutputFormat::Gsed => "gsed",
        _ => return (false, String::new()),
    };

    if let Some(script) = sed_fixup(&diagnostic.message) {
        (
            true,
            format!(
                "{} -i '{}{}' {} # {}  [{}] [{}]\n",
                command,
                diagnostic.linenum,
                script,
                filename,
                diagnostic.message,
                diagnostic.category,
                diagnostic.confidence
            ),
        )
    } else {
        (
            false,
            format!(
                "# {}:{}:  \"{}\"  [{}] [{}]\n",
                filename,
                diagnostic.linenum,
                diagnostic.message,
                diagnostic.category,
                diagnostic.confidence
            ),
        )
    }
}

#[derive(Debug)]
pub struct DiagnosticCounter {
    counting_style: CountingStyle,
    counts: BTreeMap<String, usize>,
    total: usize,
}

impl DiagnosticCounter {
    pub fn new(counting_style: CountingStyle) -> Self {
        Self {
            counting_style,
            counts: BTreeMap::new(),
            total: 0,
        }
    }

    pub fn add(&mut self, diagnostic: &Diagnostic) {
        self.total += 1;
        if self.counting_style != CountingStyle::Total {
            let category = match self.counting_style {
                CountingStyle::Total => unreachable!(),
                CountingStyle::Toplevel => diagnostic
                    .category
                    .as_str()
                    .split('/')
                    .next()
                    .unwrap_or(diagnostic.category.as_str())
                    .to_string(),
                CountingStyle::Detailed => diagnostic.category.to_string(),
            };
            *self.counts.entry(category).or_insert(0) += 1;
        }
    }

    pub fn total(&self) -> usize {
        self.total
    }

    pub fn render_summary(&self) -> String {
        let mut out = String::new();
        if self.counting_style != CountingStyle::Total {
            for (category, count) in &self.counts {
                out.push_str(&format!(
                    "Category '{}' errors found: {}\n",
                    category, count
                ));
            }
        }
        out.push_str(&format!("Total errors found: {}\n", self.total));
        out
    }
}

pub fn render(
    output_format: OutputFormat,
    counting_style: CountingStyle,
    file_names: &FileTable,
    diagnostics: &[Diagnostic],
    notes: &[Note],
    processed_files: &[ProcessedFile],
    timing: Option<Duration>,
) -> RenderedOutput {
    render_owned(
        output_format,
        counting_style,
        file_names.clone(),
        diagnostics.to_vec(),
        notes.to_vec(),
        processed_files.to_vec(),
        timing,
    )
}

pub(crate) fn render_owned(
    output_format: OutputFormat,
    counting_style: CountingStyle,
    file_names: FileTable,
    mut diagnostics: Vec<Diagnostic>,
    mut notes: Vec<Note>,
    mut processed_files: Vec<ProcessedFile>,
    timing: Option<Duration>,
) -> RenderedOutput {
    sort_diagnostics(&mut diagnostics);
    sort_notes(&mut notes);
    sort_processed_files(&mut processed_files);

    match output_format {
        OutputFormat::JUnit => render_junit(&file_names, &diagnostics, &notes, &processed_files),
        OutputFormat::Sed | OutputFormat::Gsed => {
            render_sed_like(output_format, &file_names, &diagnostics, &notes)
        }
        OutputFormat::Emacs | OutputFormat::Vs7 | OutputFormat::Eclipse => render_human(
            output_format,
            counting_style,
            &file_names,
            &diagnostics,
            &notes,
            timing,
        ),
    }
}

fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|lhs, rhs| {
        lhs.file_id
            .cmp(&rhs.file_id)
            .then_with(|| lhs.linenum.cmp(&rhs.linenum))
            .then_with(|| lhs.category.cmp(&rhs.category))
            .then_with(|| lhs.message.cmp(&rhs.message))
    });
}

fn sort_notes(notes: &mut [Note]) {
    notes.sort_by(|lhs, rhs| {
        lhs.file_id
            .cmp(&rhs.file_id)
            .then_with(|| lhs.order.cmp(&rhs.order))
            .then_with(|| lhs.text.cmp(&rhs.text))
    });
}

fn sort_processed_files(processed_files: &mut [ProcessedFile]) {
    processed_files.sort_by_key(|file| file.file_id);
}

fn render_human(
    output_format: OutputFormat,
    counting_style: CountingStyle,
    file_names: &FileTable,
    diagnostics: &[Diagnostic],
    notes: &[Note],
    timing: Option<Duration>,
) -> RenderedOutput {
    let mut rendered = RenderedOutput::default();

    for note in notes {
        match note.stream {
            NoteStream::Stdout => rendered.stdout.push_str(&note.text),
            NoteStream::Stderr => rendered.stderr.push_str(&note.text),
        }
    }

    for diagnostic in diagnostics {
        rendered
            .stderr
            .push_str(&format_diagnostic(output_format, file_names, diagnostic));
    }

    if !diagnostics.is_empty() {
        rendered
            .stdout
            .push_str(&render_counts(counting_style, diagnostics));
    }

    if let Some(duration) = timing {
        rendered
            .stdout
            .push_str(&format!("Runtime: {:.3}(s)\n", duration.as_secs_f64()));
    }

    rendered
}

fn render_sed_like(
    output_format: OutputFormat,
    file_names: &FileTable,
    diagnostics: &[Diagnostic],
    notes: &[Note],
) -> RenderedOutput {
    let mut rendered = RenderedOutput::default();

    for note in notes {
        if note.stream == NoteStream::Stderr {
            rendered.stderr.push_str(&note.text);
        }
    }

    for diagnostic in diagnostics {
        let (is_fixable, text) = format_sed_diagnostic(output_format, file_names, diagnostic);
        if is_fixable {
            rendered.stdout.push_str(&text);
        } else {
            rendered.stderr.push_str(&text);
        }
    }

    rendered
}

fn render_junit(
    file_names: &FileTable,
    diagnostics: &[Diagnostic],
    notes: &[Note],
    processed_files: &[ProcessedFile],
) -> RenderedOutput {
    let mut rendered = RenderedOutput::default();

    for note in notes {
        if note.stream == NoteStream::Stderr {
            rendered.stderr.push_str(&note.text);
        }
    }

    let mut grouped: BTreeMap<FileId, Vec<&Diagnostic>> = BTreeMap::new();
    for diagnostic in diagnostics {
        grouped
            .entry(diagnostic.file_id)
            .or_default()
            .push(diagnostic);
    }

    let tests_count = if processed_files.is_empty() {
        grouped.len()
    } else {
        processed_files.len()
    };

    rendered.stdout.push_str(&format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="cpplint" tests="{}" failures="{}" errors="0">
"#,
        tests_count,
        diagnostics.len()
    ));

    if processed_files.is_empty() {
        let synthesized_cases: Vec<ProcessedFile> = grouped
            .keys()
            .map(|file_id| ProcessedFile {
                file_id: *file_id,
                had_error: true,
            })
            .collect();

        for case in &synthesized_cases {
            render_junit_case(&mut rendered.stdout, file_names, &grouped, case);
        }
    } else {
        for case in processed_files {
            render_junit_case(&mut rendered.stdout, file_names, &grouped, case);
        }
    }

    rendered.stdout.push_str("</testsuite>\n");
    rendered
}

fn render_junit_case(
    stdout: &mut String,
    file_names: &FileTable,
    grouped: &BTreeMap<FileId, Vec<&Diagnostic>>,
    case: &ProcessedFile,
) {
    let case_filename = file_name_for(file_names, case.file_id);
    stdout.push_str(&format!(
        r#"  <testcase classname="cpplint" name="{}">
"#,
        xml_escape(case_filename)
    ));
    if let Some(entries) = grouped.get(&case.file_id) {
        for diagnostic in entries {
            let diagnostic_filename = file_name_for(file_names, diagnostic.file_id);
            let summary = format!(
                "[{}] [{}] {}:{}",
                diagnostic.category, diagnostic.confidence, diagnostic_filename, diagnostic.linenum
            );
            let body = format!(
                "{}:{}: {}",
                diagnostic_filename, diagnostic.linenum, diagnostic.message
            );
            stdout.push_str(&format!(
                r#"    <failure type="{}" message="{}">{}</failure>
"#,
                xml_escape(diagnostic.category.as_str()),
                xml_escape(&summary),
                xml_escape(&body)
            ));
        }
    }
    stdout.push_str("  </testcase>\n");
}

fn render_counts(counting_style: CountingStyle, diagnostics: &[Diagnostic]) -> String {
    let mut counter = DiagnosticCounter::new(counting_style);
    for diagnostic in diagnostics {
        counter.add(diagnostic);
    }
    counter.render_summary()
}

fn sed_fixup(message: &crate::messages::LintMessage) -> Option<&'static str> {
    use crate::messages::{BracesRedundantKind, LintMessage, OperatorSymbol};
    match message {
        LintMessage::MissingSpacesAround(OperatorSymbol::Eq) => Some(r"s/ = /=/"),
        LintMessage::MissingSpacesAround(OperatorSymbol::Ne) => Some(r"s/ != /!=/"),
        LintMessage::ExtraSpaceBeforeParenIn(ctx) if ctx.as_ref() == "if" => Some(r"s/if (/if(/"),
        LintMessage::ExtraSpaceBeforeParenIn(ctx) if ctx.as_ref() == "for" => {
            Some(r"s/for (/for(/")
        }
        LintMessage::ExtraSpaceBeforeParenIn(ctx) if ctx.as_ref() == "while" => {
            Some(r"s/while (/while(/")
        }
        LintMessage::ExtraSpaceBeforeParenIn(ctx) if ctx.as_ref() == "switch" => {
            Some(r"s/switch (/switch(/")
        }
        LintMessage::ShouldHaveSpaceBetweenSlashesAndComment => Some(r"s/\/\//\/\/ /"),
        LintMessage::MissingSpaceBeforeOpenBrace => Some(r"s/\([^ ]\){/\1 {/"),
        LintMessage::TabFound => Some(r"s/\t/  /g"),
        LintMessage::TrailingWhitespace => Some(r"s/\s*$//"),
        LintMessage::BracesRedundant(BracesRedundantKind::ClosingBrace) => Some(r"s/};/}/"),
        LintMessage::MissingSpaceAfterComma => Some(r"s/,\([^ ]\)/, \1/g"),
        _ => None,
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn file_name_for(file_names: &FileTable, file_id: FileId) -> &str {
    file_names.get(file_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_diagnostic() -> Diagnostic {
        Diagnostic {
            file_id: FileId::from_index(0),
            linenum: 7,
            category: crate::categories::Category::WhitespaceTab,
            confidence: 1,
            message: crate::messages::LintMessage::TabFound,
        }
    }

    fn sample_cast_diagnostic() -> Diagnostic {
        Diagnostic {
            file_id: FileId::from_index(0),
            linenum: 3,
            category: crate::categories::Category::ReadabilityCasting,
            confidence: 1,
            message: crate::messages::LintMessage::CStyleCast("int".into(), "float".into()),
        }
    }

    fn sample_files(names: &[&str]) -> FileTable {
        let mut file_names = FileTable::new();
        for name in names {
            file_names.intern(name);
        }
        file_names
    }

    #[test]
    fn renders_emacs_output_and_counts() {
        let rendered = render(
            OutputFormat::Emacs,
            CountingStyle::Detailed,
            &sample_files(&["sample.cc"]),
            &[sample_diagnostic()],
            &[Note {
                file_id: FileId::from_index(0),
                order: 0,
                stream: NoteStream::Stdout,
                text: "Done processing sample.cc\n".into(),
            }],
            &[],
            None,
        );

        assert!(rendered.stdout.contains("Done processing sample.cc"));
        assert!(
            rendered
                .stdout
                .contains("Category 'whitespace/tab' errors found: 1")
        );
        assert!(
            rendered
                .stderr
                .contains("sample.cc:7:  Tab found; better to use spaces")
        );
    }

    #[test]
    fn renders_junit_output() {
        let rendered = render(
            OutputFormat::JUnit,
            CountingStyle::Total,
            &sample_files(&["sample.cc"]),
            &[sample_diagnostic()],
            &[],
            &[ProcessedFile {
                file_id: FileId::from_index(0),
                had_error: true,
            }],
            None,
        );

        assert!(rendered.stdout.contains("<testsuite"));
        assert!(rendered.stdout.contains("<failure"));
        assert!(rendered.stdout.contains("sample.cc"));
    }

    #[test]
    fn renders_junit_output_without_processed_files() {
        let rendered = render(
            OutputFormat::JUnit,
            CountingStyle::Total,
            &sample_files(&["a.cc", "b.cc"]),
            &[sample_diagnostic()],
            &[],
            &[],
            None,
        );

        assert!(rendered.stdout.contains(r#"tests="1""#));
        assert!(rendered.stdout.contains("sample.cc") || rendered.stdout.contains("a.cc"));
        assert!(rendered.stdout.contains("<testcase"));
    }

    #[test]
    fn format_sed_diagnostic_emits_fixup_script_for_known_message() {
        let (is_fixable, rendered) = format_sed_diagnostic(
            OutputFormat::Sed,
            &sample_files(&["sample.cc"]),
            &sample_diagnostic(),
        );

        assert!(is_fixable);
        assert!(rendered.contains("sed -i"));
        assert!(rendered.contains(r#"s/\t/  /g"#));
    }

    #[test]
    fn format_sed_diagnostic_comments_unknown_fixes() {
        let (is_fixable, rendered) = format_sed_diagnostic(
            OutputFormat::Gsed,
            &sample_files(&["sample.cc"]),
            &sample_cast_diagnostic(),
        );

        assert!(!is_fixable);
        assert!(rendered.starts_with("# sample.cc:3:"));
        assert!(rendered.contains("C-style cast"));
    }

    #[test]
    fn render_owned_sorts_unsorted_inputs() {
        let rendered = render_owned(
            OutputFormat::Emacs,
            CountingStyle::Total,
            sample_files(&["a.cc", "b.cc"]),
            vec![
                Diagnostic {
                    file_id: FileId::from_index(1),
                    linenum: 4,
                    category: crate::categories::Category::WhitespaceTab,
                    confidence: 1,
                    message: crate::messages::LintMessage::TabFound,
                },
                Diagnostic {
                    file_id: FileId::from_index(0),
                    linenum: 2,
                    category: crate::categories::Category::WhitespaceTab,
                    confidence: 1,
                    message: crate::messages::LintMessage::TabFound,
                },
            ],
            vec![
                Note {
                    file_id: FileId::from_index(1),
                    order: 0,
                    stream: NoteStream::Stdout,
                    text: "Done processing b.cc\n".into(),
                },
                Note {
                    file_id: FileId::from_index(0),
                    order: 0,
                    stream: NoteStream::Stdout,
                    text: "Done processing a.cc\n".into(),
                },
            ],
            vec![],
            None,
        );

        assert!(
            rendered
                .stdout
                .starts_with("Done processing a.cc\nDone processing b.cc\n")
        );
        assert!(
            rendered
                .stderr
                .starts_with("a.cc:2:  Tab found; better to use spaces")
        );
    }
}
