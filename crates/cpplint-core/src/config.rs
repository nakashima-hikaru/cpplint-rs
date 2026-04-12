use crate::options::{parse_filters, Filter, IncludeOrder, Options};
use crate::string_utils::parse_comma_separated_list;
use fxhash::FxHashMap;
use parking_lot::RwLock;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigMessageKind {
    Info,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigMessage {
    pub kind: ConfigMessageKind,
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum ConfigResolution {
    Lint {
        options: Arc<Options>,
        messages: Arc<[ConfigMessage]>,
    },
    Excluded {
        messages: Arc<[ConfigMessage]>,
    },
}

#[derive(Debug)]
struct ConfigFile {
    noparent: bool,
    filters: Vec<Filter>,
    exclude_files: Vec<ExcludePattern>,
    line_length: Option<usize>,
    root: Option<PathBuf>,
    extensions: Option<std::collections::BTreeSet<String>>,
    headers: Option<std::collections::BTreeSet<String>>,
    include_order: Option<IncludeOrder>,
    messages: Arc<[ConfigMessage]>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            noparent: false,
            filters: Vec::new(),
            exclude_files: Vec::new(),
            line_length: None,
            root: None,
            extensions: None,
            headers: None,
            include_order: None,
            messages: Vec::new().into(),
        }
    }
}

#[derive(Debug, Clone)]
struct ExcludePattern {
    raw: String,
    regex: Option<Regex>,
}

#[derive(Debug, Clone)]
struct PreparedExcludePattern {
    cfg_path: PathBuf,
    raw: String,
    regex: Regex,
}

#[derive(Debug, Clone)]
struct PreparedExcludeMatch {
    cfg_path: PathBuf,
    raw: String,
    component: String,
}

#[derive(Debug, Clone, Default)]
struct PreparedLocalPlan {
    messages: Arc<[ConfigMessage]>,
    excludes: Arc<[PreparedExcludePattern]>,
}

#[derive(Debug, Clone)]
enum PreparedDirectoryOutcome {
    Lint {
        options: Arc<Options>,
        messages: Arc<[ConfigMessage]>,
    },
    Excluded {
        messages_prefix: Arc<[ConfigMessage]>,
        exclude: PreparedExcludeMatch,
    },
}

#[derive(Debug, Clone)]
struct PreparedDirectoryPlan {
    local: PreparedLocalPlan,
    outcome: PreparedDirectoryOutcome,
}

#[derive(Debug)]
pub(crate) struct DirectoryConfigCache {
    base_options: Arc<Options>,
    config_filename: String,
    plans: RwLock<FxHashMap<PathBuf, Arc<PreparedDirectoryPlan>>>,
}

static CONFIG_FILE_CACHE: LazyLock<RwLock<FxHashMap<PathBuf, Arc<ConfigFile>>>> =
    LazyLock::new(|| RwLock::new(FxHashMap::default()));

