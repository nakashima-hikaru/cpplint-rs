use clap::{Args, Parser, ValueEnum};
use cpplint_core::options::{DEFAULT_LINE_LENGTH, IncludeOrder, Options};
use cpplint_core::runner::RunnerConfig;
use cpplint_core::state::{CountingStyle, OutputFormat};
use std::ffi::OsString;
use std::path::PathBuf;

const DEFAULT_AUTO_THREADS_CAP: usize = 4;

#[derive(Debug, Clone)]
pub enum ParsedCommand {
    Check(CheckArgs),
    Rule(RuleArgs),
}

pub fn parse_cli() -> ParsedCommand {
    let args: Vec<OsString> = std::env::args_os().collect();
    parse_args(args)
}

pub fn parse_args(args: impl IntoIterator<Item = OsString>) -> ParsedCommand {
    let args_vec: Vec<OsString> = args.into_iter().collect();
    match args_vec.get(1).and_then(|arg| arg.to_str()) {
        Some("check") => {
            ParsedCommand::Check(CheckCli::parse_from(strip_subcommand(&args_vec)).check)
        }
        Some("rule") => ParsedCommand::Rule(RuleCli::parse_from(strip_subcommand(&args_vec)).rule),
        _ => ParsedCommand::Check(LegacyCheckCli::parse_from(args_vec).check),
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

    #[arg(
        long,
        help = "Number of worker threads. Default uses up to 4 threads; 0 or -1 uses all available CPUs."
    )]
    pub threads: Option<i32>,

    #[arg(long)]
    pub fix: bool,

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
            fix: self.fix,
        })
    }
}

fn parse_num_threads(threads: Option<i32>) -> Result<usize, String> {
    match threads {
        None => std::thread::available_parallelism()
            .map(|count| count.get().min(DEFAULT_AUTO_THREADS_CAP))
            .map_err(|error| error.to_string()),
        Some(0) | Some(-1) => std::thread::available_parallelism()
            .map(|count| count.get())
            .map_err(|error| error.to_string()),
        Some(value) if value > 0 => Ok(value as usize),
        Some(value) => Err(format!(
            "Number of threads should be a positive integer, 0, or -1. (--threads={})",
            value
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_num_threads_caps_default_auto_value() {
        let parsed = parse_num_threads(None).unwrap();
        assert!((1..=DEFAULT_AUTO_THREADS_CAP).contains(&parsed));
    }

    #[test]
    fn parse_num_threads_keeps_explicit_positive_value() {
        assert_eq!(parse_num_threads(Some(2)).unwrap(), 2);
    }

    #[test]
    fn parse_num_threads_allows_uncapped_auto_values() {
        let available = std::thread::available_parallelism().unwrap().get();
        assert_eq!(parse_num_threads(Some(0)).unwrap(), available);
        assert_eq!(parse_num_threads(Some(-1)).unwrap(), available);
    }

    fn get_default_check_args() -> CheckArgs {
        LegacyCheckCli::parse_from(["cpplint", "foo.cc"]).check
    }

    #[test]
    fn to_runner_config_returns_error_for_negative_verbosity() {
        let mut args = get_default_check_args();
        args.verbose = -1;
        let result = args.to_runner_config();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Verbosity should be a non-negative integer. (--verbose=-1)"
        );
    }

    #[test]
    fn to_runner_config_returns_error_for_invalid_config_filename() {
        let mut args = get_default_check_args();
        args.config = "dir/CPPLINT.cfg".to_string();
        let result = args.to_runner_config();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Config file name must not include directory components."
        );

        args.config = "dir\\CPPLINT.cfg".to_string();
        let result = args.to_runner_config();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Config file name must not include directory components."
        );
    }

    #[test]
    fn to_runner_config_returns_error_for_missing_root_directory() {
        let mut args = get_default_check_args();
        args.root = Some(PathBuf::from("does_not_exist_dir_12345"));
        let result = args.to_runner_config();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Root directory does not exist")
        );
    }

    #[test]
    fn to_runner_config_returns_error_for_missing_repository() {
        let mut args = get_default_check_args();
        args.repository = Some(PathBuf::from("does_not_exist_dir_12345"));
        let result = args.to_runner_config();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Repository path does not exist")
        );
    }

    #[test]
    fn to_runner_config_returns_error_for_invalid_threads() {
        let mut args = get_default_check_args();
        args.threads = Some(-2);
        let result = args.to_runner_config();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Number of threads should be a positive integer, 0, or -1")
        );
    }


    #[test]
    fn test_parse_args_legacy_check() {
        let args: Vec<OsString> = vec!["cpplint".into(), "--verbose=3".into(), "foo.cpp".into()];
        let parsed = parse_args(args);
        match parsed {
            ParsedCommand::Check(check_args) => {
                assert_eq!(check_args.verbose, 3);
                assert_eq!(check_args.files, vec![PathBuf::from("foo.cpp")]);
            }
            _ => panic!("Expected ParsedCommand::Check"),
        }
    }

    #[test]
    fn test_parse_args_explicit_check() {
        let args: Vec<OsString> = vec![
            "cpplint".into(),
            "check".into(),
            "--quiet".into(),
            "bar.cpp".into(),
        ];
        let parsed = parse_args(args);
        match parsed {
            ParsedCommand::Check(check_args) => {
                assert!(check_args.quiet);
                assert_eq!(check_args.files, vec![PathBuf::from("bar.cpp")]);
            }
            _ => panic!("Expected ParsedCommand::Check"),
        }
    }

    #[test]
    fn test_parse_args_rule_subcommand() {
        let args: Vec<OsString> = vec!["cpplint".into(), "rule".into(), "--all".into()];
        let parsed = parse_args(args);
        match parsed {
            ParsedCommand::Rule(rule_args) => {
                assert!(rule_args.all);
                assert!(rule_args.query.is_none());
            }
            _ => panic!("Expected ParsedCommand::Rule"),
        }
    }

    #[test]
    fn test_parse_args_flags() {
        let args: Vec<OsString> = vec![
            "cpplint".into(),
            "--linelength=100".into(),
            "--counting=detailed".into(),
            "--exclude=src/exclude.cpp".into(),
            "main.cpp".into(),
        ];
        let parsed = parse_args(args);
        match parsed {
            ParsedCommand::Check(check_args) => {
                assert_eq!(check_args.line_length, 100);
                assert!(matches!(check_args.counting, CliCountingStyle::Detailed));
                assert_eq!(check_args.exclude, vec!["src/exclude.cpp"]);
                assert_eq!(check_args.files, vec![PathBuf::from("main.cpp")]);
            }
            _ => panic!("Expected ParsedCommand::Check"),
        }
    }
}
