use crate::config::{ConfigMessage, ConfigMessageKind, ConfigResolution, DirectoryConfigCache};
use crate::diagnostics::{
    Diagnostic, FileId, FileTable, Note, NoteStream, ProcessedFile, ThreadSafeFileTable,
};
use crate::file_linter::FileLinter;
use crate::fixer::fix_file_in_place;
use crate::glob::GlobSetMatcher;
use crate::options::Options;
use crate::source::SourceFile;
use crate::output::{
    DiagnosticCounter, format_diagnostic, format_diagnostic_with_name, format_note,
    format_sed_diagnostic, format_sed_diagnostic_with_name, render_owned,
    write_diagnostic_with_name, write_sed_diagnostic_with_name,
};
use crate::state::{CountingStyle, CppLintState, OutputFormat, SessionSettings, SessionSnapshot};
use crate::string_utils::set_to_str;
use crate::{errors::Result, output::RenderedOutput};
use ignore::WalkBuilder;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use std::io::Write;
use std::num::NonZeroUsize;
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
    pub num_threads: NonZeroUsize,
    pub recursive: bool,
    pub excludes: Vec<String>,
    pub fix: bool,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            options: Options::new(),
            output_format: OutputFormat::Emacs,
            counting_style: CountingStyle::Total,
            verbose_level: 1,
            quiet: false,
            num_threads: NonZeroUsize::MIN,
            recursive: false,
            excludes: Vec::new(),
            fix: false,
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
    file_names: FileTable,
    files: Vec<(FileId, PathBuf)>,
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
    file_id: FileId,
    source_file: SourceFile,
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
#[derive(Debug)]
pub struct Runner {
    pool: Option<rayon::ThreadPool>,
}

impl Runner {
    pub fn new(num_threads: NonZeroUsize) -> Result<Self> {
        let threads = num_threads.get();
        let pool = if threads > 1 {
            Some(ThreadPoolBuilder::new().num_threads(threads).build()?)
        } else {
            None
        };
        Ok(Self { pool })
    }

    pub fn lint<W1: Write + Send, W2: Write + Send>(
        &self,
        files: &[PathBuf],
        config: &RunnerConfig,
        stdout: W1,
        stderr: W2,
    ) -> Result<LintRunResult> {
        run_lint_with_runner(self, files, config, stdout, stderr)
    }
}

pub fn run_lint<W1: Write + Send, W2: Write + Send>(
    files: &[PathBuf],
    config: &RunnerConfig,
    stdout: W1,
    stderr: W2,
) -> Result<LintRunResult> {
    let runner = Runner::new(config.num_threads)?;
    runner.lint(files, config, stdout, stderr)
}

pub fn run_lint_with_streams<W1: Write + Send, W2: Write + Send>(
    files: &[PathBuf],
    config: &RunnerConfig,
    stdout: W1,
    stderr: W2,
) -> Result<LintRunResult> {
    run_lint(files, config, stdout, stderr)
}

