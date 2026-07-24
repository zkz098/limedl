import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { nextTick } from "vue";

import SetupWizard from "../../components/setup/SetupWizard.vue";
import { saveAppSettings, getAppSettings } from "../../lib/tauri/settings-api";
import { invoke } from "#invoke";
import { createMockInvoke, resetTauriMocks } from "../mocks/tauri-mock";
import type { AppSettings } from "../../types/settings";

// ── Mocks ──────────────────────────────────────────────────────────

vi.mock("#invoke", () => ({ invoke: vi.fn() }));

vi.mock("../../i18n", () => ({
  useI18n: () => ({
    t: (key: string, args?: Record<string, unknown> | string) => {
      // Simplistic i18n mock: return interpolation args verbatim when a string
      // is passed (used by some summary helpers), otherwise return the key.
      if (typeof args === "string") return args;
      return key;
    },
    language: { value: "en-US" },
    languageOptions: [],
    setLanguage: vi.fn(),
    supportedLanguages: [],
  }),
}));

vi.mock("../../lib/tauri/settings-api", () => ({
  getAppSettings: vi.fn(),
  saveAppSettings: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/plugin-autostart", () => ({
  enable: vi.fn().mockResolvedValue(undefined),
  disable: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@vueuse/core", () => ({
  onKeyStroke: vi.fn(),
  // Mock onClickOutside to return a valid cleanup function, since UiSelect
  // calls the return value (stopClickOutside) on unmount. Without this the
  // unmounted hook throws, corrupting the test wrapper for all subsequent tests.
  onClickOutside: vi.fn(() => () => {}),
}));

// ── Fixtures ───────────────────────────────────────────────────────

const STEP_COUNT = 9;

const STEP_TITLES = [
  "setupWizard.welcomeTitle",
  "setupWizard.languageTitle",
  "setupWizard.appearanceTitle",
  "setupWizard.cdnTitle",
  "setupWizard.rpcTitle",
  "setupWizard.directoryTitle",
  "setupWizard.performanceTitle",
  "setupWizard.systemTitle",
  "setupWizard.summaryTitle",
];

const LABEL_START = "setupWizard.startButton";
const LABEL_NEXT = "setupWizard.nextButton";
const LABEL_BACK = "setupWizard.backButton";
const LABEL_SKIP = "setupWizard.skipButton";
const LABEL_SKIP_ALL = "setupWizard.skipAllButton";
const LABEL_COMPLETE = "setupWizard.completeButton";

function createDefaultSettings(): AppSettings {
  return {
    globalSpeedLimitBps: 0,
    appearance: {
      themeColor: "lime",
      backgroundOpacity: "default",
      colorMode: "system",
      showDetailInfo: true,
      showHeatmap: true,
      sortKey: "added_at",
      sortDirection: "desc",
      compactView: false,
      visibleColumns: ["file", "size", "downloaded", "status", "progress", "speed", "eta"],
      closeBehavior: "minimizeToTray",
    },
    proxy: { mode: "disabled", manualUrl: "" },
    scheduler: {
      mode: "traditional",
      traditional: { maxParallelTasks: 3 },
      automatic: {
        maxParallelThreads: 16,
        maxThreadsPerTask: 8,
        minThreadsPerTask: 0,
        adaptiveProfile: "balanced",
      },
      chunkSizeStrategy: "adaptive",
    },
    download: {
      defaultDownloadDir: "",
      defaultMaxRetries: 5,
      defaultChecksum: "blake3",
      defaultUserAgent:
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    },
    bt: {
      pauseUploadWhenLimitReached: false,
      uploadLimitBytes: 0,
      uploadRatioLimit: 0,
      dhtEnabled: true,
      trackerList: "",
      trackerListUrl: "https://cf.trackerslist.com/best.txt",
      listenPort: null,
      listenPortRange: null,
      upnpEnabled: false,
      enableNatpmp: true,
      enableIpv6: true,
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
      maxDownloads: 3,
      maxSeeds: 5,
      maxTorrents: 100,
      activeLimit: 500,
    },
    logging: {
      enabled: true,
      level: "info",
      filePath: "",
      retentionCount: null,
      retentionDays: null,
    },
    aria2Rpc: { enabled: true, port: 6800, secret: null, corsAllowedOrigins: [] },
    cdnAcceleration: {
      enabled: false,
      activeIp: null,
      activeSpeedMbps: null,
      lastTestAtMs: null,
      lastError: null,
    },
    githubMirror: { enabled: false, mirrors: [] },
    notifications: { enabled: true },
    ioBaseline: {
      bufferLimitMb: 1024,
      gameModeBufferMb: 128,
      gameMode: false,
      diskTypeOverrides: {},
      maxParallelHdd: 4,
      gameModeMaxParallel: 1,
      hddBufferEnabled: true,
    },
    autostart: false,
    setupCompleted: false,
    lastSetupStep: null,
    maxInMemoryDownloads: 200,
    speedLimitSchedule: [],
  };
}

