use clap::{Args, Parser, ValueEnum};
use cpplint_core::options::{DEFAULT_LINE_LENGTH, IncludeOrder, Options};
use cpplint_core::runner::RunnerConfig;
use cpplint_core::state::{CountingStyle, OutputFormat};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ParsedCommand {
    Check(CheckArgs),
    Rule(RuleArgs),
}

pub fn parse_cli() -> ParsedCommand {
    let args: Vec<OsString> = std::env::args_os().collect();

    match args.get(1).and_then(|arg| arg.to_str()) {
        Some("check") => ParsedCommand::Check(CheckCli::parse_from(strip_subcommand(&args)).check),
        Some("rule") => ParsedCommand::Rule(RuleCli::parse_from(strip_subcommand(&args)).rule),
        _ => ParsedCommand::Check(LegacyCheckCli::parse_from(args).check),
    }
}

fn strip_subcommand(args: &[OsString]) -> Vec<OsString> {
    let mut forwarded = Vec::with_capacity(args.len().saturating_sub(1));
    if let Some(binary) = args.first() {
        forwarded.push(binary.clone());
    }
    forwarded.extend(args.iter().skip(2).cloned());
    forwarded
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliOutputFormat {
    Emacs,
    Vs7,
    Eclipse,
    Junit,
    Sed,
    Gsed,
}

impl From<CliOutputFormat> for OutputFormat {
    fn from(value: CliOutputFormat) -> Self {
        match value {
            CliOutputFormat::Emacs => OutputFormat::Emacs,
            CliOutputFormat::Vs7 => OutputFormat::Vs7,
            CliOutputFormat::Eclipse => OutputFormat::Eclipse,
            CliOutputFormat::Junit => OutputFormat::JUnit,
            CliOutputFormat::Sed => OutputFormat::Sed,
            CliOutputFormat::Gsed => OutputFormat::Gsed,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliCountingStyle {
    Total,
    Toplevel,
    Detailed,
}

impl From<CliCountingStyle> for CountingStyle {
    fn from(value: CliCountingStyle) -> Self {
        match value {
            CliCountingStyle::Total => CountingStyle::Total,
            CliCountingStyle::Toplevel => CountingStyle::Toplevel,
            CliCountingStyle::Detailed => CountingStyle::Detailed,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliIncludeOrder {
    Default,
    Standardcfirst,
}

impl From<CliIncludeOrder> for IncludeOrder {
    fn from(value: CliIncludeOrder) -> Self {
        match value {
            CliIncludeOrder::Default => IncludeOrder::Default,
            CliIncludeOrder::Standardcfirst => IncludeOrder::StandardCFirst,
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct CheckArgs {
    #[arg(long, value_enum, default_value_t = CliOutputFormat::Emacs)]
    pub output: CliOutputFormat,

    #[arg(long, short = 'v', default_value_t = 1)]
    pub verbose: i32,

    #[arg(long)]
    pub quiet: bool,

    #[arg(long, value_enum, default_value_t = CliCountingStyle::Total)]
    pub counting: CliCountingStyle,

    #[arg(long)]
    pub root: Option<PathBuf>,

    #[arg(long)]
    pub repository: Option<PathBuf>,

    #[arg(long = "linelength", default_value_t = DEFAULT_LINE_LENGTH)]
    pub line_length: usize,

    #[arg(long, value_delimiter = ',')]
    pub filter: Vec<String>,

    #[arg(long)]
    pub recursive: bool,

    #[arg(long)]
    pub exclude: Vec<String>,

    #[arg(long)]
    pub extensions: Option<String>,

    #[arg(long)]
    pub headers: Option<String>,

    #[arg(long, value_enum, default_value_t = CliIncludeOrder::Default)]
    pub includeorder: CliIncludeOrder,

    #[arg(long, default_value = "CPPLINT.cfg")]
    pub config: String,

    #[arg(long)]
    pub timing: bool,

    #[arg(long)]
    pub threads: Option<i32>,

    #[arg(required = true, value_name = "FILE")]
    pub files: Vec<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct RuleArgs {
    #[arg(value_name = "QUERY", conflicts_with = "all")]
    pub query: Option<String>,

    #[arg(long, short = 'a')]
    pub all: bool,
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "cpplint",
    bin_name = "cpplint",
    version,
    about = "C++ style checker"
)]
struct LegacyCheckCli {
    #[command(flatten)]
    check: CheckArgs,
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "cpplint",
    bin_name = "cpplint check",
    version,
    about = "Run lint checks"
)]
struct CheckCli {
    #[command(flatten)]
    check: CheckArgs,
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "cpplint",
    bin_name = "cpplint rule",
    version,
    about = "Inspect rule families and categories"
)]
struct RuleCli {
    #[command(flatten)]
    rule: RuleArgs,
}

impl CheckArgs {
    pub fn to_runner_config(&self) -> Result<RunnerConfig, String> {
        if self.verbose < 0 {
            return Err(format!(
                "Verbosity should be a non-negative integer. (--verbose={})",
                self.verbose
            ));
        }
        if self.config.contains('/') || self.config.contains('\\') {
            return Err("Config file name must not include directory components.".to_string());
        }

        let mut options = Options::new();
        options.line_length = self.line_length;
        options.config_filename = self.config.clone();
        options.include_order = self.includeorder.into();
        options.timing = self.timing;

        if let Some(root) = &self.root {
            if !root.exists() {
                return Err(format!(
                    "Root directory does not exist. (--root={})",
                    root.display()
                ));
            }
            options.root = root.clone();
        }
        if let Some(repository) = &self.repository {
            if !repository.exists() {
                return Err(format!(
                    "Repository path does not exist. (--repository={})",
                    repository.display()
                ));
            }
            options.repository = repository.clone();
        }

        if let Some(extensions) = &self.extensions {
            options.set_extensions_from_csv(extensions);
        }
        if let Some(headers) = &self.headers {
            options.set_headers_from_csv(headers);
        }
        for filter in &self.filter {
            options.add_filter(filter);
        }

        Ok(RunnerConfig {
            options,
            output_format: self.output.into(),
            counting_style: self.counting.into(),
            verbose_level: self.verbose,
            quiet: self.quiet,
            num_threads: parse_num_threads(self.threads)?,
            recursive: self.recursive,
            excludes: self.exclude.clone(),
        })
    }
}

fn parse_num_threads(threads: Option<i32>) -> Result<usize, String> {
    match threads {
        None | Some(0) | Some(-1) => std::thread::available_parallelism()
            .map(|count| count.get())
            .map_err(|error| error.to_string()),
        Some(value) if value > 0 => Ok(value as usize),
        Some(value) => Err(format!(
            "Number of threads should be a positive integer, 0, or -1. (--threads={})",
            value
        )),
    }
}
