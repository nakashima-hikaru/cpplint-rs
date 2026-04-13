//! Core linting engine for `cpplint-rs`.
//!
//! Inspired by Ruff's layering, the crate separates:
//! - source ingestion (`source`)
//! - diagnostics/session state (`diagnostics`, `state`)
//! - rule-family registration and dispatch (`registry`, `checks`)
//! - CLI-facing orchestration (`runner`, `output`, `config`)
//!
//! Existing entry points such as `FileLinter`, `Options`, and `CppLintState`
//! remain available for compatibility.

pub mod c_headers;
pub mod categories;
pub mod checks;
pub mod cleanse;
pub mod config;
pub mod diagnostics;
pub mod errors;
pub(crate) mod facts;
pub mod file_linter;
pub mod file_reader;
pub mod fixer;
pub mod glob;
pub mod line_utils;
pub mod options;
pub mod output;
pub(crate) mod regex_utils;
pub mod registry;
pub mod runner;
pub mod source;
pub mod state;
pub mod string_utils;
pub mod suppressions;