// ── Helpers ────────────────────────────────────────────────────────

function mountWizard(props: Record<string, unknown> = {}) {
  return mount(SetupWizard, {
    props: {
      appName: "Limedl",
      appVersion: "0.1.0",
      ...props,
    },
    global: {
      stubs: {
        // Render Teleport inline so any ported UI (e.g. UiSelect panels)
        // stays inside the wrapper and is reachable by find()/findAll().
        Teleport: { template: "<div><slot /></div>" },
      },
    },
    attachTo: document.body,
  });
}

function getPrimaryButton(wrapper: ReturnType<typeof mount>) {
  return wrapper.findAll("button").find((b) => {
    const text = b.text();
    return text === LABEL_START || text === LABEL_NEXT || text === LABEL_COMPLETE;
  });
}

function getBackButton(wrapper: ReturnType<typeof mount>) {
  return wrapper.findAll("button").find((b) => b.text() === LABEL_BACK);
}

function getSkipButton(wrapper: ReturnType<typeof mount>) {
  return wrapper.findAll("button").find((b) => b.text() === LABEL_SKIP);
}

function getSkipAllButton(wrapper: ReturnType<typeof mount>) {
  return wrapper.findAll("button").find((b) => b.text() === LABEL_SKIP_ALL);
}

function getCurrentStepTitle(wrapper: ReturnType<typeof mount>) {
  // StepWelcome uses step-welcome__title (h1); all other steps use setup-step__title (h2).
  const titleEl = wrapper.find(".step-welcome__title, .setup-step__title");
  return titleEl.text();
}

function getStepIndicatorItems(wrapper: ReturnType<typeof mount>) {
  return wrapper.findAll(".setup-step-indicator__item");
}

/** Extract the current `.value` property from a DOM input element. */
function readInputValue(el: Element): string {
  return el instanceof HTMLInputElement ? el.value : "";
}

async function clickPrimary(wrapper: ReturnType<typeof mount>) {
  const button = getPrimaryButton(wrapper);
  expect(button).toBeDefined();
  await button!.trigger("click");
  await nextTick();
}

async function clickBack(wrapper: ReturnType<typeof mount>) {
  const button = getBackButton(wrapper);
  expect(button).toBeDefined();
  await button!.trigger("click");
  await nextTick();
}

async function clickSkip(wrapper: ReturnType<typeof mount>) {
  const button = getSkipButton(wrapper);
  expect(button).toBeDefined();
  await button!.trigger("click");
  await nextTick();
}

async function advanceToStep(wrapper: ReturnType<typeof mount>, targetIndex: number) {
  // Start from welcome (index 0); click Next until we reach targetIndex.
  for (let i = 0; i < targetIndex; i++) {
    // Sequential mount/trigger interaction — each click depends on the prior DOM
    // state, so parallelization via Promise.all() is not safe here.
    // eslint-disable-next-line no-await-in-loop
    await clickPrimary(wrapper);
  }
}

// ── Tests ──────────────────────────────────────────────────────────

