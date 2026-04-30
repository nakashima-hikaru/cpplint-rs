use crate::categories::Category;
use crate::file_linter::FileLinter;
use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use std::sync::LazyLock;

static COPYRIGHT_AC: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasickBuilder::new()
        .ascii_case_insensitive(true)
        .build(["Copyright"])
        .unwrap()
});

pub fn check<S: AsRef<str>>(linter: &mut FileLinter, lines: &[S]) {
    for line in lines {
        let line = line.as_ref();
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
    fn check_requires_a_copyright_notice_somewhere_in_the_file() {
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

        let state = CppLintState::new();
        let mut linter = FileLinter::new(PathBuf::from("test.cc"), &state, Options::new());
        check(
            &mut linter,
            &[
                "// filler 1",
                "// filler 2",
                "// filler 3",
                "// filler 4",
                "// filler 5",
                "// filler 6",
                "// filler 7",
                "// filler 8",
                "// filler 9",
                "// filler 10",
                "// Copyright 2026",
                "",
            ],
        );
        assert_eq!(state.error_count(), 0);
    }
}
