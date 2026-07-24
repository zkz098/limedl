//! WebSocket command manifest — single source of truth for Tauri command → JSON-RPC
//! method mapping and parameter transformation rules.
//!
//! # Adding a new WS command
//!
//! 1. Add a `WsCommandSpec` entry to [`WS_COMMANDS`] below.
//! 2. Add a handler in `crates/limedl-server/src/rpc.rs` (`dispatch_method` +
//!    sub-handler if grouped). The match arm must use the `tauri_name` string
//!    from step 1. See the consistency test `all_rpc_methods_have_dispatch_arms`
//!    below.
//! 3. Run `cargo test --features ts export_typescript_bindings` to regenerate
//!    `src/lib/ws/generated/ws-commands.ts`.
//! 4. The frontend typed wrappers in `src/lib/tauri/*-api.ts` can then call the
//!    new command through `#invoke` without any manual `METHOD_MAP` / `transformParams`
//!    edits in `ws-invoke.ts`.
//!
//! # Safety
//!
//! All `&'static str` fields are compile-time constants. The whole module has zero
//! runtime overhead in production builds.

use serde::Serialize;

#[cfg(feature = "ts")]
use ts_rs::TS;

/// Specifies how a command's `args` (the second argument to `invoke()`) should be
/// transformed before sending as JSON-RPC `params`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ParamTransform {
    /// Pass all arguments through unchanged.
    Identity,
    /// Rename a single field from `from` to `to`.
    Rename {
        from: &'static str,
        to: &'static str,
    },
    /// Extract the value of a nested field (e.g. `{ request: … }`) and use it
    /// directly as the JSON-RPC params.
    UnwrapField {
        field: &'static str,
    },
}

/// Safety classification for JSON-RPC method rate limiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "../../src/types/generated/types.ts"))]
#[serde(rename_all = "camelCase")]
pub enum SafetyClass {
    /// Read-only / query methods.
    Safe,
    /// Mutating / write methods.
    Mutating,
}

/// Describes a single WebSocket JSON-RPC command.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsCommandSpec {
    /// Tauri command name (snake_case), e.g. `"download_pause"`.
    pub tauri_name: &'static str,
    /// JSON-RPC method name (dot-separated), e.g. `"download.pause"`.
    pub rpc_method: &'static str,
    /// How to transform `args` before sending as JSON-RPC params.
    pub param_transform: ParamTransform,
    /// Safety classification for rate limiting.
    pub safety: SafetyClass,
}

/// Complete list of all WebSocket JSON-RPC commands.
///
/// This is the **single source of truth**. The frontend generated file
/// `src/lib/ws/generated/ws-commands.ts` is produced from this constant.
pub const WS_COMMANDS: &[WsCommandSpec] = &[
    // ── Download lifecycle ──────────────────────────────────────────────
    WsCommandSpec {
        tauri_name: "download_start",
        rpc_method: "download.start",
        param_transform: ParamTransform::UnwrapField { field: "request" },
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "download_pause",
        rpc_method: "download.pause",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "download_resume",
        rpc_method: "download.resume",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "download_cancel",
        rpc_method: "download.cancel",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "download_remove",
        rpc_method: "download.remove",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "download_purge",
        rpc_method: "download.purge",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "download_status",
        rpc_method: "download.status",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "download_list",
        rpc_method: "download.list",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "download_open_in_explorer",
        rpc_method: "download.openInExplorer",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Mutating,
    },
    // ── Settings ────────────────────────────────────────────────────────
    WsCommandSpec {
        tauri_name: "settings_get",
        rpc_method: "settings.get",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "settings_save",
        rpc_method: "settings.save",
        param_transform: ParamTransform::UnwrapField { field: "settings" },
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "toggle_game_mode",
        rpc_method: "settings.toggleGameMode",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "get_io_status",
        rpc_method: "settings.getIoStatus",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "detect_disk_type",
        rpc_method: "settings.detectDiskType",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "toggle_overclock_mode",
        rpc_method: "settings.toggleOverclockMode",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "get_overclock_mode",
        rpc_method: "settings.getOverclockMode",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "settings_fetch_tracker_list",
        rpc_method: "settings.fetchTrackerList",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Mutating,
    },
    // ── BitTorrent ────────────────────────────────────────────────────────
    WsCommandSpec {
        tauri_name: "bt_runtime_status",
        rpc_method: "bt.runtimeStatus",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "bt_set_speed_limit",
        rpc_method: "bt.setSpeedLimit",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "bt_preview_torrent",
        rpc_method: "bt.previewTorrent",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "bt_get_peers",
        rpc_method: "bt.getPeers",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "bt_get_trackers",
        rpc_method: "bt.getTrackers",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "bt_get_pieces",
        rpc_method: "bt.getPieces",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "get_bt_files",
        rpc_method: "bt.getFiles",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "update_bt_files",
        rpc_method: "bt.updateFiles",
        param_transform: ParamTransform::Rename {
            from: "downloadId",
            to: "taskId",
        },
        safety: SafetyClass::Mutating,
    },
    // ── CDN accelerator ────────────────────────────────────────────────
    WsCommandSpec {
        tauri_name: "cdn_fetch_ranges",
        rpc_method: "cdn.fetchRanges",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "cdn_test",
        rpc_method: "cdn.test",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "cdn_apply",
        rpc_method: "cdn.apply",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "cdn_clear",
        rpc_method: "cdn.clear",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "cdn_status",
        rpc_method: "cdn.status",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "cdn_cancel",
        rpc_method: "cdn.cancel",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Mutating,
    },
    WsCommandSpec {
        tauri_name: "cdn_detail",
        rpc_method: "cdn.detail",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Safe,
    },
    WsCommandSpec {
        tauri_name: "cdn_candidates",
        rpc_method: "cdn.candidates",
        param_transform: ParamTransform::Identity,
        safety: SafetyClass::Safe,
    },
];