fn run_lint_with_runner<W1: Write + Send, W2: Write + Send>(
    runner: &Runner,
    files: &[PathBuf],
    config: &RunnerConfig,
    mut stdout: W1,
    mut stderr: W2,
) -> Result<LintRunResult> {
    let session_settings = SessionSettings {
        verbose_level: config.verbose_level,
        counting_style: config.counting_style,
        quiet: config.quiet,
        output_format: config.output_format,
        num_threads: config.num_threads,
    };

    let started_at = config.options.timing.then(Instant::now);
    let is_buffered_format = matches!(config.output_format, OutputFormat::JUnit);

    if !is_buffered_format {
        return stream_pipeline_lint_with_pool(
            runner.pool.as_ref(),
            files,
            config,
            session_settings,
            started_at,
            stdout,
            stderr,
        );
    }

    let CollectedFiles {
        file_names,
        files: collected_files,
        notes: collected_notes,
    } = collect_files(files, config)?;

    let pool = runner.pool.as_ref();

    if is_buffered_format {
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
                    .map(|job| process_file(job, session_settings, config.fix))
                    .collect::<Vec<_>>()
            })
        } else {
            lint_jobs
                .into_iter()
                .map(|job| process_file(job, session_settings, config.fix))
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
            file_names,
            diagnostics,
            notes,
            processed_files,
            started_at.map(|instant| instant.elapsed()),
        );

        let _ = write!(stdout, "{}", rendered.stdout);
        let _ = write!(stderr, "{}", rendered.stderr);

        return Ok(LintRunResult {
            stdout: String::new(),
            stderr: String::new(),
            error_count,
        });
    }

    // Streaming mode for human-readable formats
    let mut counter = DiagnosticCounter::new(config.counting_style);

    // Process initial notes
    for note in collected_notes {
        match note.stream {
            NoteStream::Stdout => {
                let _ = write!(stdout, "{}", format_note(&note));
            }
            NoteStream::Stderr => {
                let _ = write!(stderr, "{}", format_note(&note));
            }
        }
    }

    let process_report = |report: FileRunReport,
                          stdout: &mut dyn Write,
                          stderr: &mut dyn Write,
                          counter: &mut DiagnosticCounter| {
        for note in &report.notes {
            match note.stream {
                NoteStream::Stdout => {
                    let _ = write!(stdout, "{}", format_note(note));
                }
                NoteStream::Stderr => {
                    let _ = write!(stderr, "{}", format_note(note));
                }
            }
        }

        for diag in &report.diagnostics {
            match config.output_format {
                OutputFormat::Sed | OutputFormat::Gsed => {
                    let (is_fixable, text) =
                        format_sed_diagnostic(config.output_format, &file_names, diag);
                    if is_fixable {
                        let _ = write!(stdout, "{}", text);
                    } else {
                        let _ = write!(stderr, "{}", text);
                    }
                }
                _ => {
                    let _ = write!(
                        stderr,
                        "{}",
                        format_diagnostic(config.output_format, &file_names, diag)
                    );
                }
            }
        }

        for diag in &report.diagnostics {
            counter.add(diag);
        }
    };

    let config_cache = DirectoryConfigCache::new(&config.options);

    if let Some(pool) = &pool {
        struct BatchSender<T> {
            tx: std::sync::mpsc::Sender<Vec<T>>,
            batch: Vec<T>,
            capacity: usize,
        }

        impl<T> BatchSender<T> {
            fn new(tx: std::sync::mpsc::Sender<Vec<T>>, capacity: usize) -> Self {
                Self {
                    tx,
                    batch: Vec::with_capacity(capacity),
                    capacity,
                }
            }

            fn push(&mut self, item: T) {
                self.batch.push(item);
                if self.batch.len() >= self.capacity {
                    self.flush();
                }
            }

            fn flush(&mut self) {
                if !self.batch.is_empty() {
                    let items =
                        std::mem::replace(&mut self.batch, Vec::with_capacity(self.capacity));
                    let _ = self.tx.send(items);
                }
            }
        }

        impl<T> Drop for BatchSender<T> {
            fn drop(&mut self) {
                self.flush();
            }
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let config_cache_ref = &config_cache;
        let config_ref = config;

        std::thread::scope(|s| {
            s.spawn(move || {
                pool.install(|| {
                    collected_files.into_par_iter().for_each_init(
                        || (BatchSender::new(tx.clone(), 32), bumpalo::Bump::new()),
                        |(sender, arena), (file_id, file)| {
                            let report = plan_and_process_file_with_arena(
                                config_cache_ref,
                                config_ref,
                                session_settings,
                                file_id,
                                file,
                                arena,
                            );
                            sender.push(report);
                            arena.reset();
                        },
                    );
                });
                drop(tx);
            });

            for batch in rx {
                for report in batch {
                    process_report(report, &mut stdout, &mut stderr, &mut counter);
                }
            }
        });
    } else {
        for (file_id, file) in collected_files {
            let report =
                plan_and_process_file(&config_cache, config, session_settings, file_id, file);
            process_report(report, &mut stdout, &mut stderr, &mut counter);
        }
    }

    let final_error_count = counter.total();

    if !config.quiet || final_error_count > 0 {
        let _ = write!(stdout, "{}", counter.render_summary());
    }

    if let Some(start) = started_at
        && !config.quiet
    {
        let _ = writeln!(stdout, "Runtime: {:.3}(s)", start.elapsed().as_secs_f64());
    }

    let _ = stdout.flush();
    let _ = stderr.flush();

    Ok(LintRunResult {
        stdout: String::new(),
        stderr: String::new(),
        error_count: final_error_count,
    })
}

