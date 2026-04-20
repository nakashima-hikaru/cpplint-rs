use crate::categories::Category;
use crate::file_linter::FileLinter;
use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use std::cmp;
use std::sync::LazyLock;

static COPYRIGHT_AC: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasickBuilder::new()
        .ascii_case_insensitive(true)
        .build(["Copyright"])
        .unwrap()
});

pub fn check<S: AsRef<str>>(linter: &mut FileLinter, lines: &[S]) {
    // C++ version says it should occur by line 10.
    // lines[0] is often a placeholder or empty depending on how it's read.
    let search_limit = cmp::min(lines.len(), 11);

    for (i, line) in lines.iter().enumerate().take(search_limit) {
        let line = line.as_ref();
        if i == 0 && line.is_empty() {
            continue;
        } // Skip placeholder

        if COPYRIGHT_AC.is_match(line) {
            return;
        }
    }

    linter.error(
        0,
        Category::LegalCopyright,
        5,
        crate::messages::LintMessage::NoCopyrightFound,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::Options;
    use crate::state::CppLintState;
    use std::path::PathBuf;

    #[test]
    fn check_requires_a_copyright_notice_in_the_first_10_lines() {
        let state = CppLintState::new();
        let mut linter = FileLinter::new(PathBuf::from("test.cc"), &state, Options::new());

        check(&mut linter, &["", "", "", "", "", "", "", "", "", "", ""]);
        assert_eq!(state.error_count(), 1);
        assert!(state.has_error(Category::LegalCopyright));

        let state = CppLintState::new();
        let mut linter = FileLinter::new(PathBuf::from("test.cc"), &state, Options::new());
        check(
            &mut linter,
            &["", "", "", "", "", "", "", "", "", "// Copyright 2026", ""],
        );
        assert_eq!(state.error_count(), 0);
    }
}
