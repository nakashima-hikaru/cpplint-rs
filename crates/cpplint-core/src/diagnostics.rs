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
    fn test_diagnostic_properties() {
        let diag1 = Diagnostic {
            file_index: 1,
            filename: Arc::from("test.cpp"),
            linenum: 42,
            category: Category::BuildInclude,
            confidence: 5,
            message: Arc::from("Include error"),
        };

        let diag2 = diag1.clone();

        // Test PartialEq and Eq
        assert_eq!(diag1, diag2);

        // Test fields
        assert_eq!(diag1.file_index, 1);
        assert_eq!(&*diag1.filename, "test.cpp");
        assert_eq!(diag1.linenum, 42);
        assert_eq!(diag1.category, Category::BuildInclude);
        assert_eq!(diag1.confidence, 5);
        assert_eq!(&*diag1.message, "Include error");

        // Test Debug
        let debug_str = format!("{:?}", diag1);
        assert!(debug_str.contains("Diagnostic"));
        assert!(debug_str.contains("test.cpp"));
    }

    #[test]
    fn test_note_stream_properties() {
        let stream_out = NoteStream::Stdout;
        let stream_err = NoteStream::Stderr;

        // Test PartialEq and Eq
        assert_eq!(stream_out, NoteStream::Stdout);
        assert_ne!(stream_out, stream_err);

        // Test Copy and Clone
        let stream_out_copy = stream_out;
        assert_eq!(stream_out, stream_out_copy);

        let stream_out_clone = stream_out.clone();
        assert_eq!(stream_out, stream_out_clone);

        // Test Debug
        let debug_str = format!("{:?}", stream_out);
        assert_eq!(debug_str, "Stdout");
    }

    #[test]
    fn test_note_properties() {
        let note1 = Note {
            file_index: 2,
            order: 1,
            stream: NoteStream::Stderr,
            text: Arc::from("Warning message"),
        };

        let note2 = note1.clone();

        // Test PartialEq and Eq
        assert_eq!(note1, note2);

        // Test fields
        assert_eq!(note1.file_index, 2);
        assert_eq!(note1.order, 1);
        assert_eq!(note1.stream, NoteStream::Stderr);
        assert_eq!(&*note1.text, "Warning message");

        // Test Debug
        let debug_str = format!("{:?}", note1);
        assert!(debug_str.contains("Note"));
        assert!(debug_str.contains("Warning message"));
    }

    #[test]
    fn test_processed_file_properties() {
        let pf1 = ProcessedFile {
            file_index: 3,
            filename: Arc::from("main.cpp"),
            had_error: true,
        };

        let pf2 = pf1.clone();

        // Test PartialEq and Eq
        assert_eq!(pf1, pf2);

        // Test fields
        assert_eq!(pf1.file_index, 3);
        assert_eq!(&*pf1.filename, "main.cpp");
        assert!(pf1.had_error);

        // Test Debug
        let debug_str = format!("{:?}", pf1);
        assert!(debug_str.contains("ProcessedFile"));
        assert!(debug_str.contains("main.cpp"));
    }
}
