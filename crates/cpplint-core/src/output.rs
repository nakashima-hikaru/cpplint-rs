use crate::diagnostics::{Diagnostic, Note, NoteStream, ProcessedFile};
use crate::state::{CountingStyle, OutputFormat};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderedOutput {
    pub stdout: String,
    pub stderr: String,
}

pub fn render(
    output_format: OutputFormat,
    counting_style: CountingStyle,
    diagnostics: &[Diagnostic],
    notes: &[Note],
    processed_files: &[ProcessedFile],
    timing: Option<Duration>,
) -> RenderedOutput {
    render_owned(
        output_format,
        counting_style,
        diagnostics.to_vec(),
        notes.to_vec(),
        processed_files.to_vec(),
        timing,
    )
}

pub(crate) fn render_owned(
    output_format: OutputFormat,
    counting_style: CountingStyle,
    mut diagnostics: Vec<Diagnostic>,
    mut notes: Vec<Note>,
    mut processed_files: Vec<ProcessedFile>,
    timing: Option<Duration>,
) -> RenderedOutput {
    sort_diagnostics(&mut diagnostics);
    sort_notes(&mut notes);
    sort_processed_files(&mut processed_files);

    match output_format {
        OutputFormat::JUnit => render_junit(&diagnostics, &notes, &processed_files),
        OutputFormat::Sed | OutputFormat::Gsed => {
            render_sed_like(output_format, &diagnostics, &notes)
        }
        OutputFormat::Emacs | OutputFormat::Vs7 | OutputFormat::Eclipse => {
            render_human(output_format, counting_style, &diagnostics, &notes, timing)
        }
    }
}

fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|lhs, rhs| {
        lhs.file_index
            .cmp(&rhs.file_index)
            .then_with(|| lhs.linenum.cmp(&rhs.linenum))
            .then_with(|| lhs.category.cmp(&rhs.category))
            .then_with(|| lhs.message.cmp(&rhs.message))
    });
}

fn sort_notes(notes: &mut [Note]) {
    notes.sort_by(|lhs, rhs| {
        lhs.file_index
            .cmp(&rhs.file_index)
            .then_with(|| lhs.order.cmp(&rhs.order))
            .then_with(|| lhs.text.cmp(&rhs.text))
    });
}

fn sort_processed_files(processed_files: &mut [ProcessedFile]) {
    processed_files.sort_by(|lhs, rhs| {
        lhs.file_index
            .cmp(&rhs.file_index)
            .then_with(|| lhs.filename.cmp(&rhs.filename))
    });
}

fn render_human(
    output_format: OutputFormat,
    counting_style: CountingStyle,
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
            .push_str(&format_diagnostic(output_format, diagnostic));
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
    diagnostics: &[Diagnostic],
    notes: &[Note],
) -> RenderedOutput {
    let mut rendered = RenderedOutput::default();
    let command = match output_format {
        OutputFormat::Sed => "sed",
        OutputFormat::Gsed => "gsed",
        _ => unreachable!(),
    };

    for note in notes {
        if note.stream == NoteStream::Stderr {
            rendered.stderr.push_str(&note.text);
        }
    }

    for diagnostic in diagnostics {
        if let Some(script) = sed_fixup(&diagnostic.message) {
            rendered.stdout.push_str(&format!(
                "{} -i '{}{}' {} # {}  [{}] [{}]\n",
                command,
                diagnostic.linenum,
                script,
                diagnostic.filename,
                diagnostic.message,
                diagnostic.category,
                diagnostic.confidence
            ));
        } else {
            rendered.stderr.push_str(&format!(
                "# {}:{}:  \"{}\"  [{}] [{}]\n",
                diagnostic.filename,
                diagnostic.linenum,
                diagnostic.message,
                diagnostic.category,
                diagnostic.confidence
            ));
        }
    }

    rendered
}

fn render_junit(
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

    let mut grouped: BTreeMap<&str, Vec<&Diagnostic>> = BTreeMap::new();
    for diagnostic in diagnostics {
        grouped
            .entry(&diagnostic.filename)
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
            .enumerate()
            .map(|(file_index, filename)| ProcessedFile {
                file_index,
                filename: (*filename).to_string(),
                had_error: true,
            })
            .collect();

        for case in &synthesized_cases {
            render_junit_case(&mut rendered.stdout, &grouped, case);
        }
    } else {
        for case in processed_files {
            render_junit_case(&mut rendered.stdout, &grouped, case);
        }
    }

    rendered.stdout.push_str("</testsuite>\n");
    rendered
}

fn render_junit_case(
    stdout: &mut String,
    grouped: &BTreeMap<&str, Vec<&Diagnostic>>,
    case: &ProcessedFile,
) {
    stdout.push_str(&format!(
        r#"  <testcase classname="cpplint" name="{}">
"#,
        xml_escape(&case.filename)
    ));
    if let Some(entries) = grouped.get(case.filename.as_str()) {
        for diagnostic in entries {
            let summary = format!(
                "[{}] [{}] {}:{}",
                diagnostic.category, diagnostic.confidence, diagnostic.filename, diagnostic.linenum
            );
            let body = format!(
                "{}:{}: {}",
                diagnostic.filename, diagnostic.linenum, diagnostic.message
            );
            stdout.push_str(&format!(
                r#"    <failure type="{}" message="{}">{}</failure>
"#,
                xml_escape(&diagnostic.category),
                xml_escape(&summary),
                xml_escape(&body)
            ));
        }
    }
    stdout.push_str("  </testcase>\n");
}

