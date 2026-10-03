//! GEDCOM 5.5 / 5.5.1 / 7.0 import and export.
//!
//! Import maps known structures onto the relational model and stores every unrecognised structure verbatim
//! (`raw_tag`) so export can re-emit it. Original xrefs are kept so pointers inside raw structures stay valid.

pub mod charset;
mod export;
mod import;
pub mod tree;

pub use charset::Charset;
pub use export::{export, Dialect, ExportOptions, LivingPolicy, Version};
pub use import::{import, ImportReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    /// 1-based source line, 0 if not tied to a line.
    pub line: usize,
    pub message: String,
}

impl Issue {
    pub fn new(severity: Severity, line: usize, message: String) -> Issue {
        Issue {
            severity,
            line,
            message,
        }
    }
}
