//! Localization helpers for the native client.
//!
//! Rust-side dynamic text must go through `format_*` helpers (never hardcoded
//! CJK) so the English UI does not leak Chinese. The helpers are split by
//! domain into submodules and re-exported here, so call sites keep using
//! `i18n::format_*` unchanged.
//!
//! The strings themselves live in the gettext catalogs
//! (`lang/<locale>/LC_MESSAGES/limedl-native.po`): `@tr` in `.slint` files and
//! [`tr`]/[`tr_fmt`] here read the same catalogs, so a message used from both
//! sides is translated once. `catalog.rs` is generated from them by
//! `cargo xtask i18n generate`.

use limedl_core::types::DownloadState;

mod catalog;
mod tr;

pub use tr::{tr, tr_fmt};

mod cdn;
mod dialogs;
mod disk;
mod language;
mod rewrite;
mod schedule;
mod task;
mod toast;
mod tray;
mod validation;

pub use cdn::*;
pub use dialogs::*;
pub use disk::*;
pub use language::*;
pub use rewrite::*;
pub use schedule::*;
pub use task::*;
pub use toast::*;
pub use tray::*;
pub use validation::*;

#[cfg(test)]
mod tests;
