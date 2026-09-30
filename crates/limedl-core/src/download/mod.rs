//! Shared download-orchestration kernel.
//!
//! `manager.rs` is the facade (`DownloadManager`) and composes the actor types
//! (`http_executor`, `task_lifecycle`, `scheduler`). Everything the actors need
//! in common — per-task state, run outcomes, threading policy and path
//! helpers — lives here, so the actors depend on this module instead of on the
//! facade.

mod managed;
mod shared;

pub(crate) use managed::*;
pub use managed::{DownloadCore, ManagedDownload};
pub(crate) use shared::*;
