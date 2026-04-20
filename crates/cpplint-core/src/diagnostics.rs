use crate::categories::Category;
use crate::messages::LintMessage;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(usize);

impl FileId {
    pub(crate) const fn from_index(index: usize) -> Self {
        Self(index)
    }

    pub(crate) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileTable {
    names: Vec<Arc<str>>,
}

impl FileTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, filename: &str) -> FileId {
        self.names.push(Arc::from(filename));
        FileId::from_index(self.names.len() - 1)
    }

    pub fn get(&self, file_id: FileId) -> &str {
        let index = file_id.index();
        self.names
            .get(index)
            .map(|entry| entry.as_ref())
            .unwrap_or_else(|| panic!("missing file name for file_id={index}"))
    }

    pub fn merge_from(&mut self, other: &Self) {
        for (index, name) in other.names.iter().enumerate() {
            if let Some(existing) = self.names.get(index) {
                assert!(
                    existing == name,
                    "file index {index} mismatch while merging: '{}' vs '{}'",
                    existing,
                    name
                );
            } else {
                self.names.push(Arc::clone(name));
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file_id: FileId,
    pub linenum: usize,
    pub category: Category,
    pub confidence: i32,
    pub message: LintMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub file_id: FileId,
    pub order: usize,
    pub stream: NoteStream,
    pub text: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedFile {
    pub file_id: FileId,
    pub had_error: bool,
}
