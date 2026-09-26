//! Background listeners that feed the Slint UI.
//!
//! The download event bus, the status pollers, the tray event loop and the
//! clipboard monitor. `start_event_bus_listener` used to be one 430-line
//! function with a single giant `match`; it is now one function per
//! `DownloadEvent` variant, sharing the store plumbing in [`bus`].

mod bus;
mod pollers;
mod tray;

pub use bus::start_event_bus_listener;
pub use pollers::{start_clipboard_monitor, start_status_pollers};
pub use tray::start_tray_event_loop;