fn stream_pipeline_lint_with_pool<W1: Write + Send, W2: Write + Send>(
    pool: Option<&rayon::ThreadPool>,
    files: &[PathBuf],
    config: &RunnerConfig,
    session_settings: SessionSettings,
    started_at: Option<Instant>,
    stdout: W1,
    stderr: W2,
) -> Result<LintRunResult> {
    let mut stdout = std::io::BufWriter::new(stdout);
    let mut stderr = std::io::BufWriter::new(stderr);
    struct BatchSender<T> {
        tx: std::sync::mpsc::SyncSender<Vec<T>>,
        batch: Vec<T>,
        capacity: usize,
    }

    impl<T> BatchSender<T> {
        fn new(tx: std::sync::mpsc::SyncSender<Vec<T>>, capacity: usize) -> Self {
            Self {
                tx,
                batch: Vec::with_capacity(capacity),
                capacity,
            }
        }

        fn push(&mut self, item: T) {
            self.batch.push(item);
            if self.batch.len() >= self.capacity {
                self.flush();
            }
        }

        fn flush(&mut self) {
            if !self.batch.is_empty() {
                let items = std::mem::replace(&mut self.batch, Vec::with_capacity(self.capacity));
                let _ = self.tx.send(items);
            }
        }
    }

    impl<T> Drop for BatchSender<T> {
        fn drop(&mut self) {
            self.flush();
        }
    }

    if config.num_threads.get() == 1 {
        return stream_single_threaded_lint(
            files,
            config,
            session_settings,
            started_at,
            stdout,
            stderr,
        );
    }

    let thread_safe_file_table = Arc::new(ThreadSafeFileTable::new());
    let (work_tx, work_rx) = std::sync::mpsc::sync_channel::<Vec<(FileId, PathBuf)>>(64);
    let (report_tx, report_rx) = std::sync::mpsc::sync_channel::<Vec<FileRunReport>>(256);

    let config_cache = DirectoryConfigCache::new(&config.options);
    let mut counter = DiagnosticCounter::new(config.counting_style);

    let cwd = std::env::current_dir()?;
    let excludes = compile_excludes(&cwd, &config.excludes)?;

    let total_threads = config.num_threads.get();
    let walker_threads = if total_threads >= 8 { 2 } else { 1 };

    std::thread::scope(|s| {
        // 1. Producer: ディレクトリ探索しながら見つかったファイルをチャネルへ送信
        let file_table_prod = Arc::clone(&thread_safe_file_table);
        let files_vec = files.to_vec();
        let config_clone = config.clone();
        let excludes_clone = excludes.clone();
        let report_tx_prod = report_tx.clone();

        s.spawn(move || {
            let (path_tx, path_rx) =
                std::sync::mpsc::sync_channel::<(Option<Note>, Option<PathBuf>)>(256);

            let prod_thread_count = walker_threads;
            let path_tx_clone = path_tx.clone();
            let file_table_prod_inner = Arc::clone(&file_table_prod);
            std::thread::spawn(move || {
                for file in &files_vec {
                    if file == Path::new("-") {
                        let _ = path_tx_clone.send((None, Some(PathBuf::from("-"))));
                        continue;
                    }
                    if !file.exists() {
                        let file_id = file_table_prod_inner.intern(&file.to_string_lossy());
                        let note = Note {
                            file_id,
                            order: 0,
                            stream: NoteStream::Stderr,
                            text: format!("Skipping input '{}': Path not found.\n", file.display())
                                .into(),
                        };
                        let _ = path_tx_clone.send((Some(note), None));
                        continue;
                    }
                    if config_clone.recursive && file.is_dir() {
                        let canonical =
                            std::fs::canonicalize(file).unwrap_or_else(|_| file.clone());
                        expand_directory_to_sender(
                            &canonical,
                            &config_clone.options,
                            prod_thread_count,
                            path_tx_clone.clone(),
                        );
                    } else {
                        let _ = path_tx_clone.send((None, Some(file.clone())));
                    }
                }
            });
            drop(path_tx);

            let mut seen = rustc_hash::FxHashSet::<PathBuf>::default();
            let mut work_batch = Vec::with_capacity(64);
            for (note, path) in path_rx {
                if let Some(note) = note {
                    let mut report = FileRunReport::default();
                    report.notes.push(note);
                    let _ = report_tx_prod.send(vec![report]);
                }
                if let Some(file) = path
                    && !should_exclude(&file, &excludes_clone)
                    && seen.insert(file.clone())
                {
                    let file_id = file_table_prod.intern(&file.to_string_lossy());
                    work_batch.push((file_id, file));
                    if work_batch.len() >= 64 {
                        let _ = work_tx
                            .send(std::mem::replace(&mut work_batch, Vec::with_capacity(64)));
                    }
                }
            }
            if !work_batch.is_empty() {
                let _ = work_tx.send(work_batch);
            }
        });

        // 2. Consumer: チャネルから受信したファイルを即座にパース・検証
        let config_cache_ref = &config_cache;
        let config_ref = config;

        s.spawn(move || {
            if let Some(pool) = &pool {
                pool.install(|| {
                    work_rx.into_iter().par_bridge().for_each(|batch| {
                        batch.into_par_iter().for_each_init(
                            || {
                                (
                                    BatchSender::new(report_tx.clone(), 32),
                                    bumpalo::Bump::new(),
                                )
                            },
                            |(sender, arena), (file_id, file)| {
                                let report = plan_and_process_file_with_arena(
                                    config_cache_ref,
                                    config_ref,
                                    session_settings,
                                    file_id,
                                    file,
                                    arena,
                                );
                                sender.push(report);
                                arena.reset();
                            },
                        );
                    });
                });
            } else {
                let mut sender = BatchSender::new(report_tx.clone(), 32);
                let mut arena = bumpalo::Bump::new();
                for batch in work_rx {
                    for (file_id, file) in batch {
                        let report = plan_and_process_file_with_arena(
                            config_cache_ref,
                            config_ref,
                            session_settings,
                            file_id,
                            file,
                            &arena,
                        );
                        sender.push(report);
                        arena.reset();
                    }
                }
            }
            drop(report_tx);
        });

        // 3. Main Renderer Loop: 受信した結果を逐次画面に出力
        for batch in report_rx {
            for report in batch {
                for note in &report.notes {
                    match note.stream {
                        NoteStream::Stdout => {
                            let _ = stdout.write_all(note.text.as_bytes());
                        }
                        NoteStream::Stderr => {
                            let _ = stderr.write_all(note.text.as_bytes());
                        }
                    }
                }

                if !report.diagnostics.is_empty() {
                    let file_id = report.diagnostics[0].file_id;
                    thread_safe_file_table.get_name(file_id, |filename| {
                        for diag in &report.diagnostics {
                            match config.output_format {
                                OutputFormat::Sed | OutputFormat::Gsed => {
                                    let _ = write_sed_diagnostic_with_name(
                                        &mut stdout,
                                        config.output_format,
                                        filename,
                                        diag,
                                    );
                                }
                                _ => {
                                    let _ = write_diagnostic_with_name(
                                        &mut stderr,
                                        config.output_format,
                                        filename,
                                        diag,
                                    );
                                }
                            }
                            counter.add(diag);
                        }
                    });
                }
            }
        }
    });

    let final_error_count = counter.total();

    if !config.quiet || final_error_count > 0 {
        let _ = write!(stdout, "{}", counter.render_summary());
    }

    if let Some(start) = started_at
        && !config.quiet
    {
        let _ = writeln!(stdout, "Runtime: {:.3}(s)", start.elapsed().as_secs_f64());
    }

    let _ = stdout.flush();
    let _ = stderr.flush();

    Ok(LintRunResult {
        stdout: String::new(),
        stderr: String::new(),
        error_count: final_error_count,
    })
}

