#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file_index: usize,
    pub filename: String,
    pub linenum: usize,
    pub category: String,
    pub confidence: i32,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub file_index: usize,
    pub order: usize,
    pub stream: NoteStream,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedFile {
    pub file_index: usize,
    pub filename: String,
    pub had_error: bool,
}
