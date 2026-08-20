import { WS_COMMANDS } from "./generated/ws-commands";

/**
 * Commands intentionally available only on Tauri desktop and therefore absent
 * from `WS_COMMANDS` (no WebSocket/NAS counterpart). The app updater has no
 * meaning on the NAS web server.
 */
const TAURI_ONLY_COMMANDS: readonly string[] = ["check_update_full", "download_and_install_update"];

const known = new Set<string>(WS_COMMANDS.map((c) => c.tauriName));
for (const name of TAURI_ONLY_COMMANDS) {
  known.add(name);
}

/**
 * Resolve a Tauri command name against the generated `WS_COMMANDS` manifest
 * (plus the explicit Tauri-only allowlist), so the frontend's invoke strings
 * stay in sync with the single source of truth (`ws_manifest.rs`) instead of
 * silently drifting. Throws on an unknown command.
 */
export function commandName(name: string): string {
  if (!known.has(name)) {
    throw new Error(
      `Unknown command "${name}". Add it to WS_COMMANDS in ` +
        "crates/limedl-core/src/ws_manifest.rs (or the Tauri-only allowlist " +
        "here in src/lib/ws/command-name.ts) and regenerate bindings.",
    );
  }
  return name;
}

export { TAURI_ONLY_COMMANDS };
