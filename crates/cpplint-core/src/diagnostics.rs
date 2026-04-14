use crate::categories::Category;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file_index: usize,
    pub filename: Arc<str>,
    pub linenum: usize,
    pub category: Category,
    pub confidence: i32,
    pub message: Arc<str>,
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
    pub text: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedFile {
    pub file_index: usize,
    pub filename: Arc<str>,
    pub had_error: bool,
}
