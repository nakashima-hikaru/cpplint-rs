use crate::string_utils::parse_comma_separated_list;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const DEFAULT_LINE_LENGTH: usize = 80;

const DEFAULT_SOURCE_EXTENSIONS: &[&str] = &["c", "cc", "cpp", "cxx", "c++", "cu"];
const DEFAULT_HEADER_EXTENSIONS: &[&str] = &["h", "hh", "hpp", "hxx", "h++", "cuh"];
const DEFAULT_FILTERS: &[&str] = &["-build/include_alpha", "-readability/fn_size"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub sign: bool,
    pub category: String,
    pub file: Option<String>,
    pub linenum: Option<usize>,
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
        let linenum = parts.next().and_then(|value| value.parse::<usize>().ok());

        Some(Self {
            sign,
            category,
            file,
            linenum,
        })
    }

    pub fn is_matched(&self, category: &str, file: &str, linenum: usize) -> bool {
        if !category.starts_with(&self.category) {
            return false;
        }
        if let Some(expected_file) = &self.file && expected_file != file {
            return false;
        }
        if let Some(expected_line) = self.linenum && expected_line != linenum {
            return false;
        }
        true
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
    pub line_length: usize,
    pub config_filename: String,
    pub valid_extensions: BTreeSet<String>,
    pub hpp_headers: BTreeSet<String>,
    pub include_order: IncludeOrder,
    pub filters: Vec<Filter>,
    pub timing: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
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
        }
    }
}

impl Options {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn all_extensions(&self) -> BTreeSet<String> {
        self.valid_extensions
            .union(&self.hpp_headers)
            .cloned()
            .collect()
    }

    pub fn header_extensions(&self) -> BTreeSet<String> {
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

    pub fn should_print_error(&self, category: &str, filename: &str, linenum: usize) -> bool {
        let mut result = true;
        for filter in &self.filters {
            if filter.is_matched(category, filename, linenum) {
                result = filter.sign;
            }
        }
        result
    }

    pub fn add_filter(&mut self, filter_str: &str) {
        self.filters.push(Filter::new(filter_str));
    }

    pub fn add_filters(&mut self, filters: &str) -> bool {
        let Some(parsed) = parse_filters(filters) else {
            return false;
        };
        self.filters.extend(parsed);
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
        assert_eq!(filter.linenum, Some(10));
        assert!(filter.is_matched("build/include_alpha", "test.cpp", 10));
        assert!(!filter.is_matched("build/include_alpha", "test.cpp", 11));
    }

    #[test]
    fn test_should_print_error() {
        let mut options = Options::new();
        assert!(!options.should_print_error("readability/fn_size", "test.cpp", 10));

        options.add_filter("+readability/fn_size");
        assert!(options.should_print_error("readability/fn_size", "test.cpp", 10));

        options.add_filter("-readability");
        assert!(!options.should_print_error("readability/fn_size", "test.cpp", 10));
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
        assert!(!options.should_print_error("whitespace/tab", "foo.cc", 1));
        assert!(options.should_print_error("runtime/printf", "test.cc", 14));
    }
}