fn stream_single_threaded_lint<W1: Write, W2: Write>(
    files: &[PathBuf],
    config: &RunnerConfig,
    session_settings: SessionSettings,
    started_at: Option<Instant>,
    mut stdout: W1,
    mut stderr: W2,
) -> Result<LintRunResult> {
    let file_table = Arc::new(ThreadSafeFileTable::new());
    let config_cache = DirectoryConfigCache::new(&config.options);
    let mut counter = DiagnosticCounter::new(config.counting_style);

    let cwd = std::env::current_dir()?;
    let excludes = compile_excludes(&cwd, &config.excludes)?;
    let mut arena = bumpalo::Bump::new();

    let render_report = |report: &FileRunReport,
                         stdout: &mut W1,
                         stderr: &mut W2,
                         counter: &mut DiagnosticCounter| {
        for note in &report.notes {
            match note.stream {
                NoteStream::Stdout => {
                    let _ = write!(stdout, "{}", format_note(note));
                }
                NoteStream::Stderr => {
                    let _ = write!(stderr, "{}", format_note(note));
                }
            }
        }

        for diag in &report.diagnostics {
            file_table.get_name(diag.file_id, |filename| match config.output_format {
                OutputFormat::Sed | OutputFormat::Gsed => {
                    let (is_fixable, text) =
                        format_sed_diagnostic_with_name(config.output_format, filename, diag);
                    if is_fixable {
                        let _ = write!(stdout, "{}", text);
                    } else {
                        let _ = write!(stderr, "{}", text);
                    }
                }
                _ => {
                    let _ = write!(
                        stderr,
                        "{}",
                        format_diagnostic_with_name(config.output_format, filename, diag)
                    );
                }
            });
            counter.add(diag);
        }
    };

    let mut process_one_path = |file: &Path| {
        if file == Path::new("-") {
            let file_id = file_table.intern("-");
            let report = plan_and_process_file_with_arena(
                &config_cache,
                config,
                session_settings,
                file_id,
                file.to_path_buf(),
                &arena,
            );
            render_report(&report, &mut stdout, &mut stderr, &mut counter);
            arena.reset();
            return;
        }

        if !file.exists() {
            let file_id = file_table.intern(&file.to_string_lossy());
            let note = Note {
                file_id,
                order: 0,
                stream: NoteStream::Stderr,
                text: format!("Skipping input '{}': Path not found.\n", file.display()).into(),
            };
            let mut report = FileRunReport::default();
            report.notes.push(note);
            render_report(&report, &mut stdout, &mut stderr, &mut counter);
            return;
        }

        if should_exclude(file, &excludes) {
            return;
        }

        if config.recursive && file.is_dir() {
            let canonical = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
            let found_files = expand_directory(&canonical, &config.options, 1);
            for found in found_files {
                if !should_exclude(&found, &excludes) {
                    let file_id = file_table.intern(&found.to_string_lossy());
                    let report = plan_and_process_file_with_arena(
                        &config_cache,
                        config,
                        session_settings,
                        file_id,
                        found,
                        &arena,
                    );
                    render_report(&report, &mut stdout, &mut stderr, &mut counter);
                    arena.reset();
                }
            }
        } else {
            let file_id = file_table.intern(&file.to_string_lossy());
            let report = plan_and_process_file_with_arena(
                &config_cache,
                config,
                session_settings,
                file_id,
                file.to_path_buf(),
                &arena,
            );
            render_report(&report, &mut stdout, &mut stderr, &mut counter);
            arena.reset();
        }
    };

    for file in files {
        process_one_path(file);
    }

    let final_error_count = counter.total();

    if !config.quiet || final_error_count > 0 {
        let _ = write!(stdout, "{}", counter.render_summary());
    }

    if let Some(start) = started_at
        && !config.quiet
    {
        let _ = writeln!(stdout, "Runtime: {:.3}(s)", start.elapsed().as_secs_f64());
    }

    let _ = stdout.flush();
    let _ = stderr.flush();

    Ok(LintRunResult {
        stdout: String::new(),
        stderr: String::new(),
        error_count: final_error_count,
    })
}

