//! Task-list callbacks, split by concern.
//!
//! `register` used to be a single ~800-line function holding every callback of
//! the task list; it is now a table of contents over the modules below.

mod batch;
mod clipboard;
mod list;
mod selection;
mod single;

use crate::context::AppContext;

/// Wire every task-list callback onto the main window.
pub fn register(ctx: &AppContext) {
    list::register(ctx);
    selection::register(ctx);
    batch::register(ctx);
    single::register(ctx);
    clipboard::register(ctx);
}
