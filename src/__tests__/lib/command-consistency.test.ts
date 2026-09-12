import { describe, it, expect } from "vitest";

import { WS_COMMANDS } from "../../lib/ws/generated/ws-commands";

// Read the api sources as raw strings (Vite `?raw`) so we can statically scan
// every commandName(...) invocation without node fs access.
import appApi from "../../lib/ipc/app-api.ts?raw";
import cdnApi from "../../lib/ipc/cdn-api.ts?raw";
import dialogApi from "../../lib/ipc/dialog-api.ts?raw";
import downloadApi from "../../lib/ipc/download-api.ts?raw";
import settingsApi from "../../lib/ipc/settings-api.ts?raw";

/**
 * Guards the RPC command surface against drift.
 *
 * The command name set is declared in just one source (`ws_manifest.rs` →
 * generated `ws-commands.ts`), then used by the server dispatcher and the
 * frontend `*-api.ts` invoke calls. This test forces both sides to agree:
 *   - every command the frontend invokes exists in the manifest
 *   - every manifest command is actually wired into a frontend api function
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
  it("every frontend-invoked command exists in WS_COMMANDS", () => {
    const used = extractInvokeCommands();
    const known = new Set(WS_COMMANDS.map((c) => c.tauriName));
    for (const cmd of used) {
      expect(known.has(cmd), `invoked "${cmd}" is not in WS_COMMANDS`).toBe(true);
    }
  });

  it("every WS_COMMANDS command is wired into a frontend api function", () => {
    const used = new Set(extractInvokeCommands());
    const manifestNames = WS_COMMANDS.map((c) => c.tauriName);
    for (const cmd of manifestNames) {
      expect(used.has(cmd), `WS_COMMANDS "${cmd}" is not invoked by any frontend api`).toBe(true);
    }
  });

  it("the frontend invokes nothing outside the manifest", () => {
    const used = new Set(extractInvokeCommands());
    const inManifest = new Set(WS_COMMANDS.map((c) => c.tauriName));
    const invokedNotInManifest = [...used].filter((c) => !inManifest.has(c)).toSorted();
    expect(invokedNotInManifest).toEqual([]);
  });
});
