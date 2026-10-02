use std::time::Duration;
use std::{path::PathBuf, sync::Arc};

use foldhash::HashMap;

use axum::{
    Router,
    extract::{
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use irontide::core::Id20;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::{
    backend_registry::BackendRegistry,
    bt_backend::LazyBtBackend,
    dispatcher::Dispatcher,
    event_bus::{DownloadEvent, EventBus},
    http::has_header,
    manager::DownloadManager,
    types::{
        Aria2RpcSettings, BtPeerInfo, ChecksumMode, DownloadState, DownloadSummary,
        StartDownloadRequest, TaskId, TaskKind,
    },
};

mod context;
mod dispatch;
mod download;
mod options;
mod protocol;
mod query;
mod server;
mod system;
mod transport;

pub(crate) use context::*;
pub(crate) use dispatch::*;
pub(crate) use download::*;
pub(crate) use options::*;
pub(crate) use protocol::*;
pub(crate) use query::*;
pub(crate) use system::*;
pub(crate) use transport::*;

pub use protocol::internal_id_to_gid;
pub use server::Aria2RpcServer;
pub use system::cleanup_old_aria2_temp_files;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod e2e_tests;
