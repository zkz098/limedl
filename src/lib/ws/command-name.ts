import { WS_COMMANDS } from "./generated/ws-commands";

const known = new Set<string>(WS_COMMANDS.map((c) => c.tauriName));

/**
 * Resolve a command name against the generated `WS_COMMANDS` manifest, so the
 * frontend's invoke strings stay in sync with the single source of truth
 * (`ws_manifest.rs`) instead of silently drifting. Throws on an unknown command.
 */
export function commandName(name: string): string {
  if (!known.has(name)) {
    throw new Error(
      `Unknown command "${name}". Add it to WS_COMMANDS in ` +
        "crates/limedl-core/src/ws_manifest.rs and regenerate bindings.",
    );
  }
  return name;
}
