use crate::errors::Result;
use encoding_rs_io::DecodeReaderBytesBuilder;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadFileResult {
    pub lines: Vec<String>,
    pub crlf_lines: Vec<usize>,
    pub lf_lines_count: usize,
    pub invalid_utf8_lines: Vec<usize>,
    pub null_lines: Vec<usize>,
}

pub fn read_lines(path: &Path) -> Result<ReadFileResult> {
    let mut bytes = Vec::new();
    if path == Path::new("-") {
        io::stdin().read_to_end(&mut bytes)?;
    } else {
        File::open(path)?.read_to_end(&mut bytes)?;
    }

    let mut invalid_utf8_lines = Vec::new();
    let mut null_lines = Vec::new();
    for (linenum, raw_line) in bytes.split(|&byte| byte == b'\n').enumerate() {
        let line_bytes = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        if std::str::from_utf8(line_bytes).is_err() {
            invalid_utf8_lines.push(linenum);
        }
        if line_bytes.contains(&b'\0') {
            null_lines.push(linenum);
        }
    }

    let mut decoded_bytes = Vec::new();
    DecodeReaderBytesBuilder::new()
        .bom_sniffing(true)
        .build(Cursor::new(bytes))
        .read_to_end(&mut decoded_bytes)?;
    let decoded = String::from_utf8_lossy(&decoded_bytes).into_owned();

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