/// Classify a JSON-RPC method as Safe or Mutating based on `WS_COMMANDS`.
/// Unknown RPC methods (not in manifest) default to Mutating.
pub fn classify_rpc_safety(rpc_method: &str) -> SafetyClass {
    WS_COMMANDS
        .iter()
        .find(|c| c.rpc_method == rpc_method)
        .map(|c| c.safety)
        .unwrap_or(SafetyClass::Mutating)
}

// ── Event manifest ──────────────────────────────────────────────────────────

/// Describes a single WebSocket event notification mapping from `DownloadEvent`
/// variant to its frontend event names.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsEventSpec {
    /// WebSocket JSON-RPC notification `type` field value (e.g. `"updated"`).
    pub ws_type: &'static str,
    /// Tauri event name emitted to the frontend (e.g. `"download-updated"`).
    pub tauri_event_name: &'static str,
}

/// Complete list of all WebSocket event notification mappings.
///
/// Each entry maps a `DownloadEvent` variant's WebSocket `type` field (used in
/// the RPC adapter JSON-RPC notifications in `rpc.rs`) to its corresponding
/// Tauri event name (used by the Tauri adapter in `lib.rs` and the frontend
/// event dispatcher in `ws-invoke.ts`).
///
/// This is the **single source of truth** for event name mappings across the
/// Tauri adapter, the RPC adapter, and the frontend. See the consistency tests
/// below (`ws_event_types_appear_in_rpc_adapter` and
/// `ws_event_tauri_names_appear_in_lib_rs`) for cross-crate guard.
///
/// Note: `aria2Notification` uses a dynamic `event_name` in the Tauri adapter
/// (not a fixed string), so its `tauri_event_name` is only meaningful for the
/// NAS WebSocket frontend mapping and is excluded from the lib.rs consistency
/// check.
pub const WS_EVENTS: &[WsEventSpec] = &[
    WsEventSpec {
        ws_type: "updated",
        tauri_event_name: "download-updated",
    },
    WsEventSpec {
        ws_type: "progress",
        tauri_event_name: "download-progress",
    },
    WsEventSpec {
        ws_type: "aria2Notification",
        tauri_event_name: "aria2-notification",
    },
    WsEventSpec {
        ws_type: "cdnProgress",
        tauri_event_name: "cdn-test-progress",
    },
    WsEventSpec {
        ws_type: "cdnComplete",
        tauri_event_name: "cdn-test-complete",
    },
    WsEventSpec {
        ws_type: "warning",
        tauri_event_name: "download-warning",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `literal` (a quoted string like `"foo"`) appears in `text`
    /// followed (after ASCII whitespace) by one of `|`, `=`, or `,`. These
    /// characters form the syntactic neighbors of a JSON-RPC match arm pattern
    /// (`"foo" | "bar" => ...`) and a positional emit/field reference
    /// (`emit("foo", ...)` / json `{"type": "foo", ...}`), both of which are the
    /// contexts where command/event literal strings are actually expected to
    /// appear in `rpc.rs` and `lib.rs`. A bare string in a comment, a doc
    /// example, or a freestanding format string would not be followed by any
    /// of these chars and is therefore rejected, tightening the original
    /// `text.contains(&quoted)` guard.
    fn appears_in_structural_context(text: &str, literal: &str) -> bool {
        let mut search_start = 0;
        while let Some(pos) = text[search_start..].find(literal) {
            let abs_pos = search_start + pos;
            let after = abs_pos + literal.len();
            if after < text.len() {
                let tail = &text[after..];
                for ch in tail.chars() {
                    if ch.is_ascii_whitespace() {
                        continue;
                    }
                    if matches!(ch, '|' | '=' | ',') {
                        return true;
                    }
                    break;
                }
            }
            search_start = abs_pos + 1;
            if search_start >= text.len() {
                break;
            }
        }
        false
    }

    /// Verify that every entry has a non-empty name and method.
    #[test]
    fn ws_commands_are_valid() {
        for cmd in WS_COMMANDS {
            assert!(!cmd.tauri_name.is_empty(), "tauri_name must not be empty");
            assert!(!cmd.rpc_method.is_empty(), "rpc_method must not be empty");
        }
    }

    /// Verify that all tauri command names are unique.
    #[test]
    fn ws_commands_have_unique_names() {
        let mut seen = std::collections::HashSet::new();
        for cmd in WS_COMMANDS {
            assert!(
                seen.insert(cmd.tauri_name),
                "duplicate tauri_name: {}",
                cmd.tauri_name
            );
        }
    }

    /// Verify that all rpc method names are unique.
    #[test]
    fn ws_commands_have_unique_methods() {
        let mut seen = std::collections::HashSet::new();
        for cmd in WS_COMMANDS {
            assert!(
                seen.insert(cmd.rpc_method),
                "duplicate rpc_method: {}",
                cmd.rpc_method
            );
        }
    }

    /// Cross-crate consistency guard: every `tauri_name` in `WS_COMMANDS` must
    /// appear as a string literal in `crates/limedl-server/src/rpc.rs`
    /// `dispatch_method`.  After the dual-source elimination (rpc.rs now routes
    /// via the rpc_method → tauri_name lookup derived from WS_COMMANDS), this
    /// test verifies that every ws_manifest entry has a corresponding match arm
    /// in the dispatch function.
    ///
    /// Uses `include_str!` to embed the rpc.rs source at compile time — no
    /// runtime filesystem access needed.
    #[test]
    fn all_rpc_methods_have_dispatch_arms() {
        let rpc_source = include_str!("../../limedl-server/src/rpc.rs");
        for cmd in WS_COMMANDS {
            let quoted = format!("\"{}\"", cmd.tauri_name);
            assert!(
                appears_in_structural_context(rpc_source, &quoted),
                "rpc.rs is missing a match arm for tauri_name '{}' (rpc_method: '{}').\n\
                 Add a handler in crates/limedl-server/src/rpc.rs dispatch_method()\n\
                 (and a sub-handler such as handle_download_action / handle_bt_get_details\n\
                 / handle_cdn_routes if the command belongs to a grouped dispatch arm).",
                cmd.tauri_name, cmd.rpc_method
            );
        }
    }

    // ── Event manifest tests ──────────────────────────────────────────────

    /// Verify that every event entry has non-empty fields.
    #[test]
    fn ws_events_are_valid() {
        for ev in WS_EVENTS {
            assert!(!ev.ws_type.is_empty(), "ws_type must not be empty");
            assert!(
                !ev.tauri_event_name.is_empty(),
                "tauri_event_name must not be empty"
            );
        }
    }

    /// Verify that all event ws_type values are unique.
    #[test]
    fn ws_events_have_unique_ws_types() {
        let mut seen = std::collections::HashSet::new();
        for ev in WS_EVENTS {
            assert!(
                seen.insert(ev.ws_type),
                "duplicate ws_type: {}",
                ev.ws_type
            );
        }
    }

    /// Cross-crate consistency guard: every `ws_type` in `WS_EVENTS` must
    /// appear as a string literal in `crates/limedl-server/src/rpc.rs` event
    /// relay (the match on `DownloadEvent` variants that produces the `type`
    /// field in JSON-RPC notification params).
    #[test]
    fn ws_event_types_appear_in_rpc_adapter() {
        let rpc_source = include_str!("../../limedl-server/src/rpc.rs");
        for ev in WS_EVENTS {
            let quoted = format!("\"{}\"", ev.ws_type);
            assert!(
                appears_in_structural_context(rpc_source, &quoted),
                "rpc.rs is missing ws_type string '{}' (tauri_event_name: '{}').\n\
                 Add or fix the event relay arm in crates/limedl-server/src/rpc.rs.",
                ev.ws_type, ev.tauri_event_name
            );
        }
    }

    /// Cross-crate consistency guard: every `tauri_event_name` in `WS_EVENTS`
    /// must appear as a string literal in `src-tauri/src/lib.rs` Tauri event
    /// emission.
    ///
    /// Exception: `aria2Notification` uses a dynamic `event_name` in the Tauri
    /// adapter (passed through directly from the BT backend), not a fixed
    /// event name string, so it is excluded from this check.
    #[test]
    fn ws_event_tauri_names_appear_in_lib_rs() {
        let lib_source = include_str!("../../../src-tauri/src/lib.rs");
        for ev in WS_EVENTS {
            // Aria2Notification uses a dynamic event_name in the Tauri adapter
            if ev.ws_type == "aria2Notification" {
                continue;
            }
            let quoted = format!("\"{}\"", ev.tauri_event_name);
            assert!(
                appears_in_structural_context(lib_source, &quoted),
                "lib.rs is missing tauri_event_name string '{}' (ws_type: '{}').\n\
                 Add or fix the emit arm in src-tauri/src/lib.rs.",
                ev.tauri_event_name, ev.ws_type
            );
        }
    }
}
