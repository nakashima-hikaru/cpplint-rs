use thiserror::Error;

#[derive(Error, Debug)]
pub enum CppLintError {
    #[error("Glob error: {0}")]
    Glob(#[from] globset::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),


    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Rayon thread pool error: {0}")]
    Rayon(#[from] rayon::ThreadPoolBuildError),

    #[error("General error: {0}")]
    General(String),
}

pub type Result<T> = std::result::Result<T, CppLintError>;
