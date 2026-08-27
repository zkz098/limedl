import { describe, it, expect } from "vitest";

import { WS_COMMANDS } from "../../lib/ws/generated/ws-commands";
import { TAURI_ONLY_COMMANDS } from "../../lib/ws/command-name";

// Read the api sources as raw strings (Vite `?raw`) so we can statically scan
// every commandName(...) invocation without node fs access.
import appApi from "../../lib/tauri/app-api.ts?raw";
import cdnApi from "../../lib/tauri/cdn-api.ts?raw";
import dialogApi from "../../lib/tauri/dialog-api.ts?raw";
import downloadApi from "../../lib/tauri/download-api.ts?raw";
import settingsApi from "../../lib/tauri/settings-api.ts?raw";

/**
 * Guards the RPC command surface against drift.
 *
 * The command name set is declared in just one source (`ws_manifest.rs` →
 * generated `ws-commands.ts`), then re-used by the Tauri command layer and the
 * frontend `*-api.ts` invoke calls. This test forces those three to agree:
 *   - every command the frontend invokes is known (manifest ∪ Tauri-only)
 *   - every manifest command is actually wired into the frontend api
 *   - the Tauri-only allowlist exactly matches the invoked-but-not-in-manifest set
 */
const API_SOURCES = [appApi, cdnApi, dialogApi, downloadApi, settingsApi];

function extractInvokeCommands(): string[] {
  const commands: string[] = [];
  const re = /commandName\(\s*"([a-z_0-9]+)"/g;
  for (const src of API_SOURCES) {
    let m: RegExpExecArray | null;
    while ((m = re.exec(src))) {
      commands.push(m[1]);
    }
  }
  return commands;
}

describe("RPC command-surface consistency", () => {
  it("every frontend-invoked command exists in WS_COMMANDS or the Tauri-only allowlist", () => {
    const used = extractInvokeCommands();
    const known = new Set(WS_COMMANDS.map((c) => c.tauriName));
    for (const name of TAURI_ONLY_COMMANDS) known.add(name);
    for (const cmd of used) {
      expect(
        known.has(cmd),
        `invoked "${cmd}" is not in WS_COMMANDS nor Tauri-only allowlist`,
      ).toBe(true);
    }
  });

  it("every WS_COMMANDS command is wired into a frontend api function", () => {
    const used = new Set(extractInvokeCommands());
    const manifestNames = WS_COMMANDS.map((c) => c.tauriName);
    for (const cmd of manifestNames) {
      expect(used.has(cmd), `WS_COMMANDS "${cmd}" is not invoked by any frontend api`).toBe(true);
    }
  });

  it("the Tauri-only allowlist exactly matches invoked commands absent from WS_COMMANDS", () => {
    const used = new Set(extractInvokeCommands());
    const inManifest = new Set(WS_COMMANDS.map((c) => c.tauriName));
    const invokedNotInManifest = [...used].filter((c) => !inManifest.has(c)).toSorted();
    const allowlist = [...TAURI_ONLY_COMMANDS].toSorted();
    expect(invokedNotInManifest).toEqual(allowlist);
  });
});
