import { test as base } from "@playwright/test";
import { WsMocker } from "./helpers/ws-mocker";

type MyFixtures = {
  wsMocker: WsMocker;
};

/**
 * Extended test fixture with WebSocket mocking support for NAS WebUI tests.
 *
 * Usage:
 * ```ts
 * import { test, expect } from "../fixtures";
 *
 * test("mocked download start", async ({ page, wsMocker }) => {
 *   await wsMocker.install(page);
 *   await page.goto("/");
 *   // ...
 * });
 * ```
 */
export const test = base.extend<MyFixtures>({
  wsMocker: async ({ context, page }, use) => {
    const mocker = new WsMocker();

    // Register auto-responses for RPC calls that the app makes on startup.
    // Without these, the app blocks waiting for responses and never renders.
    // Settings MUST include all nested fields that the app's init code reads
    // (e.g. download.defaultDownloadDir, scheduler.mode, etc.) — otherwise
    // applyAppSettingsDefaults throws and the app may fail to render.
    mocker.setAutoResponse("settings.get", {
      setupCompleted: true,
      globalSpeedLimitBps: 0,
      notifications: { enabled: true },
      appearance: {
        colorMode: "dark",
        themeColor: "lime",
        sortKey: "added_at",
        sortDirection: "desc",
        compactView: false,
        showDetailInfo: true,
        showHeatmap: true,
        backgroundOpacity: "default",
        visibleColumns: ["file", "size", "downloaded", "status", "progress", "speed", "eta"],
      },
      proxy: { mode: "disabled", manualUrl: "" },
      download: {
        defaultDownloadDir: "/downloads",
        defaultMaxRetries: 3,
        defaultChecksum: "none",
        defaultUserAgent: "",
      },
      scheduler: {
        mode: "automatic",
        traditional: { maxParallelTasks: 3 },
        automatic: {
          maxParallelThreads: 8,
          maxThreadsPerTask: 4,
          minThreadsPerTask: 1,
          adaptiveProfile: "balanced",
        },
        chunkSizeStrategy: "adaptive",
      },
      bt: {
        pauseUploadWhenLimitReached: false,
        uploadLimitBytes: 0,
        uploadRatioLimit: 0,
        dhtEnabled: true,
        trackerList: "",
        trackerListUrl: "",
        listenPort: null,
        listenPortRange: null,
        upnpEnabled: true,
        enableNatpmp: true,
        enableIpv6: false,
        enablePex: true,
        enableLsd: true,
        enableUtp: true,
        enableFastExtension: true,
        enableHolepunch: true,
        enableWebSeed: true,
        enableSuperSeeding: false,
        globalDownloadRateLimit: 0,
        globalUploadRateLimit: 0,
        preallocateMode: "none",
        encryptionMode: "enabled",
        maxDownloads: 5,
        maxSeeds: 3,
        maxTorrents: 20,
        activeLimit: 10,
      },
      logging: {
        enabled: false,
        level: "info",
        filePath: "",
        retentionCount: null,
        retentionDays: null,
      },
      aria2Rpc: { enabled: false, port: 6800, secret: null },
      cdnAcceleration: {
        enabled: false,
        activeIp: null,
        activeSpeedMbps: null,
        lastTestAtMs: null,
        lastError: null,
      },
      ioBaseline: {
        bufferLimitMb: 256,
        gameModeBufferMb: 64,
        gameMode: false,
        diskTypeOverrides: {},
        maxParallelHdd: 2,
        gameModeMaxParallel: 1,
        hddBufferEnabled: true,
      },
      network: {},
      cdn: {},
      labs: {},
      autostart: false,
      lastSetupStep: null,
    });
    mocker.setAutoResponse("download.list", []);
    mocker.setAutoResponse("bt.runtimeStatus", {
      dhtNodes: 0,
      uploadSpeedBytesPerSecond: 0,
      peerCount: 0,
      torrentCount: 0,
    });
    mocker.setAutoResponse("settings.getIoStatus", {
      gameMode: false,
      bufferUsageBytes: 0,
      bufferLimitBytes: 0,
      degradationCount: 0,
      activeSlots: 0,
      maxSlots: 0,
      queuedCount: 0,
    });
    mocker.setAutoResponse("settings.getOverclockMode", false);

    await mocker.install(page);
    // Seed localStorage to skip the setup wizard; must be on context
    // because page.addInitScript in a Playwright fixture does not
    // reliably run before page.goto. context.addInitScript does.
    await context.addInitScript(() => {
      localStorage.setItem("limedl.setupCompleted", "true");
    });
    await use(mocker);
  },
});

export { expect } from "@playwright/test";
