/// TypeScript binding export test.
///
/// Run with: `cargo test --manifest-path crates/limedl-core/Cargo.toml --features ts`
///
/// When compiled with `--features ts`, each type annotated with
/// `#[cfg_attr(feature = "ts", derive(TS))]` is exported to
/// `src/types/generated/types.ts`.
///
/// Additionally, the WS command manifest (`src/ws_manifest.rs`) is exported to
/// `src/lib/ws/generated/ws-commands.ts`.
#[cfg(feature = "ts")]
#[test]
fn export_typescript_bindings() {
    use crate::cdn::{CdnTestPhase, CdnTestProgress, DefaultNodeResult, SpeedTestResult};
    use crate::types::*;
    use crate::ws_manifest::SafetyClass;
    use ts_rs::{Config, TS};

    // Use "number" instead of "bigint" for u64/i64 fields — the frontend
    // uses JavaScript number which safely handles values up to 2^53.
    let config = Config::default()
        .with_out_dir(".")
        .with_large_int("number");

    // ===== Settings types =====
    AppSettings::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export AppSettings: {e}");
    });

    // ===== Download top-level types not covered by AppSettings deps =====
    DownloadSnapshot::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export DownloadSnapshot: {e}");
    });
    StartDownloadRequest::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export StartDownloadRequest: {e}");
    });
    DownloadSummary::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export DownloadSummary: {e}");
    });
    DownloadProgress::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export DownloadProgress: {e}");
    });
    BtRuntimeStatus::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export BtRuntimeStatus: {e}");
    });
    TorrentFileEntry::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export TorrentFileEntry: {e}");
    });
    BtPeerInfo::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export BtPeerInfo: {e}");
    });
    BtTrackerInfo::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export BtTrackerInfo: {e}");
    });
    BtPieceInfo::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export BtPieceInfo: {e}");
    });
    BtFileStatus::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export BtFileStatus: {e}");
    });
    SerializableError::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export SerializableError: {e}");
    });

    // ===== CDN types =====
    SpeedTestResult::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export SpeedTestResult: {e}");
    });
    DefaultNodeResult::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export DefaultNodeResult: {e}");
    });
    CdnTestPhase::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export CdnTestPhase: {e}");
    });
    CdnTestProgress::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export CdnTestProgress: {e}");
    });

    // ===== SafetyClass (referenced by WsCommandSpec) =====
    SafetyClass::export_all(&config).unwrap_or_else(|e| {
        panic!("Failed to export SafetyClass: {e}");
    });

    // ===== WebSocket command manifest =====
    export_ws_commands().unwrap_or_else(|e| {
        panic!("Failed to export ws-commands.ts: {e}");
    });

    // ===== WebSocket event manifest =====
    export_ws_events().unwrap_or_else(|e| {
        panic!("Failed to export ws-events.ts: {e}");
    });

    // ===== AppSettings default value =====
    export_settings_default().unwrap_or_else(|e| {
        panic!("Failed to export settings-default.ts: {e}");
    });
}

