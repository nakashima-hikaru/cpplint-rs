use crate::errors::Result;
use crate::file_reader::{self, ReadFileResult};
use std::path::{Path, PathBuf};

/// A source file handle that decouples lint orchestration from file-system access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    path: PathBuf,
    display_name: String,
}

impl SourceFile {
    pub fn new(path: PathBuf) -> Self {
        let display_name = path.to_string_lossy().to_string();
        Self { path, display_name }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn read(&self) -> Result<DecodedSource> {
        let read_result = file_reader::read_lines(&self.path)?;
        Ok(DecodedSource::from_read_result(self.clone(), read_result))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSource {
    source_file: SourceFile,
    lines: Vec<String>,
    crlf_lines: Vec<usize>,
    lf_lines_count: usize,
    invalid_utf8_lines: Vec<usize>,
    null_lines: Vec<usize>,
}

impl DecodedSource {
    pub fn from_read_result(source_file: SourceFile, read_result: ReadFileResult) -> Self {
        Self {
            source_file,
            lines: read_result.lines,
            crlf_lines: read_result.crlf_lines,
            lf_lines_count: read_result.lf_lines_count,
            invalid_utf8_lines: read_result.invalid_utf8_lines,
            null_lines: read_result.null_lines,
        }
    }

    pub fn from_lines(source_file: SourceFile, mut lines: Vec<String>) -> Self {
        if lines.is_empty() {
            lines.push(String::new());
        }

        let null_lines = lines
            .iter()
            .enumerate()
            .filter_map(|(linenum, line)| line.contains('\0').then_some(linenum))
            .collect();
        let lf_lines_count = lines.len();

        Self {
            source_file,
            lines,
            crlf_lines: Vec::new(),
            lf_lines_count,
            invalid_utf8_lines: Vec::new(),
            null_lines,
        }
    }

    pub fn source_file(&self) -> &SourceFile {
        &self.source_file
    }

    pub fn invalid_utf8_lines(&self) -> &[usize] {
        &self.invalid_utf8_lines
    }

    pub fn null_lines(&self) -> &[usize] {
        &self.null_lines
    }

    pub fn crlf_lines(&self) -> &[usize] {
        &self.crlf_lines
    }

    pub fn has_mixed_line_endings(&self) -> bool {
        let lf_count = if !self.lines.is_empty()
            && self.lines.last().is_some_and(|s| s.is_empty())
            && self.lf_lines_count > 0
        {
            self.lf_lines_count - 1
        } else {
            self.lf_lines_count
        };
        lf_count > 0 && !self.crlf_lines.is_empty()
    }

    pub fn into_lines(self) -> Vec<String> {
        self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_lines_scans_virtual_source_input() {
        let source = DecodedSource::from_lines(
            SourceFile::new(PathBuf::from("sample.cc")),
            vec!["\0".to_string(), "\u{FFFD}".to_string()],
        );

        assert_eq!(source.source_file().display_name(), "sample.cc");
        assert_eq!(source.null_lines(), &[0]);
        assert!(source.invalid_utf8_lines().is_empty());
        assert!(!source.has_mixed_line_endings());
    }
}
