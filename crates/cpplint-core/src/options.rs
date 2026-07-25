use crate::string_utils::parse_comma_separated_list;
use rustc_hash::FxHashSet;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

pub const DEFAULT_LINE_LENGTH: NonZeroUsize = match NonZeroUsize::new(80) {
    Some(val) => val,
    None => unreachable!(),
};

const DEFAULT_SOURCE_EXTENSIONS: &[&str] = &["c", "cc", "cpp", "cxx", "c++", "cu"];
const DEFAULT_HEADER_EXTENSIONS: &[&str] = &["h", "hh", "hpp", "hxx", "h++", "cuh"];
const DEFAULT_FILTERS: &[&str] = &["-build/include_alpha", "-readability/fn_size"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub sign: bool,
    pub category: String,
    pub file: Option<String>,
    pub linenum: Option<NonZeroUsize>,
}

impl Filter {
    pub fn new(filter_str: &str) -> Self {
        Self::parse(filter_str).unwrap_or(Self {
            sign: false,
            category: String::new(),
            file: None,
            linenum: None,
        })
    }

    pub fn parse(filter_str: &str) -> Option<Self> {
        let mut chars = filter_str.chars();
        let sign = match chars.next()? {
            '+' => true,
            '-' => false,
            _ => return None,
        };

        let rest = &filter_str[1..];
        let mut parts = rest.splitn(3, ':');
        let category = parts.next().unwrap_or("").to_string();
        let file = parts
            .next()
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        let linenum = parts
            .next()
            .and_then(|value| value.parse::<usize>().ok())
            .and_then(NonZeroUsize::new);

        Some(Self {
            sign,
            category,
            file,
            linenum,
        })
    }

    pub fn is_matched(&self, category: &str, file: &str, linenum: usize) -> bool {
        self.matches_category_and_file(category, file)
            && self
                .linenum
                .is_none_or(|expected_line| expected_line.get() == linenum)
    }