impl DirectoryConfigCache {
    pub(crate) fn new(base_options: &Options) -> Self {
        Self {
            base_options: Arc::new(base_options.clone()),
            config_filename: base_options.config_filename.clone(),
            plans: RwLock::new(FxHashMap::default()),
        }
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(crate) fn resolve_for_file(&self, filename: &Path, quiet: bool) -> ConfigResolution {
        if filename == Path::new("-") {
            return ConfigResolution::Lint {
                options: Arc::clone(&self.base_options),
                messages: Vec::new().into(),
            };
        }

        let directory = filename.parent().unwrap_or_else(|| Path::new(""));
        let cached_plan = { self.plans.read().get(directory).cloned() };
        let plan = if let Some(plan) = cached_plan {
            plan
        } else {
            let built = Arc::new(build_directory_plan(
                self.base_options.as_ref(),
                &self.config_filename,
                directory,
            ));
            let mut plans = self.plans.write();
            Arc::clone(
                plans
                    .entry(directory.to_path_buf())
                    .or_insert_with(|| Arc::clone(&built)),
            )
        };
        plan.resolve_for_file(filename, quiet)
    }
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn resolve_for_file(base_options: &Options, filename: &Path, quiet: bool) -> ConfigResolution {
    let cache = DirectoryConfigCache::new(base_options);
    cache.resolve_for_file(filename, quiet)
}

impl PreparedDirectoryPlan {
    fn resolve_for_file(&self, filename: &Path, quiet: bool) -> ConfigResolution {
        if let Some(component) = filename.file_name().and_then(|value| value.to_str()) {
            for pattern in self.local.excludes.iter() {
                if pattern.regex.is_match(component) {
                    return ConfigResolution::Excluded {
                        messages: build_excluded_messages(
                            &self.local.messages,
                            filename,
                            &pattern.cfg_path,
                            component,
                            &pattern.raw,
                            quiet,
                        ),
                    };
                }
            }
        }

        match &self.outcome {
            PreparedDirectoryOutcome::Lint { options, messages } => ConfigResolution::Lint {
                options: Arc::clone(options),
                messages: Arc::clone(messages),
            },
            PreparedDirectoryOutcome::Excluded {
                messages_prefix,
                exclude,
            } => ConfigResolution::Excluded {
                messages: build_excluded_messages(
                    messages_prefix,
                    filename,
                    &exclude.cfg_path,
                    &exclude.component,
                    &exclude.raw,
                    quiet,
                ),
            },
        }
    }
}

fn build_directory_plan(
    base_options: &Options,
    config_filename: &str,
    directory: &Path,
) -> PreparedDirectoryPlan {
    let mut options = base_options.clone();
    let mut local_messages = Vec::new();
    let mut local_excludes = Vec::new();
    let local_cfg_path = directory.join(config_filename);
    if local_cfg_path.is_file() {
        let config = read_config_file(&local_cfg_path);
        apply_config_layer(
            &mut options,
            &mut local_messages,
            &local_cfg_path,
            config.as_ref(),
        );
        local_excludes = prepare_local_excludes(&local_cfg_path, config.as_ref());
        let local = PreparedLocalPlan {
            messages: local_messages.clone().into(),
            excludes: local_excludes.clone().into(),
        };
        if config.noparent {
            return PreparedDirectoryPlan {
                local,
                outcome: PreparedDirectoryOutcome::Lint {
                    options: Arc::new(options),
                    messages: local_messages.into(),
                },
            };
        }
    }

    let local = PreparedLocalPlan {
        messages: local_messages.clone().into(),
        excludes: local_excludes.into(),
    };
    let mut messages = local_messages;
    let mut path = directory.to_path_buf();
    loop {
        let Some(parent) = path.parent() else {
            break;
        };
        if parent == path {
            break;
        }

        let cfg_path = parent.join(config_filename);
        if cfg_path.is_file() {
            let config = read_config_file(&cfg_path);
            let component = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_string();
            apply_config_layer(&mut options, &mut messages, &cfg_path, config.as_ref());
            if matches_exclude(config.as_ref(), &component) {
                return PreparedDirectoryPlan {
                    local,
                    outcome: PreparedDirectoryOutcome::Excluded {
                        messages_prefix: messages.into(),
                        exclude: PreparedExcludeMatch {
                            cfg_path,
                            raw: first_matching_exclude_raw(config.as_ref(), &component)
                                .unwrap_or_default(),
                            component,
                        },
                    },
                };
            }
            if config.noparent {
                break;
            }
        }

        path = parent.to_path_buf();
    }

    PreparedDirectoryPlan {
        local,
        outcome: PreparedDirectoryOutcome::Lint {
            options: Arc::new(options),
            messages: messages.into(),
        },
    }
}

fn build_excluded_messages(
    prefix: &[ConfigMessage],
    filename: &Path,
    cfg_path: &Path,
    component: &str,
    raw: &str,
    quiet: bool,
) -> Arc<[ConfigMessage]> {
    let mut messages = prefix.to_vec();
    if !quiet {
        messages.push(ConfigMessage {
            kind: ConfigMessageKind::Info,
            text: format!(
                "Ignoring \"{}\": file excluded by \"{}\". File path component {} matches pattern {}\n",
                filename.display(),
                cfg_path.display(),
                component,
                raw
            ),
        });
    }
    messages.into()
}

fn apply_config_layer(
    options: &mut Options,
    messages: &mut Vec<ConfigMessage>,
    cfg_path: &Path,
    config: &ConfigFile,
) {
    messages.extend(config.messages.iter().cloned());
    options.filters.extend(config.filters.clone());

    if let Some(line_length) = config.line_length {
        options.line_length = line_length;
    }
    if let Some(root_override) = &config.root {
        options.root = cfg_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(root_override);
    }
    if let Some(extensions) = &config.extensions {
        options.valid_extensions = extensions.clone();
    }
    if let Some(headers) = &config.headers {
        options.hpp_headers = headers.clone();
        for header in headers {
            options.valid_extensions.insert(header.clone());
        }
    }
    if let Some(include_order) = config.include_order {
        options.include_order = include_order;
    }
}

fn prepare_local_excludes(cfg_path: &Path, config: &ConfigFile) -> Vec<PreparedExcludePattern> {
    config
        .exclude_files
        .iter()
        .filter_map(|pattern| {
            pattern.regex.as_ref().map(|regex| PreparedExcludePattern {
                cfg_path: cfg_path.to_path_buf(),
                raw: pattern.raw.clone(),
                regex: regex.clone(),
            })
        })
        .collect()
}

fn matches_exclude(config: &ConfigFile, component: &str) -> bool {
    config.exclude_files.iter().any(|pattern| {
        pattern
            .regex
            .as_ref()
            .is_some_and(|regex| regex.is_match(component))
    })
}

fn first_matching_exclude_raw(config: &ConfigFile, component: &str) -> Option<String> {
    config.exclude_files.iter().find_map(|pattern| {
        pattern
            .regex
            .as_ref()
            .is_some_and(|regex| regex.is_match(component))
            .then(|| pattern.raw.clone())
    })
}

fn read_config_file(path: &Path) -> Arc<ConfigFile> {
    if let Some(config) = CONFIG_FILE_CACHE.read().get(path).cloned() {
        return config;
    }

    let Ok(contents) = std::fs::read_to_string(path) else {
        let config = ConfigFile {
            messages: vec![ConfigMessage {
                kind: ConfigMessageKind::Error,
                text: format!(
                    "Skipping config file '{}': Can't open for reading\n",
                    path.display()
                ),
            }]
            .into(),
            ..Default::default()
        };
        let config = Arc::new(config);
        CONFIG_FILE_CACHE
            .write()
            .insert(path.to_path_buf(), Arc::clone(&config));
        return config;
    };

    let mut config = ConfigFile::default();
    let mut messages = Vec::new();
    for raw_line in contents.lines() {
        let line = raw_line
            .split_once('#')
            .map(|(prefix, _)| prefix)
            .unwrap_or(raw_line)
            .trim();
        if line.is_empty() {
            continue;
        }

        if line == "set noparent" {
            config.noparent = true;
            continue;
        }

        let Some((name, value)) = line.split_once('=') else {
            messages.push(ConfigMessage {
                kind: ConfigMessageKind::Error,
                text: format!(
                    "Invalid configuration option ({}) in file {}\n",
                    line,
                    path.display()
                ),
            });
            continue;
        };

        let name = name.trim();
        let value = value.trim();
        match name {
            "filter" => {
                if let Some(parsed) = parse_filters(value) {
                    config.filters.extend(parsed);
                } else {
                    messages.push(ConfigMessage {
                        kind: ConfigMessageKind::Error,
                        text: format!(
                            "{}: Every filter must start with + or - ({})\n",
                            path.display(),
                            value
                        ),
                    });
                }
            }
            "exclude_files" => {
                let regex = match Regex::new(value) {
                    Ok(regex) => Some(regex),
                    Err(error) => {
                        messages.push(ConfigMessage {
                            kind: ConfigMessageKind::Error,
                            text: format!(
                                "Invalid exclude_files regex ({}) in file {}: {}\n",
                                value,
                                path.display(),
                                error
                            ),
                        });
                        None
                    }
                };
                config.exclude_files.push(ExcludePattern {
                    raw: value.to_string(),
                    regex,
                });
            }
            "linelength" => match value.parse::<usize>() {
                Ok(line_length) => config.line_length = Some(line_length),
                Err(_) => messages.push(ConfigMessage {
                    kind: ConfigMessageKind::Error,
                    text: format!("Line length must be numeric in file ({})\n", path.display()),
                }),
            },
            "root" => config.root = Some(PathBuf::from(value)),
            "extensions" => config.extensions = Some(parse_comma_separated_list(value)),
            "headers" => config.headers = Some(parse_comma_separated_list(value)),
            "includeorder" => {
                config.include_order = match value {
                    "" | "default" => Some(IncludeOrder::Default),
                    "standardcfirst" => Some(IncludeOrder::StandardCFirst),
                    _ => {
                        messages.push(ConfigMessage {
                            kind: ConfigMessageKind::Error,
                            text: format!(
                                "Invalid includeorder value {} in file {}\n",
                                value,
                                path.display()
                            ),
                        });
                        None
                    }
                }
            }
            _ => messages.push(ConfigMessage {
                kind: ConfigMessageKind::Error,
                text: format!(
                    "Invalid configuration option ({}) in file {}\n",
                    name,
                    path.display()
                ),
            }),
        }
    }

    config.messages = messages.into();
    let config = Arc::new(config);
    CONFIG_FILE_CACHE
        .write()
        .insert(path.to_path_buf(), Arc::clone(&config));
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn assert_options_eq(actual: &Options, expected: &Options) {
        assert_eq!(actual.root, expected.root);
        assert_eq!(actual.repository, expected.repository);
        assert_eq!(actual.line_length, expected.line_length);
        assert_eq!(actual.config_filename, expected.config_filename);
        assert_eq!(actual.valid_extensions, expected.valid_extensions);
        assert_eq!(actual.hpp_headers, expected.hpp_headers);
        assert_eq!(actual.include_order, expected.include_order);
        assert_eq!(actual.filters, expected.filters);
        assert_eq!(actual.timing, expected.timing);
    }

    fn assert_resolution_eq(actual: ConfigResolution, expected: ConfigResolution) {
        match (actual, expected) {
            (
                ConfigResolution::Lint {
                    options: actual_options,
                    messages: actual_messages,
                },
                ConfigResolution::Lint {
                    options: expected_options,
                    messages: expected_messages,
                },
            ) => {
                assert_options_eq(actual_options.as_ref(), expected_options.as_ref());
                assert_eq!(actual_messages, expected_messages);
            }
            (
                ConfigResolution::Excluded {
                    messages: actual_messages,
                },
                ConfigResolution::Excluded {
                    messages: expected_messages,
                },
            ) => {
                assert_eq!(actual_messages, expected_messages);
            }
            (actual, expected) => panic!("resolution mismatch: {actual:?} != {expected:?}"),
        }
    }

    fn unique_temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("cpplint-rs-config-{}", unique))
    }