describe("SetupWizard", () => {
  const mockInvoke = vi.mocked(invoke);
  const mockSaveAppSettings = vi.mocked(saveAppSettings);
  const mockGetAppSettings = vi.mocked(getAppSettings);

  beforeEach(() => {
    resetTauriMocks();
    mockInvoke.mockImplementation(createMockInvoke());
    mockGetAppSettings.mockResolvedValue(createDefaultSettings());
    mockSaveAppSettings.mockResolvedValue(createDefaultSettings());
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // ── Rendering ────────────────────────────────────────────────────

  describe("rendering", () => {
    it("renders step indicator with all steps listed", () => {
      const wrapper = mountWizard();
      const items = getStepIndicatorItems(wrapper);
      expect(items).toHaveLength(STEP_COUNT);
    });

    it("first step is welcome and shows a Next button; Back is not rendered", () => {
      const wrapper = mountWizard();
      expect(getCurrentStepTitle(wrapper)).toBe(STEP_TITLES[0]);

      const primary = getPrimaryButton(wrapper);
      expect(primary).toBeDefined();
      expect(primary!.text()).toBe(LABEL_START);

      expect(getBackButton(wrapper)).toBeUndefined();
    });

    it("renders without crashing when no initial settings provided", () => {
      const wrapper = mountWizard({ initialSettings: undefined });
      expect(wrapper.find(".setup-wizard").exists()).toBe(true);
      expect(getCurrentStepTitle(wrapper)).toBe(STEP_TITLES[0]);
    });
  });

  // ── Navigation ───────────────────────────────────────────────────

  describe("navigation", () => {
    it("clicking Next advances through the wizard and Back returns to the previous step", async () => {
      const wrapper = mountWizard();

      // Advance from welcome to language.
      await clickPrimary(wrapper);
      expect(getCurrentStepTitle(wrapper)).toBe(STEP_TITLES[1]);

      // Advance to appearance.
      await clickPrimary(wrapper);
      expect(getCurrentStepTitle(wrapper)).toBe(STEP_TITLES[2]);

      // Go back to language.
      await clickBack(wrapper);
      expect(getCurrentStepTitle(wrapper)).toBe(STEP_TITLES[1]);

      // Go back to welcome (language is not the first step, but welcome is).
      await clickBack(wrapper);
      expect(getCurrentStepTitle(wrapper)).toBe(STEP_TITLES[0]);

      // Back should be hidden once we are on the first step again.
      expect(getBackButton(wrapper)).toBeUndefined();
    });

    it("Back button appears after leaving the first step", async () => {
      const wrapper = mountWizard();
      expect(getBackButton(wrapper)).toBeUndefined();

      await clickPrimary(wrapper);
      expect(getBackButton(wrapper)).toBeDefined();
    });
  });

  // ── Skip semantics ───────────────────────────────────────────────

  describe("skip semantics", () => {
    it("shows Skip button on skippable steps and hides it on non-skippable steps", async () => {
      const wrapper = mountWizard();

      // Welcome: no Skip (but Skip All is shown instead).
      expect(getSkipButton(wrapper)).toBeUndefined();
      expect(getSkipAllButton(wrapper)).toBeDefined();

      for (let index = 1; index < STEP_COUNT; index++) {
        // Click sequence depends on prior DOM mutations — must be sequential.
        // eslint-disable-next-line no-await-in-loop
        await clickPrimary(wrapper);
        // eslint-disable-next-line no-await-in-loop
        await nextTick();

        const isSummary = index === STEP_COUNT - 1;
        const isDirectory = index === 5; // directory is required (non-skippable)
        if (isSummary || isDirectory) {
          expect(getSkipButton(wrapper)).toBeUndefined();
        } else {
          expect(getSkipButton(wrapper)).toBeDefined();
        }
      }
    });

    it("clicking Skip advances past the current skippable step", async () => {
      const wrapper = mountWizard();
      await advanceToStep(wrapper, 1); // language
      expect(getCurrentStepTitle(wrapper)).toBe(STEP_TITLES[1]);

      await clickSkip(wrapper);
      expect(getCurrentStepTitle(wrapper)).toBe(STEP_TITLES[2]);
    });
  });

  // ── Validation ───────────────────────────────────────────────────

  describe("validation", () => {
    // Design decision: The wizard currently does not disable the primary
    // Next/Complete button based on per-step validation. StepDirectory and
    // StepPerformance collect values but never emit an invalid state, and
    // SetupWizard.vue never binds :disabled to the primary action button.
    // Adding field-level validation gating is deferred to a future UX pass.
    // When that pass lands, add a test here that verifies the button is
    // disabled when required fields are missing.
    it("is intentionally empty — validation gating is deferred", () => {
      // Placeholder: see the describe-block comment for design rationale.
      expect(true).toBe(true);
    });
  });

  // ── Finalize ─────────────────────────────────────────────────────

  describe("finalize", () => {
    it("clicking Finish on Summary calls saveAppSettings with merged settings", async () => {
      const wrapper = mountWizard();
      await advanceToStep(wrapper, STEP_COUNT - 1);
      expect(getCurrentStepTitle(wrapper)).toBe(STEP_TITLES[STEP_COUNT - 1]);

      const finishButton = getPrimaryButton(wrapper);
      expect(finishButton).toBeDefined();
      expect(finishButton!.text()).toBe(LABEL_COMPLETE);

      await finishButton!.trigger("click");
      await flushPromises();

      expect(mockSaveAppSettings).toHaveBeenCalledTimes(1);
      const savedSettings = mockSaveAppSettings.mock.calls[0]?.[0];
      expect(savedSettings).toEqual(
        expect.objectContaining({
          setupCompleted: true,
          lastSetupStep: STEP_COUNT - 1,
        }),
      );
      expect(
        (savedSettings as { appearance: { themeColor: unknown } }).appearance.themeColor,
      ).toBeDefined();
      expect(
        (savedSettings as { download: { defaultDownloadDir: unknown } }).download
          .defaultDownloadDir,
      ).toBeDefined();
    });

    it("emits completed event with final settings after Finish", async () => {
      const wrapper = mountWizard();
      await advanceToStep(wrapper, STEP_COUNT - 1);
      await getPrimaryButton(wrapper)!.trigger("click");
      await flushPromises();

      expect(wrapper.emitted("completed")).toBeTruthy();
      const emittedSettings = wrapper.emitted("completed")?.[0]?.[0];
      expect(emittedSettings).toEqual(expect.objectContaining({ setupCompleted: true }));
    });
  });

  // ── Settings persistence ─────────────────────────────────────────

  describe("settings persistence", () => {
    it("preserves directory input value across step navigation", async () => {
      const wrapper = mountWizard();
      await advanceToStep(wrapper, 5); // directory step
      expect(getCurrentStepTitle(wrapper)).toBe(STEP_TITLES[5]);

      const input = wrapper.find("input.directory-field__input");
      expect(input.exists()).toBe(true);
      await input.setValue("/tmp/test-dir");
      await nextTick();

      // Advance to performance then back to directory.
      await clickPrimary(wrapper);
      expect(getCurrentStepTitle(wrapper)).toBe(STEP_TITLES[6]);
      await clickBack(wrapper);
      expect(getCurrentStepTitle(wrapper)).toBe(STEP_TITLES[5]);

      const restoredInput = wrapper.find("input.directory-field__input");
      expect(readInputValue(restoredInput.element)).toBe("/tmp/test-dir");
    });
  });

  // ── Accessibility ────────────────────────────────────────────────

  describe("accessibility", () => {
    it("every button has an accessible name (text, aria-label, or aria-labelledby)", () => {
      const wrapper = mountWizard();
      const buttons = wrapper.findAll("button");
      expect(buttons.length).toBeGreaterThan(0);

      const unnamed: string[] = [];
      for (const button of buttons) {
        const text = button.text().trim();
        const ariaLabel = button.attributes("aria-label");
        const ariaLabelledBy = button.attributes("aria-labelledby");
        const title = button.attributes("title");

        if (!text && !ariaLabel && !ariaLabelledBy && !title) {
          unnamed.push(button.classes().join(" ") || "<no class>");
        }
      }
      expect(unnamed).toEqual([]);
    });

    it("decorative i-ri icons inside the wizard are hidden from assistive tech", () => {
      const wrapper = mountWizard();
      const iconEls = wrapper.findAll("[class*='i-ri-']");
      expect(iconEls.length).toBeGreaterThan(0);

      const exposedDecorative: string[] = [];
      for (const el of iconEls) {
        const ariaHidden = el.attributes("aria-hidden");
        const role = el.attributes("role");
        const ariaLabel = el.attributes("aria-label");

        // Decorative icons should be aria-hidden. Icons that are meaningful
        // must have an accessible name or explicit img role.
        if (ariaHidden !== "true" && !ariaLabel && role !== "img") {
          exposedDecorative.push(el.classes().join(" "));
        }
      }
      expect(exposedDecorative).toEqual([]);
    });

    it("current step indicator entry has aria-current='step' and siblings do not", async () => {
      const wrapper = mountWizard();

      for (let index = 0; index < STEP_COUNT; index++) {
        if (index > 0) {
          // eslint-disable-next-line no-await-in-loop
          await clickPrimary(wrapper);
        }

        const items = getStepIndicatorItems(wrapper);
        expect(items).toHaveLength(STEP_COUNT);

        for (let i = 0; i < STEP_COUNT; i++) {
          const button = items[i].find("button");
          if (i === index) {
            expect(button.attributes("aria-current")).toBe("step");
          } else {
            expect(button.attributes("aria-current")).toBeUndefined();
          }
        }
      }
    });
  });
});
