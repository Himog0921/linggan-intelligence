//! Optional OCR second opinions, never ground truth or automatic corrections.
pub mod contract;
pub mod difference;
pub mod report;
pub mod review;
pub mod sampling;
pub mod schedule;
pub mod source;
pub mod store;
pub use contract::*;
pub const CONTRACT: &str = "ocr-visual-audit.v1";
pub const PROMPT_VERSION: &str = "ocr-blind-read.v4";