    fn matches_category_and_file(&self, category: &str, file: &str) -> bool {
        category.starts_with(&self.category)
            && self
                .file
                .as_ref()
                .is_none_or(|expected_file| expected_file == file)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IncludeOrder {
    #[default]
    Default,
    StandardCFirst,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub root: PathBuf,
    pub repository: PathBuf,
    pub line_length: NonZeroUsize,
    pub config_filename: String,
    pub valid_extensions: FxHashSet<String>,
    pub hpp_headers: FxHashSet<String>,
    pub include_order: IncludeOrder,
    pub filters: Vec<Filter>,
    pub timing: bool,
    category_defaults: [bool; crate::categories::Category::COUNT],
    has_specific_filters: bool,
}

impl Default for Options {
    fn default() -> Self {
        let mut opts = Self {
            root: PathBuf::new(),
            repository: PathBuf::new(),
            line_length: DEFAULT_LINE_LENGTH,
            config_filename: "CPPLINT.cfg".to_string(),
            valid_extensions: DEFAULT_SOURCE_EXTENSIONS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            hpp_headers: DEFAULT_HEADER_EXTENSIONS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            include_order: IncludeOrder::Default,
            filters: DEFAULT_FILTERS
                .iter()
                .map(|value| Filter::new(value))
                .collect(),
            timing: false,
            category_defaults: [true; crate::categories::Category::COUNT],
            has_specific_filters: false,
        };
        opts.recompute_category_defaults();
        opts
    }
}

impl Options {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn recompute_category_defaults(&mut self) {
        self.has_specific_filters = self
            .filters
            .iter()
            .any(|f| f.file.is_some() || f.linenum.is_some());
        for &cat in crate::categories::Category::ALL {
            let mut result = true;
            for filter in &self.filters {
                if filter.file.is_none()
                    && filter.linenum.is_none()
                    && cat.as_str().starts_with(&filter.category)
                {
                    result = filter.sign;
                }
            }
            self.category_defaults[cat.index()] = result;
        }
    }

    pub fn all_extensions(&self) -> FxHashSet<String> {
        self.valid_extensions
            .union(&self.hpp_headers)
            .cloned()
            .collect()
    }

    pub fn header_extensions(&self) -> FxHashSet<String> {
        self.hpp_headers.clone()
    }

    pub fn is_valid_file(&self, path: &Path) -> bool {
        let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
            return false;
        };
        self.valid_extensions.contains(ext) || self.hpp_headers.contains(ext)
    }

    pub fn set_extensions_from_csv(&mut self, value: &str) {
        self.valid_extensions = parse_comma_separated_list(value);
    }

    pub fn set_headers_from_csv(&mut self, value: &str) {
        self.hpp_headers = parse_comma_separated_list(value);
        for header in self.hpp_headers.clone() {
            self.valid_extensions.insert(header);
        }
    }

    pub fn set_include_order_from_str(&mut self, value: &str) -> bool {
        self.include_order = match value {
            "" | "default" => IncludeOrder::Default,
            "standardcfirst" => IncludeOrder::StandardCFirst,
            _ => return false,
        };
        true
    }

    pub fn should_print_error(
        &self,
        category: crate::categories::Category,
        filename: &str,
        linenum: usize,
    ) -> bool {
        if !self.has_specific_filters {
            return self.category_defaults[category.index()];
        }
        let mut result = self.category_defaults[category.index()];
        for filter in &self.filters {
            if (filter.file.is_some() || filter.linenum.is_some())
                && filter.is_matched(category.as_str(), filename, linenum)
            {
                result = filter.sign;
            }
        }
        result
    }

    pub(crate) fn can_print_error_for_some_line(
        &self,
        category: crate::categories::Category,
        filename: &str,
    ) -> bool {
        let mut default_result = true;
        let mut line_results = Vec::<(usize, bool)>::new();

        for filter in &self.filters {
            if !filter.matches_category_and_file(category.as_str(), filename) {
                continue;
            }

            if let Some(linenum) = filter.linenum {
                if let Some((_, state)) = line_results
                    .iter_mut()
                    .find(|(candidate, _)| *candidate == linenum.get())
                {
                    *state = filter.sign;
                } else {
                    line_results.push((linenum.get(), filter.sign));
                }
                continue;
            }

            default_result = filter.sign;
            for (_, state) in &mut line_results {
                *state = filter.sign;
            }
        }

        default_result || line_results.into_iter().any(|(_, state)| state)
    }

    pub fn add_filter(&mut self, filter_str: &str) {
        self.filters.push(Filter::new(filter_str));
        self.recompute_category_defaults();
    }

    pub fn add_filters(&mut self, filters: &str) -> bool {
        let Some(parsed) = parse_filters(filters) else {
            return false;
        };
        self.filters.extend(parsed);
        self.recompute_category_defaults();
        true
    }
}

pub fn parse_filters(filters: &str) -> Option<Vec<Filter>> {
    let mut parsed = Vec::new();
    for item in filters.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        parsed.push(Filter::parse(item)?);
    }
    Some(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_matching() {
        let filter = Filter::new("-build/include:test.cpp:10");
        assert!(!filter.sign);
        assert_eq!(filter.category, "build/include");
        assert_eq!(filter.file.as_deref(), Some("test.cpp"));
        assert_eq!(filter.linenum, NonZeroUsize::new(10));
        assert!(filter.is_matched("build/include_alpha", "test.cpp", 10));
        assert!(!filter.is_matched("build/include_alpha", "test.cpp", 11));
    }

    #[test]
    fn test_should_print_error() {
        let mut options = Options::new();
        assert!(!options.should_print_error(
            crate::categories::Category::BuildIncludeAlpha,
            "test.cpp",
            10
        ));
        assert!(!options.should_print_error(
            crate::categories::Category::ReadabilityFnSize,
            "test.cpp",
            10
        ));

        options.add_filter("+build/include_alpha");
        assert!(options.should_print_error(
            crate::categories::Category::BuildIncludeAlpha,
            "test.cpp",
            10
        ));

        options.add_filter("+readability/fn_size");
        assert!(options.should_print_error(
            crate::categories::Category::ReadabilityFnSize,
            "test.cpp",
            10
        ));

        options.add_filter("-readability");
        assert!(!options.should_print_error(
            crate::categories::Category::ReadabilityFnSize,
            "test.cpp",
            10
        ));
    }

    #[test]
    fn test_extensions_and_headers_are_merged() {
        let mut options = Options::new();
        options.set_extensions_from_csv("cc,cpp");
        options.set_headers_from_csv("hpp,hxx");

        let all = options.all_extensions();
        assert!(all.contains("cc"));
        assert!(all.contains("hpp"));
        assert!(options.header_extensions().contains("hxx"));
    }

    #[test]
    fn test_add_filters_parses_list() {
        let mut options = Options::new();
        assert!(options.add_filters("-whitespace,+runtime/printf:test.cc:14"));
        assert!(!options.should_print_error(
            crate::categories::Category::WhitespaceTab,
            "foo.cc",
            1
        ));
        assert!(options.should_print_error(
            crate::categories::Category::RuntimePrintf,
            "test.cc",
            14
        ));
    }

    #[test]
    fn test_can_print_error_for_some_line_tracks_line_overrides() {
        let mut options = Options::new();
        assert!(options.add_filters("-,+runtime/printf:test.cc:14,-runtime/printf:test.cc:15"));

        assert!(
            options.can_print_error_for_some_line(
                crate::categories::Category::RuntimePrintf,
                "test.cc"
            )
        );
        assert!(
            !options.can_print_error_for_some_line(
                crate::categories::Category::RuntimePrintf,
                "other.cc"
            )
        );

        options.add_filter("-runtime/printf");
        assert!(
            !options.can_print_error_for_some_line(
                crate::categories::Category::RuntimePrintf,
                "test.cc"
            )
        );
    }
}