    #[test]
    fn nested_configs_can_exclude_files() {
        let root = unique_temp_dir();
        let child = root.join("child");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(root.join("CPPLINT.cfg"), "set noparent\n").unwrap();
        std::fs::write(child.join("CPPLINT.cfg"), "exclude_files=target.cc\n").unwrap();
        let file = child.join("target.cc");
        std::fs::write(&file, "int main() {}\n").unwrap();

        let resolution = resolve_for_file(&Options::new(), &file, false);
        assert!(matches!(resolution, ConfigResolution::Excluded { .. }));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn config_overrides_are_applied() {
        let root = unique_temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("CPPLINT.cfg"),
            "set noparent\nlinelength=120\nheaders=hpp,hxx\nincludeorder=standardcfirst\n",
        )
        .unwrap();
        let file = root.join("demo.cc");
        std::fs::write(&file, "int main() {}\n").unwrap();

        let resolution = resolve_for_file(&Options::new(), &file, false);
        let ConfigResolution::Lint { options, .. } = resolution else {
            panic!("file should not be excluded");
        };

        assert_eq!(options.line_length, 120);
        assert!(options.hpp_headers.contains("hpp"));
        assert_eq!(options.include_order, IncludeOrder::StandardCFirst);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn directory_cache_matches_direct_resolution_for_sibling_files() {
        let root = unique_temp_dir();
        let child = root.join("child");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(
            root.join("CPPLINT.cfg"),
            "set noparent\nfilter=-whitespace/line_length\nextensions=cc,h\n",
        )
        .unwrap();
        std::fs::write(
            child.join("CPPLINT.cfg"),
            "linelength=120\nexclude_files=skip\\.cc\n",
        )
        .unwrap();

        let keep = child.join("keep.cc");
        let skip = child.join("skip.cc");
        std::fs::write(&keep, "int main() {}\n").unwrap();
        std::fs::write(&skip, "int main() {}\n").unwrap();

        let base_options = Options::new();
        let cache = DirectoryConfigCache::new(&base_options);
        assert_resolution_eq(
            cache.resolve_for_file(&keep, false),
            resolve_for_file(&base_options, &keep, false),
        );
        assert_resolution_eq(
            cache.resolve_for_file(&skip, false),
            resolve_for_file(&base_options, &skip, false),
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}
