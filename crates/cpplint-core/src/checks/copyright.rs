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

pub fn check(linter: &mut FileLinter, lines: &[String]) {
    // C++ version says it should occur by line 10.
    // lines[0] is often a placeholder or empty depending on how it's read.
    let search_limit = cmp::min(lines.len(), 11);

    for (i, line) in lines.iter().enumerate().take(search_limit) {
        if i == 0 && line.is_empty() {
            continue;
        } // Skip placeholder

        if COPYRIGHT_AC.is_match(line) {
            return;
        }
    }

    linter.error(
        0,
        "legal/copyright",
        5,
        "No copyright message found.  You should have a line: \"Copyright [year] <Copyright Owner>\"",
    );
}
