//! Localization helpers for the native client.
//!
//! Rust-side dynamic text must go through `format_*` helpers (never hardcoded
//! CJK) so the English UI does not leak Chinese. The helpers are split by
//! domain into submodules and re-exported here, so call sites keep using
//! `i18n::format_*` unchanged.

use limedl_core::types::DownloadState;

mod cdn;
mod dialogs;
mod language;
mod rewrite;
mod schedule;
mod task;
mod toast;
mod tray;
mod validation;

pub use cdn::*;
pub use dialogs::*;
pub use language::*;
pub use rewrite::*;
pub use schedule::*;
pub use task::*;
pub use toast::*;
pub use tray::*;
pub use validation::*;

#[cfg(test)]
mod tests;
