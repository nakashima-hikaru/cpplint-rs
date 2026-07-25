use crate::errors::Result;
use encoding_rs_io::DecodeReaderBytesBuilder;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawLineScan {
    pub invalid_utf8_lines: Vec<usize>,
    pub null_lines: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadFileResult {
    pub lines: Vec<String>,
    pub crlf_lines: Vec<usize>,
    pub lf_lines_count: usize,
    pub invalid_utf8_lines: Vec<usize>,
    pub null_lines: Vec<usize>,
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
        && let Ok(mmap) = unsafe { memmap2::MmapOptions::new().map(&file) }
    {
        return Ok(FileBytes::Mmap(mmap));
    }

    let mut bytes = Vec::with_capacity(len as usize);
    file.take(len).read_to_end(&mut bytes)?;
    Ok(FileBytes::Heap(bytes))
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

pub(crate) fn decode_bytes(bytes: &[u8]) -> Result<String> {
    let mut decoded_bytes = Vec::new();
    DecodeReaderBytesBuilder::new()
        .bom_sniffing(true)
        .build(Cursor::new(bytes))
        .read_to_end(&mut decoded_bytes)?;
    Ok(String::from_utf8_lossy(&decoded_bytes).into_owned())
}

pub fn read_lines(path: &Path) -> Result<ReadFileResult> {
    let bytes = read_raw_bytes(path)?;
    let RawLineScan {
        invalid_utf8_lines,
        null_lines,
    } = scan_raw_lines(&bytes);
    let decoded = decode_bytes(&bytes)?;

    let mut lines = Vec::new();
    let mut crlf_lines = Vec::new();
    let mut lf_lines_count = 0usize;

    for (linenum, raw_line) in decoded.split('\n').enumerate() {
        let mut line = raw_line.to_string();
        if line.ends_with('\r') {
            line.pop();
            crlf_lines.push(linenum);
        } else {
            lf_lines_count += 1;
        }

        lines.push(line);
    }

    if lines.is_empty() {
        lines.push(String::new());
        lf_lines_count = 1;
    }

    Ok(ReadFileResult {
        lines,
        crlf_lines,
        lf_lines_count,
        invalid_utf8_lines,
        null_lines,
    })
}
