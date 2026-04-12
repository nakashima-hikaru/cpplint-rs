use crate::config::{ConfigMessage, ConfigMessageKind, ConfigResolution, DirectoryConfigCache};
use crate::diagnostics::{Diagnostic, Note, NoteStream, ProcessedFile};
use crate::file_linter::FileLinter;
use crate::glob::GlobPattern;
use crate::options::Options;
use crate::output::render_owned;
use crate::state::{CountingStyle, CppLintState, OutputFormat, SessionSettings, SessionSnapshot};
use crate::string_utils::set_to_str;
use crate::{errors::Result, output::RenderedOutput};
use ignore::WalkBuilder;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct RunnerConfig {
    pub options: Options,
    pub output_format: OutputFormat,
    pub counting_style: CountingStyle,
    pub verbose_level: i32,
    pub quiet: bool,
    pub num_threads: usize,
    pub recursive: bool,
    pub excludes: Vec<String>,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            options: Options::new(),
            output_format: OutputFormat::Emacs,
            counting_style: CountingStyle::Total,
            verbose_level: 1,
            quiet: false,
            num_threads: 1,
            recursive: false,
            excludes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintRunResult {
    pub stdout: String,
    pub stderr: String,
    pub error_count: usize,
}

#[derive(Debug, Default)]
struct CollectedFiles {
    files: Vec<(usize, PathBuf)>,
    notes: Vec<Note>,
}

#[derive(Debug, Default)]
struct FileRunReport {
    error_count: usize,
    diagnostics: Vec<Diagnostic>,
    notes: Vec<Note>,
    processed_files: Vec<ProcessedFile>,
}

#[derive(Debug, Clone)]
struct PlannedLintJob {
    file_index: usize,
    file: PathBuf,
    display_name: String,
    options: Arc<Options>,
    initial_notes: Vec<Note>,
    failure_note_order: usize,
    done_note_order: usize,
}

#[derive(Debug)]
enum PlannedEntry {
    LintJob(PlannedLintJob),
    Report(FileRunReport),
}

#[derive(Debug, Default)]
struct PlannedRun {
    lint_jobs: Vec<PlannedLintJob>,
    reports: Vec<FileRunReport>,
}

impl From<SessionSnapshot> for FileRunReport {
    fn from(snapshot: SessionSnapshot) -> Self {
        Self {
            error_count: snapshot.error_count,
            diagnostics: snapshot.diagnostics,
            notes: snapshot.notes,
            processed_files: snapshot.processed_files,
        }
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn run_lint(files: &[PathBuf], config: &RunnerConfig) -> Result<LintRunResult> {
    let session_settings = SessionSettings {
        verbose_level: config.verbose_level,
        counting_style: config.counting_style,
        quiet: config.quiet,
        output_format: config.output_format,
        num_threads: config.num_threads.max(1),
    };

    let started_at = config.options.timing.then(Instant::now);
    let CollectedFiles {
        files: collected_files,
        notes: collected_notes,
    } = collect_files(files, config)?;
    let pool = if config.num_threads > 1 {
        Some(
            ThreadPoolBuilder::new()
                .num_threads(config.num_threads.max(1))
                .build()?,
        )
    } else {
        None
    };
    let PlannedRun {
        lint_jobs,
        reports: planned_reports,
    } = if let Some(pool) = &pool {
        pool.install(|| plan_files(collected_files, config))
    } else {
        plan_files(collected_files, config)
    };

    let reports = if let Some(pool) = &pool {
        pool.install(|| {
            lint_jobs
                .into_par_iter()
                .map(|job| process_file(job, session_settings))
                .collect::<Vec<_>>()
        })
    } else {
        lint_jobs
            .into_iter()
            .map(|job| process_file(job, session_settings))
            .collect::<Vec<_>>()
    };

    let mut error_count = 0usize;
    let mut diagnostics = Vec::new();
    let mut notes = collected_notes;
    let mut processed_files = Vec::new();

    for report in planned_reports {
        error_count += report.error_count;
        diagnostics.extend(report.diagnostics);
        notes.extend(report.notes);
        processed_files.extend(report.processed_files);
    }

    for report in reports {
        error_count += report.error_count;
        diagnostics.extend(report.diagnostics);
        notes.extend(report.notes);
        processed_files.extend(report.processed_files);
    }

    let rendered: RenderedOutput = render_owned(
        config.output_format,
        config.counting_style,
        diagnostics,
        notes,
        processed_files,
        started_at.map(|instant| instant.elapsed()),
    );

    Ok(LintRunResult {
        stdout: rendered.stdout,
        stderr: rendered.stderr,
        error_count,
    })
}

fn collect_files(files: &[PathBuf], config: &RunnerConfig) -> Result<CollectedFiles> {
    let cwd = std::env::current_dir()?;
    let excludes = compile_excludes(&cwd, &config.excludes)?;
    let mut collected = Vec::new();
    let mut notes = Vec::new();

    for (file_index, file) in files.iter().enumerate() {
        if file == Path::new("-") {
            collected.push((file_index, PathBuf::from("-")));
            continue;
        }

        if !file.exists() {
            notes.push(Note {
                file_index,
                order: 0,
                stream: NoteStream::Stderr,
                text: format!("Skipping input '{}': Path not found.\n", file.display()),
            });
            continue;
        }

        let canonical = std::fs::canonicalize(file).unwrap_or_else(|_| file.clone());
        if config.recursive && canonical.is_dir() {
            collected.extend(expand_directory(file_index, &canonical, &config.options));
        } else {
            collected.push((file_index, canonical));
        }
    }

    collected.retain(|(_, file)| !should_exclude(file, &excludes));
    collected.sort_by_cached_key(|(_, file)| file.to_string_lossy().into_owned());
    collected.dedup_by(|lhs, rhs| lhs.1 == rhs.1);
    for (sorted_index, (file_index, _)) in collected.iter_mut().enumerate() {
        *file_index = sorted_index;
    }
    Ok(CollectedFiles {
        files: collected,
        notes,
    })
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn plan_files(files: Vec<(usize, PathBuf)>, config: &RunnerConfig) -> PlannedRun {
    let config_cache = DirectoryConfigCache::new(&config.options);
    let entries = if config.num_threads <= 1 {
        files
            .into_iter()
            .map(|(file_index, file)| plan_single_file(&config_cache, config, file_index, file))
            .collect::<Vec<_>>()
    } else {
        files
            .into_par_iter()
            .map(|(file_index, file)| plan_single_file(&config_cache, config, file_index, file))
            .collect::<Vec<_>>()
    };

    let mut planned = PlannedRun::default();
    for entry in entries {
        match entry {
            PlannedEntry::LintJob(job) => planned.lint_jobs.push(job),
            PlannedEntry::Report(report) => planned.reports.push(report),
        }
    }

    planned
}

fn plan_single_file(
    config_cache: &DirectoryConfigCache,
    config: &RunnerConfig,
    file_index: usize,
    file: PathBuf,
) -> PlannedEntry {
    let display_name = file.to_string_lossy().to_string();
    let mut note_order = 0usize;
    let mut initial_notes = Vec::new();

    let options = match config_cache.resolve_for_file(&file, config.quiet) {
        ConfigResolution::Lint { options, messages } => {
            for message in messages.iter() {
                initial_notes.push(note_from_config_message(file_index, note_order, message));
                note_order += 1;
            }
            options
        }
        ConfigResolution::Excluded { messages } => {
            let mut report = FileRunReport::default();
            for message in messages.iter() {
                report
                    .notes
                    .push(note_from_config_message(file_index, note_order, message));
                note_order += 1;
            }
            return PlannedEntry::Report(report);
        }
    };

    if file != Path::new("-") && file.is_file() && !options.is_valid_file(&file) {
        let mut report = FileRunReport::default();
        report.notes.push(Note {
            file_index,
            order: note_order,
            stream: NoteStream::Stderr,
            text: format!(
                "Ignoring {}; not a valid file name ({})\n",
                display_name,
                set_to_str(&options.all_extensions(), "[", ", ", "]")
            ),
        });
        report.processed_files.push(ProcessedFile {
            file_index,
            filename: display_name.clone(),
            had_error: false,
        });
        if !config.quiet {
            report.notes.push(Note {
                file_index,
                order: note_order + 1,
                stream: NoteStream::Stdout,
                text: format!("Done processing {}\n", display_name),
            });
        }
        return PlannedEntry::Report(report);
    }

    PlannedEntry::LintJob(PlannedLintJob {
        file_index,
        file,
        display_name,
        options,
        initial_notes,
        failure_note_order: note_order,
        done_note_order: note_order + 1,
    })
}

fn note_from_config_message(file_index: usize, order: usize, message: &ConfigMessage) -> Note {
    Note {
        file_index,
        order,
        stream: match message.kind {
            ConfigMessageKind::Info => NoteStream::Stdout,
            ConfigMessageKind::Error => NoteStream::Stderr,
        },
        text: message.text.clone(),
    }
}

fn compile_excludes(cwd: &Path, excludes: &[String]) -> Result<Vec<GlobPattern>> {
    excludes
        .iter()
        .filter(|pattern| !pattern.is_empty())
        .map(|pattern| {
            let absolute = if Path::new(pattern).is_absolute() {
                PathBuf::from(pattern)
            } else {
                cwd.join(pattern)
            };
            GlobPattern::new(&absolute.to_string_lossy(), true)
        })
        .collect()
}

fn should_exclude(file: &Path, excludes: &[GlobPattern]) -> bool {
    let normalized = file.to_string_lossy();
    excludes.iter().any(|pattern| pattern.is_match(&normalized))
}

fn expand_directory(
    file_index: usize,
    directory: &Path,
    options: &Options,
) -> Vec<(usize, PathBuf)> {
    let mut walk = WalkBuilder::new(directory);
    walk.hidden(false)
        .git_ignore(false)
        .git_exclude(false)
        .parents(false)
        .ignore(false);

    let mut files = Vec::new();
    for entry in walk.build().flatten() {
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        let path = entry.into_path();
        if options.is_valid_file(&path) {
            files.push((file_index, path));
        }
    }
    files
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn process_file(job: PlannedLintJob, session_settings: SessionSettings) -> FileRunReport {
    let PlannedLintJob {
        file_index,
        file,
        display_name,
        options,
        initial_notes,
        failure_note_order,
        done_note_order,
    } = job;
    let state = CppLintState::with_settings(session_settings);
    for note in initial_notes {
        match note.stream {
            NoteStream::Stdout => state.record_info(note.file_index, note.order, note.text),
            NoteStream::Stderr => state.record_raw_error(note.file_index, note.order, note.text),
        }
    }

    let has_error = {
        let mut linter = FileLinter::with_index(file, &state, options, file_index);
        match linter.process_file() {
            Ok(()) => Some(linter.has_error()),
            Err(_) => None,
        }
    };

    let Some(has_error) = has_error else {
        state.record_raw_error(
            file_index,
            failure_note_order,
            format!(
                "Skipping input '{}': Can't open for reading\n",
                display_name
            ),
        );
        return state.into_snapshot().into();
    };

    state.record_processed_file(file_index, &display_name, has_error);
    if !session_settings.quiet || has_error {
        state.record_info(
            file_index,
            done_note_order,
            format!("Done processing {}\n", display_name),
        );
    }
    state.into_snapshot().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::OutputFormat;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("cpplint-rs-runner-{}", unique))
    }

    #[test]
    fn runner_returns_done_processing_for_clean_file() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("demo.cc");
        std::fs::write(&file, "int main() {}\n").unwrap();

        let config = RunnerConfig {
            output_format: OutputFormat::Emacs,
            ..RunnerConfig::default()
        };
        let result = run_lint(&[file], &config).unwrap();
        assert!(result.stdout.contains("Done processing"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runner_parallel_matches_serial_results() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let clean = root.join("clean.cc");
        let dirty = root.join("dirty.cc");
        std::fs::write(&clean, "// Copyright 2026\nint main() { return 0; }\n").unwrap();
        std::fs::write(&dirty, "// Copyright 2026\nint x=0;\n").unwrap();

        let serial = run_lint(
            &[dirty.clone(), clean.clone()],
            &RunnerConfig {
                output_format: OutputFormat::Emacs,
                num_threads: 1,
                ..RunnerConfig::default()
            },
        )
        .unwrap();
        let parallel = run_lint(
            &[dirty, clean],
            &RunnerConfig {
                output_format: OutputFormat::Emacs,
                num_threads: 2,
                ..RunnerConfig::default()
            },
        )
        .unwrap();

        assert_eq!(serial, parallel);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn planning_skips_invalid_files_before_worker_execution() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("demo.txt");
        std::fs::write(&file, "hello\n").unwrap();

        let planned = plan_files(
            vec![(0, file)],
            &RunnerConfig {
                quiet: false,
                ..RunnerConfig::default()
            },
        );

        assert!(planned.lint_jobs.is_empty());
        assert_eq!(planned.reports.len(), 1);
        assert!(
            planned.reports[0]
                .notes
                .iter()
                .any(|note| note.text.contains("not a valid file name"))
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}
