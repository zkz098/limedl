//! Settings callbacks.
//!
//! `register` used to be a single ~650-line function; it is now a table of
//! contents over the modules below.

mod aria2_clients;
mod dialog;
mod disk_overrides;
mod paths;
mod schedule;

use crate::context::AppContext;

/// Wire every settings callback onto the main window.
pub fn register(ctx: &AppContext) {
    dialog::register(ctx);
    schedule::register(ctx);
    disk_overrides::register(ctx);
    aria2_clients::register(ctx);
    paths::register(ctx);
}
