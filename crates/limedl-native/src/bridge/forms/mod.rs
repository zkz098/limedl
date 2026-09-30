//! Settings-form conversion, split by concern.
//!
//! - `combo.rs` — canonical option value lists shared with the Slint ComboBoxes
//! - `enums.rs` — enum <-> persisted-string conversions
//! - `to_form.rs` — `AppSettings` -> `SettingsFormData`
//! - `from_form.rs` — `SettingsFormData` -> `AppSettings` (validation + one
//!   applier per settings section)
//! - `speed_limit.rs` — speed-limit schedule row parsing/serialization

pub mod combo;
mod enums;
mod from_form;
mod speed_limit;
mod to_form;

pub(crate) use enums::*;
pub use from_form::*;
pub use speed_limit::*;
pub use to_form::*;
