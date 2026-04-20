use clap::{Args, Parser, ValueEnum};
use cpplint_core::options::{DEFAULT_LINE_LENGTH, Filter, IncludeOrder, Options};
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

    #[arg(long, short = 'v', alias = "v", default_value_t = 1)]
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
        if self.verbose <= 0 {
            return Err(format!(
                "Verbosity should be a positive integer. (--verbose={})",
                self.verbose
            ));
        }
        if self.line_length == 0 {
            return Err("Line length should be a positive integer.".to_string());
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
            if Filter::parse(filter).is_none() {
                return Err(format!("Invalid filter: --filter={}", filter));
            }
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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("cpplint-rs-{}-{}", prefix, unique))
    }

    fn base_check_args() -> CheckArgs {
        CheckArgs {
            output: CliOutputFormat::Emacs,
            verbose: 1,
            quiet: false,
            counting: CliCountingStyle::Total,
            root: None,
            repository: None,
            line_length: DEFAULT_LINE_LENGTH,
            filter: Vec::new(),
            recursive: false,
            exclude: Vec::new(),
            extensions: None,
            headers: None,
            includeorder: CliIncludeOrder::Default,
            config: "CPPLINT.cfg".to_string(),
            timing: false,
            threads: None,
            fix: false,
            files: vec![PathBuf::from("sample.cc")],
        }
    }

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

    #[test]
    fn to_runner_config_rejects_invalid_inputs() {
        let mut args = base_check_args();
        args.verbose = -1;
        let err = args.to_runner_config().unwrap_err();
        assert!(err.contains("Verbosity should be a positive integer"));

        let mut args = base_check_args();
        args.verbose = 0;
        let err = args.to_runner_config().unwrap_err();
        assert!(err.contains("Verbosity should be a positive integer"));

        let mut args = base_check_args();
        args.line_length = 0;
        let err = args.to_runner_config().unwrap_err();
        assert!(err.contains("Line length should be a positive integer"));

        let mut args = base_check_args();
        args.config = "dir/CPPLINT.cfg".to_string();
        let err = args.to_runner_config().unwrap_err();
        assert!(err.contains("must not include directory components"));

        let mut args = base_check_args();
        args.filter = vec!["foo".to_string()];
        let err = args.to_runner_config().unwrap_err();
        assert!(err.contains("Invalid filter"));

        let mut args = base_check_args();
        args.filter = vec!["".to_string()];
        let err = args.to_runner_config().unwrap_err();
        assert!(err.contains("Invalid filter"));

        let mut args = base_check_args();
        args.root = Some(PathBuf::from("/definitely/missing"));
        let err = args.to_runner_config().unwrap_err();
        assert!(err.contains("--root=/definitely/missing"));
    }

    #[test]
    fn test_parse_arguments() {
        assert!(LegacyCheckCli::try_parse_from(["cpplint", "--help"]).is_err());
        assert!(LegacyCheckCli::try_parse_from(["cpplint", "--version"]).is_err());
        assert!(LegacyCheckCli::try_parse_from(["cpplint", "--output=blah", "foo.cc"]).is_err());
        assert!(LegacyCheckCli::try_parse_from(["cpplint", "--v=f", "foo.cc"]).is_err());
        assert!(LegacyCheckCli::try_parse_from(["cpplint", "--headers"]).is_err());

        let parsed = LegacyCheckCli::try_parse_from([
            "cpplint",
            "--filter=+runtime/printf,-whitespace",
            "--linelength=120",
            "--v=1",
            "foo.cc",
        ])
        .unwrap();
        assert_eq!(parsed.check.verbose, 1);
        assert_eq!(parsed.check.line_length, 120);
        assert_eq!(parsed.check.filter, vec!["+runtime/printf", "-whitespace"]);

        let config = parsed.check.to_runner_config().unwrap();
        assert_eq!(config.options.line_length, 120);
        assert_eq!(config.verbose_level, 1);
    }

    #[test]
    fn to_runner_config_transfers_normalized_options() {
        let root = unique_temp_dir("root");
        let repository = unique_temp_dir("repo");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&repository).unwrap();

        let args = CheckArgs {
            output: CliOutputFormat::Vs7,
            verbose: 2,
            quiet: true,
            counting: CliCountingStyle::Detailed,
            root: Some(root.clone()),
            repository: Some(repository.clone()),
            line_length: 120,
            filter: vec!["+runtime/printf".to_string()],
            recursive: true,
            exclude: vec!["third_party/**".to_string()],
            extensions: Some("cc,cpp".to_string()),
            headers: Some("hpp,hxx".to_string()),
            includeorder: CliIncludeOrder::Standardcfirst,
            config: "CPPLINT.custom".to_string(),
            timing: true,
            threads: Some(0),
            fix: true,
            files: vec![PathBuf::from("sample.cc")],
        };

        let config = args.to_runner_config().unwrap();

        assert_eq!(config.output_format, OutputFormat::Vs7);
        assert_eq!(config.counting_style, CountingStyle::Detailed);
        assert_eq!(config.verbose_level, 2);
        assert!(config.quiet);
        assert_eq!(
            config.num_threads,
            std::thread::available_parallelism().unwrap().get()
        );
        assert!(config.recursive);
        assert_eq!(config.excludes, vec!["third_party/**"]);
        assert!(config.fix);
        assert_eq!(config.options.line_length, 120);
        assert_eq!(config.options.config_filename, "CPPLINT.custom");
        assert_eq!(config.options.include_order, IncludeOrder::StandardCFirst);
        assert!(config.options.valid_extensions.contains("cc"));
        assert!(config.options.valid_extensions.contains("hpp"));
        assert!(config.options.header_extensions().contains("hxx"));
        assert!(
            config
                .options
                .filters
                .iter()
                .any(|filter| filter.category == "runtime/printf" && filter.sign)
        );
        assert_eq!(config.options.root, root);
        assert_eq!(config.options.repository, repository);

        std::fs::remove_dir_all(&config.options.root).unwrap();
        std::fs::remove_dir_all(&config.options.repository).unwrap();
    }
}
