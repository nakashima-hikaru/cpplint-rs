use fxhash::FxHashMap;
use regex::Regex;
use std::cell::RefCell;
use std::sync::Arc;

enum CachedRegex {
    Standard(Arc<Regex>),
    Invalid,
}

thread_local! {
    static REGEX_CACHE: RefCell<FxHashMap<String, CachedRegex>> =
        RefCell::new(FxHashMap::default());
}

fn get_cached_regex(pattern: &str) -> Option<Arc<Regex>> {
    REGEX_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.get(pattern) {
            return match cached {
                CachedRegex::Standard(re) => Some(Arc::clone(re)),
                CachedRegex::Invalid => None,
            };
        }

        let compiled = if let Ok(re) = Regex::new(pattern) {
            CachedRegex::Standard(Arc::new(re))
        } else {
            CachedRegex::Invalid
        };

        let entry = cache.entry(pattern.to_string()).or_insert(compiled);
        match entry {
            CachedRegex::Standard(re) => Some(Arc::clone(re)),
            CachedRegex::Invalid => None,
        }
    })
}

/// Checks if a pattern matches anywhere in the string.
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn regex_search(pattern: &str, s: &str) -> bool {
    if let Some(re) = get_cached_regex(pattern) {
        return re.is_match(s);
    }
    false
}

/// Matches a pattern against a substring defined by a range.
#[cfg(test)]
pub fn regex_match_with_range(pattern: &str, s: &str, start: usize, len: usize) -> bool {
    if start + len > s.len() {
        return false;
    }
    let sub = &s[start..start + len];
    // For Match() behavior, we should anchor at the start.
    // C++ RegexMatchWithRange behavior: matches the whole substring if start with ^ and end with $
    // We'll just match the substring.
    regex_search(pattern, sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_search_python_cases() {
        let cases = vec![
            ("Python|Perl|Tcl", "Perl", true),
            ("", "", true),
            ("abc", "abc", true),
            ("abc", "xbc", false),
            ("abc", "xabcy", true),
            ("ab*c", "abc", true),
            ("ab*bc", "abbbbc", true),
            ("ab+bc", "abc", false),
            ("^abc$", "abc", true),
            ("^abc$", "abcc", false),
            ("abc$", "aabc", true),
            ("a.c", "axc", true),
            ("a.*c", "axyzd", false),
            ("a[bc]d", "abd", true),
            ("a[b-d]e", "ace", true),
            ("a[-d]", "a-", true),
            ("a]", "a]", true),
            ("a[\\-d]", "a-", true),
            ("a[^bc]d", "aed", true),
            ("ab|cd", "abcd", true),
            ("$b", "b", false),
            ("\\w+", "--ab_cd0123---", true),
        ];

        for (pattern, s, expected) in cases {
            assert_eq!(
                regex_search(pattern, s),
                expected,
                "pattern: {}, s: {}",
                pattern,
                s
            );
        }
    }

    #[test]
    fn test_regex_match_with_range() {
        assert!(regex_match_with_range("^test$", "rangetest", 5, 4));
    }
}
