use std::borrow::Cow;
use std::cell::RefCell;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::ops::Range;
use std::path::Path;

use encoding_rs_io::DecodeReaderBytesBuilder;

use crate::errors::Result;

thread_local! {
    static READ_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(16384));
    static DECODE_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(16384));
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadFileResult {
    pub content: String,
    pub line_ranges: Vec<Range<usize>>,
    pub crlf_lines: Vec<usize>,
    pub lf_lines_count: usize,
    pub invalid_utf8_lines: Vec<usize>,
    pub null_lines: Vec<usize>,
    pub had_utf8_bom: bool,
}

impl ReadFileResult {
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.line_ranges
            .iter()
            .map(move |r| &self.content[r.clone()])
    }

    pub fn to_lines_vec(&self) -> Vec<String> {
        self.line_ranges
            .iter()
            .map(|r| self.content[r.clone()].to_string())
            .collect()
    }
}

pub(crate) fn read_raw_bytes_with_buffer<F, R>(path: &Path, f: F) -> Result<R>
where
    F: FnOnce(&[u8]) -> Result<R>,
{
    if path == Path::new("-") {
        let mut bytes = Vec::new();
        io::stdin().read_to_end(&mut bytes)?;
        return f(&bytes);
    }

    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let len = metadata.len();

    if len >= 65536
        && let Ok(mmap) = unsafe { memmap2::MmapOptions::new().map(&file) }
    {
        return f(&mmap);
    }

    READ_BUF.with(|buf_cell| {
        let mut bytes = buf_cell.borrow_mut();
        bytes.clear();
        let cap = bytes.capacity();
        if cap < len as usize {
            bytes.reserve(len as usize - cap);
        }
        file.take(len).read_to_end(&mut bytes)?;
        f(&bytes)
    })
}

pub(crate) struct RawScanResult<'a> {
    pub decoded: Cow<'a, str>,
    pub invalid_utf8_lines: Vec<usize>,
    pub null_lines: Vec<usize>,
}

pub(crate) fn scan_and_decode_bytes<'a>(bytes: &'a [u8]) -> Result<RawScanResult<'a>> {
    let has_bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
    let utf8_res = std::str::from_utf8(bytes);
    let all_valid_utf8 = utf8_res.is_ok();
    let has_null = bytes.contains(&b'\0');

    let mut invalid_utf8_lines = Vec::new();
    let mut null_lines = Vec::new();

    if !all_valid_utf8 || has_null {
        for (linenum, raw_line) in bytes.split(|&byte| byte == b'\n').enumerate() {
            let line_bytes = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
            if !all_valid_utf8 && std::str::from_utf8(line_bytes).is_err() {
                invalid_utf8_lines.push(linenum);
            }
            if has_null && line_bytes.contains(&b'\0') {
                null_lines.push(linenum);
            }
        }
    }

    let decoded = if !has_bom && let Ok(s) = utf8_res {
        Cow::Borrowed(s)
    } else {
        DECODE_BUF.with(|buf_cell| {
            let mut decoded_bytes = buf_cell.borrow_mut();
            decoded_bytes.clear();
            DecodeReaderBytesBuilder::new()
                .bom_sniffing(true)
                .build(Cursor::new(bytes))
                .read_to_end(&mut decoded_bytes)?;
            Ok::<_, crate::errors::CppLintError>(Cow::Owned(
                String::from_utf8_lossy(&decoded_bytes).into_owned(),
            ))
        })?
    };

    Ok(RawScanResult {
        decoded,
        invalid_utf8_lines,
        null_lines,
    })
}

pub fn read_lines(path: &Path) -> Result<ReadFileResult> {
    read_raw_bytes_with_buffer(path, |bytes| {
        let had_utf8_bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
        let RawScanResult {
            decoded,
            invalid_utf8_lines,
            null_lines,
        } = scan_and_decode_bytes(bytes)?;

        let est_lines = (bytes.len() / 30).max(1);
        let mut line_ranges = Vec::with_capacity(est_lines);
        let mut crlf_lines = Vec::new();
        let mut lf_lines_count = 0usize;

        let mut offset = 0;
        for (linenum, raw_line) in decoded.split('\n').enumerate() {
            let line_len = raw_line.len();
            let (end, has_crlf) = if let Some(stripped) = raw_line.strip_suffix('\r') {
                (offset + stripped.len(), true)
            } else {
                (offset + line_len, false)
            };

            if has_crlf {
                crlf_lines.push(linenum);
            } else {
                lf_lines_count += 1;
            }

            line_ranges.push(offset..end);
            offset += line_len + 1;
        }

        if line_ranges.is_empty() {
            line_ranges.push(0..0);
            lf_lines_count = 1;
        }

        Ok(ReadFileResult {
            content: decoded.into_owned(),
            line_ranges,
            crlf_lines,
            lf_lines_count,
            invalid_utf8_lines,
            null_lines,
            had_utf8_bom,
        })
    })
}

