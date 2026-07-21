use crate::errors::Result;
use crate::file_reader::{self, RawLineScan, ReadFileResult};
use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use std::iter::ExactSizeIterator;
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

    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[inline]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn read_into<'a>(&self, arena: &'a Bump) -> Result<DecodedSource<'a>> {
        let bytes = file_reader::read_raw_bytes(&self.path)?;
        let RawLineScan {
            invalid_utf8_lines,
            null_lines,
        } = file_reader::scan_raw_lines(&bytes);
        let decoded = file_reader::decode_bytes(&bytes)?;
        Ok(DecodedSource::from_decoded_text(
            arena,
            self.clone(),
            decoded,
            invalid_utf8_lines,
            null_lines,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSource<'a> {
    source_file: SourceFile,
    lines: BumpVec<'a, &'a str>,
    crlf_lines: BumpVec<'a, usize>,
    lf_lines_count: usize,
    invalid_utf8_lines: BumpVec<'a, usize>,
    null_lines: BumpVec<'a, usize>,
}

impl<'a> DecodedSource<'a> {
    fn from_decoded_text(
        arena: &'a Bump,
        source_file: SourceFile,
        decoded: String,
        invalid_utf8_lines_in: Vec<usize>,
        null_lines_in: Vec<usize>,
    ) -> Self {
        let allocated_decoded = arena.alloc_str(&decoded);
        let est_lines = allocated_decoded.bytes().filter(|&b| b == b'\n').count() + 1;
        let mut lines = BumpVec::with_capacity_in(est_lines, arena);
        let mut crlf_lines = BumpVec::new_in(arena);
        let mut lf_lines_count = 0usize;

        for (linenum, raw_line) in allocated_decoded.split('\n').enumerate() {
            let line = if let Some(line) = raw_line.strip_suffix('\r') {
                crlf_lines.push(linenum);
                line
            } else {
                lf_lines_count += 1;
                raw_line
            };
            lines.push(line);
        }

        if lines.is_empty() {
            lines.push("");
            lf_lines_count = 1;
        }

        let mut invalid_utf8_lines = BumpVec::with_capacity_in(invalid_utf8_lines_in.len(), arena);
        invalid_utf8_lines.extend_from_slice(&invalid_utf8_lines_in);

        let mut null_lines = BumpVec::with_capacity_in(null_lines_in.len(), arena);
        null_lines.extend_from_slice(&null_lines_in);

        Self {
            source_file,
            lines,
            crlf_lines,
            lf_lines_count,
            invalid_utf8_lines,
            null_lines,
        }
    }

    pub fn from_read_result(
        arena: &'a Bump,
        source_file: SourceFile,
        read_result: ReadFileResult,
    ) -> Self {
        let mut lines = BumpVec::with_capacity_in(read_result.lines.len(), arena);
        for line in read_result.lines {
            lines.push(arena.alloc_str(&line) as &str);
        }

        let mut crlf_lines = BumpVec::with_capacity_in(read_result.crlf_lines.len(), arena);
        crlf_lines.extend_from_slice(&read_result.crlf_lines);

        let mut invalid_utf8_lines =
            BumpVec::with_capacity_in(read_result.invalid_utf8_lines.len(), arena);
        invalid_utf8_lines.extend_from_slice(&read_result.invalid_utf8_lines);

        let mut null_lines = BumpVec::with_capacity_in(read_result.null_lines.len(), arena);
        null_lines.extend_from_slice(&read_result.null_lines);

        Self {
            source_file,
            lines,
            crlf_lines,
            lf_lines_count: read_result.lf_lines_count,
            invalid_utf8_lines,
            null_lines,
        }
    }

    pub fn from_lines<I, S>(arena: &'a Bump, source_file: SourceFile, lines_in: I) -> Self
    where
        I: IntoIterator<Item = S>,
        I::IntoIter: ExactSizeIterator,
        S: AsRef<str>,
    {
        let input = lines_in.into_iter();
        let mut lines = BumpVec::with_capacity_in(input.len().max(1), arena);
        let mut null_lines = BumpVec::new_in(arena);

        for (linenum, line) in input.enumerate() {
            let line = line.as_ref();
            if line.contains('\0') {
                null_lines.push(linenum);
            }
            lines.push(arena.alloc_str(line) as &str);
        }

        if lines.is_empty() {
            lines.push("");
        }
        let lf_lines_count = lines.len();

        Self {
            source_file,
            lines,
            crlf_lines: BumpVec::new_in(arena),
            lf_lines_count,
            invalid_utf8_lines: BumpVec::new_in(arena),
            null_lines,
        }
    }

    #[inline]
    pub fn source_file(&self) -> &SourceFile {
        &self.source_file
    }

    #[inline]
    pub fn invalid_utf8_lines(&self) -> &[usize] {
        &self.invalid_utf8_lines
    }

    #[inline]
    pub fn null_lines(&self) -> &[usize] {
        &self.null_lines
    }

    #[inline]
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

    #[inline]
    pub fn lines(&self) -> &[&'a str] {
        &self.lines
    }

    #[inline]
    pub fn into_lines(self) -> BumpVec<'a, &'a str> {
        self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_lines_scans_virtual_source_input() {
        let arena = Bump::new();
        let source = DecodedSource::from_lines(
            &arena,
            SourceFile::new(PathBuf::from("sample.cc")),
            vec!["\0".to_string(), "\u{FFFD}".to_string()],
        );

        assert_eq!(source.source_file().display_name(), "sample.cc");
        assert_eq!(source.null_lines(), &[0]);
        assert!(source.invalid_utf8_lines().is_empty());
        assert!(!source.has_mixed_line_endings());
    }

    #[test]
    fn from_read_result_preserves_metadata_and_lines() {
        let arena = Bump::new();
        let source = DecodedSource::from_read_result(
            &arena,
            SourceFile::new(PathBuf::from("sample.cc")),
            ReadFileResult {
                lines: vec!["alpha".to_string(), "beta".to_string()],
                crlf_lines: vec![0],
                lf_lines_count: 2,
                invalid_utf8_lines: vec![2],
                null_lines: vec![1],
            },
        );

        assert_eq!(source.source_file().display_name(), "sample.cc");
        assert_eq!(source.lines(), &["alpha", "beta"]);
        assert_eq!(source.crlf_lines(), &[0]);
        assert_eq!(source.invalid_utf8_lines(), &[2]);
        assert_eq!(source.null_lines(), &[1]);
        assert!(source.has_mixed_line_endings());

        let lines = source.into_lines();
        assert_eq!(lines.len(), 2);
    }
}