fn format_diagnostic(output_format: OutputFormat, diagnostic: &Diagnostic) -> String {
    match output_format {
        OutputFormat::Vs7 => format!(
            "{}({}): error cpplint: [{}] {} [{}]\n",
            diagnostic.filename,
            diagnostic.linenum,
            diagnostic.category,
            diagnostic.message,
            diagnostic.confidence
        ),
        OutputFormat::Eclipse => format!(
            "{}:{}: warning: {}  [{}] [{}]\n",
            diagnostic.filename,
            diagnostic.linenum,
            diagnostic.message,
            diagnostic.category,
            diagnostic.confidence
        ),
        OutputFormat::Emacs | OutputFormat::JUnit | OutputFormat::Sed | OutputFormat::Gsed => {
            format!(
                "{}:{}:  {}  [{}] [{}]\n",
                diagnostic.filename,
                diagnostic.linenum,
                diagnostic.message,
                diagnostic.category,
                diagnostic.confidence
            )
        }
    }
}

fn render_counts(counting_style: CountingStyle, diagnostics: &[Diagnostic]) -> String {
    let mut out = String::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    if counting_style != CountingStyle::Total {
        for diagnostic in diagnostics {
            let category = match counting_style {
                CountingStyle::Total => unreachable!(),
                CountingStyle::Toplevel => diagnostic
                    .category
                    .split('/')
                    .next()
                    .unwrap_or(diagnostic.category.as_str())
                    .to_string(),
                CountingStyle::Detailed => diagnostic.category.clone(),
            };
            *counts.entry(category).or_insert(0) += 1;
        }

        for (category, count) in counts {
            out.push_str(&format!(
                "Category '{}' errors found: {}\n",
                category, count
            ));
        }
    }

    out.push_str(&format!("Total errors found: {}\n", diagnostics.len()));
    out
}

fn sed_fixup(message: &str) -> Option<&'static str> {
    match message {
        "Missing spaces around =" => Some(r"s/ = /=/"),
        "Missing spaces around !=" => Some(r"s/ != /!=/"),
        "Extra space before ( in if (" => Some(r"s/if (/if(/"),
        "Extra space before ( in for (" => Some(r"s/for (/for(/"),
        "Extra space before ( in while (" => Some(r"s/while (/while(/"),
        "Extra space before ( in switch (" => Some(r"s/switch (/switch(/"),
        "Should have a space between // and comment" => Some(r"s/\/\//\/\/ /"),
        "Missing space before {" => Some(r"s/\([^ ]\){/\1 {/"),
        "Tab found; better to use spaces" => Some(r"s/\t/  /g"),
        "Line ends in whitespace.  Consider deleting these extra spaces." => Some(r"s/\s*$//"),
        "You don't need a ; after a }" => Some(r"s/};/}/"),
        "Missing space after ," => Some(r"s/,\([^ ]\)/, \1/g"),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_diagnostic() -> Diagnostic {
        Diagnostic {
            file_index: 0,
            filename: "sample.cc".to_string(),
            linenum: 7,
            category: "whitespace/tab".to_string(),
            confidence: 1,
            message: "Tab found; better to use spaces".to_string(),
        }
    }

    #[test]
    fn renders_emacs_output_and_counts() {
        let rendered = render(
            OutputFormat::Emacs,
            CountingStyle::Detailed,
            &[sample_diagnostic()],
            &[Note {
                file_index: 0,
                order: 0,
                stream: NoteStream::Stdout,
                text: "Done processing sample.cc\n".to_string(),
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
            &[sample_diagnostic()],
            &[],
            &[ProcessedFile {
                file_index: 0,
                filename: "sample.cc".to_string(),
                had_error: true,
            }],
            None,
        );

        assert!(rendered.stdout.contains("<testsuite"));
        assert!(rendered.stdout.contains("<failure"));
        assert!(rendered.stdout.contains("sample.cc"));
    }

    #[test]
    fn render_owned_sorts_unsorted_inputs() {
        let rendered = render_owned(
            OutputFormat::Emacs,
            CountingStyle::Total,
            vec![
                Diagnostic {
                    file_index: 1,
                    filename: "b.cc".to_string(),
                    linenum: 4,
                    category: "whitespace/tab".to_string(),
                    confidence: 1,
                    message: "Tab found; better to use spaces".to_string(),
                },
                Diagnostic {
                    file_index: 0,
                    filename: "a.cc".to_string(),
                    linenum: 2,
                    category: "whitespace/tab".to_string(),
                    confidence: 1,
                    message: "Tab found; better to use spaces".to_string(),
                },
            ],
            vec![
                Note {
                    file_index: 1,
                    order: 0,
                    stream: NoteStream::Stdout,
                    text: "Done processing b.cc\n".to_string(),
                },
                Note {
                    file_index: 0,
                    order: 0,
                    stream: NoteStream::Stdout,
                    text: "Done processing a.cc\n".to_string(),
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
