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
pub(crate) struct RawLineScan {
    pub invalid_utf8_lines: Vec<usize>,
    pub null_lines: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadFileResult {
    pub content: String,
    pub line_ranges: Vec<Range<usize>>,
    pub crlf_lines: Vec<usize>,
    pub lf_lines_count: usize,
    pub invalid_utf8_lines: Vec<usize>,
    pub null_lines: Vec<usize>,
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

pub(crate) enum FileBytes {
    Mmap(memmap2::Mmap),
    Heap(Vec<u8>),
}

impl std::ops::Deref for FileBytes {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &[u8] {
        match self {
            FileBytes::Mmap(mmap) => mmap,
            FileBytes::Heap(vec) => vec,
        }
    }
}

pub(crate) fn read_raw_bytes(path: &Path) -> Result<FileBytes> {
    if path == Path::new("-") {
        let mut bytes = Vec::new();
        io::stdin().read_to_end(&mut bytes)?;
        return Ok(FileBytes::Heap(bytes));
    }

    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let len = metadata.len();

    if len >= 16384
        && let Ok(mmap) = unsafe { memmap2::MmapOptions::new().map(&file) } {
            return Ok(FileBytes::Mmap(mmap));
        }

    let mut bytes = Vec::with_capacity(len as usize);
    file.take(len).read_to_end(&mut bytes)?;
    Ok(FileBytes::Heap(bytes))
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

    if len >= 16384
        && let Ok(mmap) = unsafe { memmap2::MmapOptions::new().map(&file) } {
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

pub(crate) fn scan_raw_lines(bytes: &[u8]) -> RawLineScan {
    let mut invalid_utf8_lines = Vec::new();
    let mut null_lines = Vec::new();

    // ⚡ Bolt: Fast path for files that are fully valid UTF-8 and contain no null bytes.
    // The standard library `from_utf8` and slice `contains` are highly optimized and
    // process the entire buffer much faster than checking line-by-line.
    let all_valid_utf8 = std::str::from_utf8(bytes).is_ok();
    let has_null = bytes.contains(&b'\0');

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

    RawLineScan {
        invalid_utf8_lines,
        null_lines,
    }
}

use std::borrow::Cow;

pub(crate) fn decode_bytes<'a>(bytes: &'a [u8]) -> Result<Cow<'a, str>> {
    // Fast path: Pure UTF-8 without BOM
    if !bytes.starts_with(&[0xEF, 0xBB, 0xBF])
        && let Ok(s) = std::str::from_utf8(bytes) {
            return Ok(Cow::Borrowed(s));
        }

    DECODE_BUF.with(|buf_cell| {
        let mut decoded_bytes = buf_cell.borrow_mut();
        decoded_bytes.clear();
        DecodeReaderBytesBuilder::new()
            .bom_sniffing(true)
            .build(Cursor::new(bytes))
            .read_to_end(&mut decoded_bytes)?;
        Ok(Cow::Owned(
            String::from_utf8_lossy(&decoded_bytes).into_owned(),
        ))
    })
}

pub fn read_lines(path: &Path) -> Result<ReadFileResult> {
    read_raw_bytes_with_buffer(path, |bytes| {
        let RawLineScan {
            invalid_utf8_lines,
            null_lines,
        } = scan_raw_lines(bytes);
        let decoded = decode_bytes(bytes)?;

        let est_lines = decoded.bytes().filter(|&b| b == b'\n').count() + 1;
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
        })
    })
}
