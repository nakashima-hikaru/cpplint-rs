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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_note_creation() {
        let text: Arc<str> = Arc::from("test message");
        let note = Note {
            file_index: 1,
            order: 42,
            stream: NoteStream::Stdout,
            text: Arc::clone(&text),
        };

        assert_eq!(note.file_index, 1);
        assert_eq!(note.order, 42);
        assert_eq!(note.stream, NoteStream::Stdout);
        assert_eq!(note.text, text);
    }

    #[test]
    fn test_note_equality() {
        let note1 = Note {
            file_index: 0,
            order: 10,
            stream: NoteStream::Stderr,
            text: Arc::from("error occurred"),
        };

        let note2 = Note {
            file_index: 0,
            order: 10,
            stream: NoteStream::Stderr,
            text: Arc::from("error occurred"),
        };

        let note3 = Note {
            file_index: 1,
            order: 10,
            stream: NoteStream::Stderr,
            text: Arc::from("error occurred"),
        };

        assert_eq!(note1, note2);
        assert_ne!(note1, note3);
    }

    #[test]
    fn test_note_clone() {
        let note1 = Note {
            file_index: 2,
            order: 5,
            stream: NoteStream::Stdout,
            text: Arc::from("info"),
        };

        let note2 = note1.clone();

        assert_eq!(note1, note2);
    }

    #[test]
    fn test_note_debug() {
        let note = Note {
            file_index: 3,
            order: 7,
            stream: NoteStream::Stderr,
            text: Arc::from("debug info"),
        };

        let debug_str = format!("{:?}", note);
        assert!(debug_str.contains("Note"));
        assert!(debug_str.contains("file_index: 3"));
        assert!(debug_str.contains("order: 7"));
        assert!(debug_str.contains("stream: Stderr"));
        assert!(debug_str.contains("text: \"debug info\""));
    }
}