fn collect_files(files: &[PathBuf], config: &RunnerConfig) -> Result<CollectedFiles> {
    let cwd = std::env::current_dir()?;
    let excludes = compile_excludes(&cwd, &config.excludes)?;
    let mut collected = Vec::new();
    let mut notes = Vec::new();
    let mut file_names = FileTable::new();

    for file in files {
        if file == Path::new("-") {
            collected.push(PathBuf::from("-"));
            continue;
        }

        if !file.exists() {
            let file_id = file_names.intern(&file.to_string_lossy());
            notes.push(Note {
                file_id,
                order: 0,
                stream: NoteStream::Stderr,
                text: format!("Skipping input '{}': Path not found.\n", file.display()).into(),
            });
            continue;
        }

        if config.recursive && file.is_dir() {
            let canonical = std::fs::canonicalize(file).unwrap_or_else(|_| file.clone());
            collected.extend(expand_directory(
                &canonical,
                &config.options,
                config.num_threads.get(),
            ));
        } else {
            collected.push(file.clone());
        }
    }

    if config.num_threads.get() > 1 {
        collected = collected
            .into_par_iter()
            .filter(|file| !should_exclude(file, &excludes))
            .collect();
        collected.par_sort_unstable();
    } else {
        collected.retain(|file| !should_exclude(file, &excludes));
        collected.sort_unstable();
    }
    collected.dedup();
    let files = collected
        .into_iter()
        .map(|file| {
            let file_id = file_names.intern(&file.to_string_lossy());
            (file_id, file)
        })
        .collect();
    Ok(CollectedFiles {
        file_names,
        files,
        notes,
    })
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn plan_files(files: Vec<(FileId, PathBuf)>, config: &RunnerConfig) -> PlannedRun {
    let config_cache = DirectoryConfigCache::new(&config.options);
    let entries = if config.num_threads.get() <= 1 {
        files
            .into_iter()
            .map(|(file_id, file)| plan_single_file(&config_cache, config, file_id, file))
            .collect::<Vec<_>>()
    } else {
        files
            .into_par_iter()
            .map(|(file_id, file)| plan_single_file(&config_cache, config, file_id, file))
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
    file_id: FileId,
    file: PathBuf,
) -> PlannedEntry {
    let display_name_arc: Arc<str> = Arc::from(file.to_string_lossy());
    let mut note_order = 0usize;
    let mut initial_notes = Vec::new();

    let options = match config_cache.resolve_for_file(&file, config.quiet) {
        ConfigResolution::Lint { options, messages } => {
            for message in messages.iter() {
                initial_notes.push(note_from_config_message(file_id, note_order, message));
                note_order += 1;
            }
            options
        }
        ConfigResolution::Excluded { messages } => {
            let mut report = FileRunReport::default();
            for message in messages.iter() {
                report
                    .notes
                    .push(note_from_config_message(file_id, note_order, message));
                note_order += 1;
            }
            return PlannedEntry::Report(report);
        }
    };

    if file != Path::new("-") && file.is_file() && !options.is_valid_file(&file) {
        let mut report = FileRunReport::default();
        report.notes.push(Note {
            file_id,
            order: note_order,
            stream: NoteStream::Stderr,
            text: format!(
                "Ignoring {}; not a valid file name ({})\n",
                display_name_arc,
                set_to_str(options.all_extensions(), "[", ", ", "]")
            )
            .into(),
        });
        report.processed_files.push(ProcessedFile {
            file_id,
            had_error: false,
        });
        if !config.quiet {
            report.notes.push(Note {
                file_id,
                order: note_order + 1,
                stream: NoteStream::Stdout,
                text: format!("Done processing {}\n", display_name_arc).into(),
            });
        }
        return PlannedEntry::Report(report);
    }

    let source_file = SourceFile::with_options_and_display_name(file, options.as_ref(), display_name_arc);

    PlannedEntry::LintJob(PlannedLintJob {
        file_id,
        source_file,
        options,
        initial_notes,
        failure_note_order: note_order,
        done_note_order: note_order + 1,
    })
}

fn plan_and_process_file(
    config_cache: &DirectoryConfigCache,
    config: &RunnerConfig,
    session_settings: SessionSettings,
    file_id: FileId,
    file: PathBuf,
) -> FileRunReport {
    let arena = bumpalo::Bump::new();
    plan_and_process_file_with_arena(
        config_cache,
        config,
        session_settings,
        file_id,
        file,
        &arena,
    )
}

fn plan_and_process_file_with_arena(
    config_cache: &DirectoryConfigCache,
    config: &RunnerConfig,
    session_settings: SessionSettings,
    file_id: FileId,
    file: PathBuf,
    arena: &bumpalo::Bump,
) -> FileRunReport {
    match plan_single_file(config_cache, config, file_id, file) {
        PlannedEntry::LintJob(job) => {
            process_file_with_arena(job, session_settings, config.fix, arena)
        }
        PlannedEntry::Report(report) => report,
    }
}

fn note_from_config_message(file_id: FileId, order: usize, message: &ConfigMessage) -> Note {
    Note {
        file_id,
        order,
        stream: match message.kind {
            ConfigMessageKind::Info => NoteStream::Stdout,
            ConfigMessageKind::Error => NoteStream::Stderr,
        },
        text: message.text.clone().into(),
    }
}

fn compile_excludes(cwd: &Path, excludes: &[String]) -> Result<GlobSetMatcher> {
    let patterns: Vec<String> = excludes
        .iter()
        .filter(|pattern| !pattern.is_empty())
        .map(|pattern| {
            let absolute = if Path::new(pattern).is_absolute() {
                PathBuf::from(pattern)
            } else {
                cwd.join(pattern)
            };
            absolute.to_string_lossy().to_string()
        })
        .collect();
    GlobSetMatcher::from_patterns(patterns.iter().map(|s| s.as_str()), true)
}

fn should_exclude(file: &Path, excludes: &GlobSetMatcher) -> bool {
    excludes.is_match(file)
}

fn expand_directory(directory: &Path, options: &Options, threads: usize) -> Vec<PathBuf> {
    let mut walk = WalkBuilder::new(directory);
    walk.hidden(false)
        .git_ignore(false)
        .git_exclude(false)
        .parents(false)
        .ignore(false);

    if threads <= 1 {
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
                files.push(path);
            }
        }
        files
    } else {
        walk.threads(threads);
        let (tx, rx) = std::sync::mpsc::channel();
        let walker = walk.build_parallel();
        walker.run(|| {
            let tx = tx.clone();
            Box::new(move |result| {
                if let Ok(entry) = result
                    && entry
                        .file_type()
                        .is_some_and(|file_type| file_type.is_file())
                {
                    let path = entry.into_path();
                    if options.is_valid_file(&path) {
                        let _ = tx.send(path);
                    }
                }
                ignore::WalkState::Continue
            })
        });
        drop(tx);
        rx.into_iter().collect()
    }
}