/// Generate `src/lib/ws/generated/ws-commands.ts` from [`WS_COMMANDS`].
///
/// Uses `CARGO_MANIFEST_DIR` (set by Cargo at compile time) to resolve the
/// absolute output path, ensuring the file is written to the correct location
/// regardless of the current working directory at test time.
#[cfg(feature = "ts")]
fn export_ws_commands() -> std::io::Result<()> {
    use std::io::Write;
    use std::path::PathBuf;

    use crate::ws_manifest::{ParamTransform, SafetyClass, WS_COMMANDS};

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR = crates/limedl-core/
    // target:            = ../../src/lib/ws/generated/ws-commands.ts
    let output_path = manifest_dir
        .join("../../src/lib/ws/generated/ws-commands.ts")
        .canonicalize()
        .unwrap_or_else(|_| {
            // Directory may not exist yet — construct path without canonicalize
            let base = manifest_dir.parent().unwrap().parent().unwrap().join("src");
            base.join("lib/ws/generated/ws-commands.ts")
        });

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut out = std::fs::File::create(&output_path)?;

    writeln!(
        out,
        "// AUTO-GENERATED by `cargo test --features ts`. DO NOT EDIT."
    )?;
    writeln!(
        out,
        "// Source of truth: crates/limedl-core/src/ws_manifest.rs"
    )?;
    writeln!(out)?;

    // ── Interface definition ────────────────────────────────────────────
    writeln!(out, "export interface WsCommandSpec {{")?;
    writeln!(out, "  tauriName: string;")?;
    writeln!(out, "  rpcMethod: string;")?;
    writeln!(out, "  paramTransform:")?;
    writeln!(out, "    | {{ kind: \"identity\" }}")?;
    writeln!(
        out,
        "    | {{ kind: \"rename\"; from: string; to: string }}"
    )?;
    writeln!(
        out,
        "    | {{ kind: \"unwrapField\"; field: string }};"
    )?;
    writeln!(out, "  safety: \"safe\" | \"mutating\";")?;
    writeln!(out, "}}")?;
    writeln!(out)?;

    // ── WS_COMMANDS array ──────────────────────────────────────────────
    writeln!(out, "export const WS_COMMANDS: readonly WsCommandSpec[] = [")?;
    for cmd in WS_COMMANDS {
        let transform = match &cmd.param_transform {
            ParamTransform::Identity => r#"{ kind: "identity" }"#.to_string(),
            ParamTransform::Rename { from, to } => {
                format!(r#"{{ kind: "rename", from: "{}", to: "{}" }}"#, from, to)
            }
            ParamTransform::UnwrapField { field } => {
                format!(r#"{{ kind: "unwrapField", field: "{}" }}"#, field)
            }
        };
        let safety_str = match cmd.safety {
            SafetyClass::Safe => "safe",
            SafetyClass::Mutating => "mutating",
        };
        writeln!(
            out,
            r#"  {{ tauriName: "{}", rpcMethod: "{}", paramTransform: {}, safety: "{}" }},"#,
            cmd.tauri_name, cmd.rpc_method, transform, safety_str
        )?;
    }
    writeln!(out, "];")?;
    writeln!(out)?;

    // ── METHOD_MAP convenience lookup ──────────────────────────────────
    writeln!(
        out,
        "export const METHOD_MAP: Record<string, string> = Object.fromEntries("
    )?;
    writeln!(
        out,
        "  WS_COMMANDS.map(c => [c.tauriName, c.rpcMethod])"
    )?;
    writeln!(out, ");")?;
    writeln!(out)?;

    writeln!(out, "// Re-export for backwards compatibility")?;
    writeln!(out, "export default WS_COMMANDS;")?;

    eprintln!(
        "[ws-manifest] Wrote {} entries to {:?}",
        WS_COMMANDS.len(),
        output_path
    );
    Ok(())
}

/// Generate `src/lib/ws/generated/ws-events.ts` from [`WS_EVENTS`].
///
/// Uses `CARGO_MANIFEST_DIR` (set by Cargo at compile time) to resolve the
/// absolute output path, ensuring the file is written to the correct location
/// regardless of the current working directory at test time.
#[cfg(feature = "ts")]
fn export_ws_events() -> std::io::Result<()> {
    use std::io::Write;
    use std::path::PathBuf;

    use crate::ws_manifest::WS_EVENTS;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR = crates/limedl-core/
    // target:            = ../../src/lib/ws/generated/ws-events.ts
    let output_path = manifest_dir
        .join("../../src/lib/ws/generated/ws-events.ts")
        .canonicalize()
        .unwrap_or_else(|_| {
            let base = manifest_dir.parent().unwrap().parent().unwrap().join("src");
            base.join("lib/ws/generated/ws-events.ts")
        });

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut out = std::fs::File::create(&output_path)?;

    writeln!(
        out,
        "// AUTO-GENERATED by `cargo test --features ts`. DO NOT EDIT."
    )?;
    writeln!(
        out,
        "// Source of truth: crates/limedl-core/src/ws_manifest.rs"
    )?;
    writeln!(out)?;

    // ── Interface definition ────────────────────────────────────────────
    writeln!(out, "export interface WsEventSpec {{")?;
    writeln!(out, "  wsType: string;")?;
    writeln!(out, "  tauriEventName: string;")?;
    writeln!(out, "}}")?;
    writeln!(out)?;

    // ── WS_EVENTS array ────────────────────────────────────────────────
    writeln!(out, "export const WS_EVENTS: readonly WsEventSpec[] = [")?;
    for ev in WS_EVENTS {
        writeln!(
            out,
            r#"  {{ wsType: "{}", tauriEventName: "{}" }},"#,
            ev.ws_type, ev.tauri_event_name
        )?;
    }
    writeln!(out, "];")?;
    writeln!(out)?;

    // ── EVENT_TYPE_MAP convenience lookup ───────────────────────────────
    writeln!(
        out,
        "export const EVENT_TYPE_MAP: Record<string, string> = Object.fromEntries("
    )?;
    writeln!(
        out,
        "  WS_EVENTS.map(c => [c.wsType, c.tauriEventName])"
    )?;
    writeln!(out, ");")?;
    writeln!(out)?;

    eprintln!(
        "[ws-events] Wrote {} entries to {:?}",
        WS_EVENTS.len(),
        output_path
    );
    Ok(())
}

/// Generate `src/types/generated/settings-default.ts` from Rust's
/// `AppSettings::default()`, making Rust the single source of truth for the
/// frontend's fallback defaults (replaces the hand-maintained TS copy).
#[cfg(feature = "ts")]
fn export_settings_default() -> std::io::Result<()> {
    use std::io::Write;
    use std::path::PathBuf;

    use crate::types::AppSettings;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_path = manifest_dir
        .join("../../src/types/generated/settings-default.ts")
        .canonicalize()
        .unwrap_or_else(|_| {
            let base = manifest_dir.parent().unwrap().parent().unwrap().join("src");
            base.join("types/generated/settings-default.ts")
        });

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // `AppSettings`' settings sub-structs use `#[serde(default)]` (not
    // `skip_serializing_if`), so serializing `default()` emits every field.
    let json =
        serde_json::to_string_pretty(&AppSettings::default()).map_err(std::io::Error::other)?;

    let mut out = std::fs::File::create(&output_path)?;
    writeln!(
        out,
        "// AUTO-GENERATED by `cargo test --features ts`. DO NOT EDIT."
    )?;
    writeln!(
        out,
        "// Source of truth: crates/limedl-core/src/types.rs (`AppSettings::default()`)."
    )?;
    writeln!(out)?;
    writeln!(out, "import type {{ AppSettings }} from \"./types\";")?;
    writeln!(out)?;
    writeln!(out, "export const DEFAULT_APP_SETTINGS: AppSettings = {json};")?;
    writeln!(out)?;

    eprintln!("[settings-default] Wrote {:?}", output_path);
    Ok(())
}
