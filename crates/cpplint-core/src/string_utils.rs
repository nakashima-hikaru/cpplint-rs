use std::collections::BTreeSet;

/// Split string by comma, strip white spaces, and remove duplicated items.
pub fn parse_comma_separated_list(s: &str) -> BTreeSet<String> {
    s.split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

/// Converts a set of strings to a formatted string.
pub fn set_to_str(set: &BTreeSet<String>, prefix: &str, delim: &str, suffix: &str) -> String {
    let mut result = prefix.to_string();
    for (i, item) in set.iter().enumerate() {
        if i > 0 {
            result.push_str(delim);
        }
        result.push_str(item);
    }
    result.push_str(suffix);
    result
}

/// Returns the last non-space character or '\0'.
pub fn get_last_non_space(s: &str) -> char {
    s.trim_end().chars().next_back().unwrap_or('\0')
}

/// Returns true if the string consists of only digits.
pub fn str_is_digit(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

pub fn is_word_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

pub fn is_word_match(s: &str, start: usize, end: usize) -> bool {
    let bytes = s.as_bytes();
    let before_ok = start == 0 || !is_word_char(bytes[start - 1]);
    let after_ok = end == bytes.len() || !is_word_char(bytes[end]);
    before_ok && after_ok
}

pub fn contains_word(s: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }

    let mut search_start = 0usize;
    while let Some(offset) = s[search_start..].find(word) {
        let start = search_start + offset;
        let end = start + word.len();
        if is_word_match(s, start, end) {
            return true;
        }
        search_start = start + 1;
    }
    false
}

pub fn trimmed_starts_with_word(s: &str, word: &str) -> bool {
    let trimmed = s.trim_start();
    let Some(rest) = trimmed.strip_prefix(word) else {
        return false;
    };
    rest.is_empty() || !is_word_char(rest.as_bytes()[0])
}

pub fn ends_with_word(s: &str, word: &str) -> bool {
    if !s.ends_with(word) {
        return false;
    }
    let start = s.len() - word.len();
    start == 0 || !is_word_char(s.as_bytes()[start - 1])
}

pub fn contains_word_start(s: &str, word: &str) -> bool {
    let mut search_start = 0;
    while let Some(offset) = s[search_start..].find(word) {
        let start = search_start + offset;
        if start == 0 || !is_word_char(s.as_bytes()[start - 1]) {
            return true;
        }
        search_start = start + 1;
    }
    false
}

pub fn ends_with_word_and_optional_spaces(s: &str, word: &str) -> bool {
    let trimmed = s.trim_end();
    ends_with_word(trimmed, word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_comma_separated_list() {
        let expected: BTreeSet<String> = vec!["a", "b", "see", "d"]
            .into_iter()
            .map(String::from)
            .collect();
        let actual = parse_comma_separated_list("a,b, see ,,d");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_set_to_str_custom() {
        let set: BTreeSet<String> = vec!["a", "bar", "foo"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(
            set_to_str(&set, "prefix(", " | ", ").end()"),
            "prefix(a | bar | foo).end()"
        );
    }

    #[test]
    fn test_get_last_non_space() {
        assert_eq!(get_last_non_space("a \t\r\n\x0B\x0C"), 'a');
        assert_eq!(get_last_non_space("\t\r\n\x0B\x0Ca"), 'a');
        assert_eq!(get_last_non_space(""), '\0');
    }

    #[test]
    fn test_contains_word() {
        assert!(contains_word("if (x)", "if"));
        assert!(contains_word("value == final;", "final"));
        assert!(!contains_word("ifdef FOO", "if"));
        assert!(!contains_word("virtualize", "virtual"));
    }

    #[test]
    fn test_trimmed_starts_with_word() {
        assert!(trimmed_starts_with_word("  else {", "else"));
        assert!(!trimmed_starts_with_word("  elsewhere", "else"));
    }

    #[test]
    fn test_ends_with_word() {
        assert!(ends_with_word("decltype", "decltype"));
        assert!(ends_with_word("  decltype", "decltype"));
        assert!(!ends_with_word("my_decltype", "decltype"));
        assert!(!ends_with_word("decltype ", "decltype"));
    }

    #[test]
    fn test_contains_word_start() {
        assert!(contains_word_start("requires (x)", "requires"));
        assert!(contains_word_start("requires_x", "requires"));
        assert!(!contains_word_start("my_requires", "requires"));
    }

    #[test]
    fn test_ends_with_word_and_optional_spaces() {
        assert!(ends_with_word_and_optional_spaces("decltype", "decltype"));
        assert!(ends_with_word_and_optional_spaces("decltype  ", "decltype"));
        assert!(!ends_with_word_and_optional_spaces("my_decltype", "decltype"));
    }
}