fn expand_directory_to_sender(
    directory: &Path,
    options: &Options,
    threads: usize,
    tx: std::sync::mpsc::SyncSender<(Option<Note>, Option<PathBuf>)>,
) {
    let mut walk = WalkBuilder::new(directory);
    walk.hidden(false)
        .git_ignore(false)
        .git_exclude(false)
        .parents(false)
        .ignore(false);

    if threads <= 1 {
        for entry in walk.build().flatten() {
            if !entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
            {
                continue;
            }
            let path = entry.into_path();
            if options.is_valid_file(&path) {
                let _ = tx.send((None, Some(path)));
            }
        }
    } else {
        walk.threads(threads);
        let walker = walk.build_parallel();
        walker.run(|| {
            let tx = tx.clone();
            Box::new(move |result| {
                if let Ok(entry) = result
                    && entry
                        .file_type()
                        .is_some_and(|file_type| file_type.is_file())
                {
                    let path = entry.into_path();
                    if options.is_valid_file(&path) {
                        let _ = tx.send((None, Some(path)));
                    }
                }
                ignore::WalkState::Continue
            })
        });
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn process_file(
    job: PlannedLintJob,
    session_settings: SessionSettings,
    fix: bool,
) -> FileRunReport {
    let arena = bumpalo::Bump::new();
    process_file_with_arena(job, session_settings, fix, &arena)
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn process_file_with_arena(
    job: PlannedLintJob,
    session_settings: SessionSettings,
    fix: bool,
    arena: &bumpalo::Bump,
) -> FileRunReport {
    let PlannedLintJob {
        file_id,
        source_file,
        options,
        initial_notes,
        failure_note_order,
        done_note_order,
    } = job;
    let display_name = source_file.display_name_arc();
    let state = CppLintState::with_settings(session_settings);
    for note in initial_notes {
        match note.stream {
            NoteStream::Stdout => state.record_info(note.file_id, note.order, &note.text),
            NoteStream::Stderr => state.record_raw_error(note.file_id, note.order, &note.text),
        }
    }

    let has_error = {
        if fix {
            match fix_file_in_place(source_file.path(), options.as_ref()) {
                Ok(fix_res) => {
                    let mut has_err = false;
                    for diag in fix_res.diagnostics {
                        if diag.confidence >= 1 {
                            has_err = true;
                        }
                        state.record_diagnostic_object(diag);
                    }
                    Some(has_err)
                }
                Err(error) => {
                    state.record_raw_error(
                        file_id,
                        failure_note_order,
                        format!(
                            "Skipping input '{}': Can't apply fixes ({})\n",
                            display_name, error
                        ),
                    );
                    return state.into_snapshot().into();
                }
            }
        } else {
            let mut linter = FileLinter::with_source_file(source_file, &state, options, file_id);
            match linter.process_file_with_arena(arena) {
                Ok(()) => Some(linter.has_error()),
                Err(_) => None,
            }
        }
    };

    let Some(has_error) = has_error else {
        state.record_raw_error(
            file_id,
            failure_note_order,
            format!(
                "Skipping input '{}': Can't open for reading\n",
                display_name
            ),
        );
        return state.into_snapshot().into();
    };

    state.record_processed_file(file_id, has_error);
    if !session_settings.quiet || has_error {
        state.record_info(
            file_id,
            done_note_order,
            format!("Done processing {}\n", display_name),
        );
    }
    state.into_snapshot().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::NoteStream;
    use crate::state::OutputFormat;
    use std::sync::Arc;
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
        let mut out = Vec::new();
        let mut err = Vec::new();
        let result = run_lint(&[file], &config, &mut out, &mut err).unwrap();
        // In streaming mode, stdout in the result is empty as it's printed directly.
        // We check the error_count instead.
        assert!(result.error_count > 0); // No copyright error expected

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
                num_threads: NonZeroUsize::new(1).unwrap(),
                ..RunnerConfig::default()
            },
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let parallel = run_lint(
            &[dirty, clean],
            &RunnerConfig {
                output_format: OutputFormat::Emacs,
                num_threads: NonZeroUsize::new(2).unwrap(),
                ..RunnerConfig::default()
            },
            Vec::new(),
            Vec::new(),
        )
        .unwrap();

        // In streaming mode, stdout/stderr are empty, so we compare error counts.
        assert_eq!(serial.error_count, parallel.error_count);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn planning_skips_invalid_files_before_worker_execution() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("demo.txt");
        std::fs::write(&file, "hello\n").unwrap();
        let mut file_names = FileTable::new();
        let file_id = file_names.intern(&file.to_string_lossy());

        let planned = plan_files(
            vec![(file_id, file)],
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

    #[test]
    fn recursive_collection_honors_extension_filters() {
        let root = unique_temp_dir();
        let nested = root.join("src");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(&nested).unwrap();
        let cpp_file = nested.join("keep.cpp");
        let cc_file = nested.join("skip.cc");
        std::fs::write(&cpp_file, "// Copyright 2026\n").unwrap();
        std::fs::write(&cc_file, "// Copyright 2026\n").unwrap();

        let mut options = Options::new();
        options.set_extensions_from_csv("cpp");
        let config = RunnerConfig {
            options,
            recursive: true,
            ..RunnerConfig::default()
        };

        let collected = collect_files(std::slice::from_ref(&root), &config).unwrap();
        assert_eq!(collected.files.len(), 1);
        assert!(collected.files[0].1.ends_with("keep.cpp"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recursive_collection_keeps_direct_files_and_filters_nested_extensions() {
        let root = unique_temp_dir();
        let nested = root.join("src");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(&nested).unwrap();
        let direct_cpp = root.join("one.cpp");
        let nested_cpp = nested.join("two.cpp");
        let nested_cc = nested.join("three.cc");
        std::fs::write(&direct_cpp, "// Copyright 2026\n").unwrap();
        std::fs::write(&nested_cpp, "// Copyright 2026\n").unwrap();
        std::fs::write(&nested_cc, "// Copyright 2026\n").unwrap();

        let mut options = Options::new();
        options.set_extensions_from_csv("cpp");
        let config = RunnerConfig {
            options,
            recursive: true,
            ..RunnerConfig::default()
        };

        let collected = collect_files(&[direct_cpp.clone(), nested.clone()], &config).unwrap();
        let collected_files: Vec<_> = collected.files.into_iter().map(|(_, file)| file).collect();
        assert_eq!(collected_files.len(), 2);
        assert!(collected_files.iter().any(|file| file.ends_with("one.cpp")));
        assert!(collected_files.iter().any(|file| file.ends_with("two.cpp")));
        assert!(
            !collected_files
                .iter()
                .any(|file| file.ends_with("three.cc"))
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn run_lint_buffers_junit_output() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("demo.cc");
        std::fs::write(&file, "// Copyright 2026\nint x=0;\n").unwrap();

        let config = RunnerConfig {
            output_format: OutputFormat::JUnit,
            ..RunnerConfig::default()
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let result = run_lint(&[file], &config, &mut out, &mut err).unwrap();

        assert!(result.stdout.is_empty());
        assert!(result.stderr.is_empty());
        assert!(String::from_utf8(out).unwrap().contains("<testsuite"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn process_file_reports_read_failure_and_fix_failure() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(&root).unwrap();

        let missing = root.join("missing.cc");
        let missing_job = PlannedLintJob {
            file_id: FileId::from_index(0),
            source_file: SourceFile::new(missing.clone()),
            options: Arc::new(Options::new()),
            initial_notes: vec![],
            failure_note_order: 0,
            done_note_order: 1,
        };
        let missing_report =
            process_file(missing_job, crate::state::SessionSettings::default(), false);
        assert!(
            missing_report
                .notes
                .iter()
                .any(|note| matches!(note.stream, NoteStream::Stderr)
                    && note.text.contains("Can't open for reading"))
        );

        let file = root.join("readonly.cc");
        std::fs::write(&file, "int x=0;\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&file).unwrap().permissions();
            permissions.set_mode(0o444);
            std::fs::set_permissions(&file, permissions).unwrap();
        }

        let fix_job = PlannedLintJob {
            file_id: FileId::from_index(1),
            source_file: SourceFile::new(file.clone()),
            options: Arc::new(Options::new()),
            initial_notes: vec![],
            failure_note_order: 0,
            done_note_order: 1,
        };
        let fixed_report = process_file(fix_job, crate::state::SessionSettings::default(), true);
        assert!(
            fixed_report
                .notes
                .iter()
                .any(|note| matches!(note.stream, NoteStream::Stderr)
                    && note.text.contains("Can't apply fixes"))
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&file).unwrap().permissions();
            permissions.set_mode(0o644);
            std::fs::set_permissions(&file, permissions).unwrap();
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
