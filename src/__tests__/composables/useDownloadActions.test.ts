import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { ref, computed } from "vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// Mock the i18n module so `t()` returns predictable keys
vi.mock("../../i18n", () => ({
  t: vi.fn((key: string, options?: Record<string, unknown>) => {
    if (options) {
      const serialized = JSON.stringify(options);
      return `${key} ${serialized}`;
    }
    return key;
  }),
}));

// Mock the download-api module so no Tauri calls are made
vi.mock("../../lib/tauri/download-api", () => ({
  cancelDownload: vi.fn(),
  openDownloadInExplorer: vi.fn(),
  pauseDownload: vi.fn(),
  purgeDownload: vi.fn(),
  removeDownload: vi.fn(),
  resumeDownload: vi.fn(),
}));

import { resetTauriMocks } from "../mocks/tauri-mock";
import {
  useDownloadActions,
  type UseDownloadActionsInput,
} from "../../composables/useDownloadActions";
import { createMockDownloadSnapshot, resetMockIds } from "../fixtures/downloads";
import type { DownloadSnapshot, DownloadSummary } from "../../types/download";

// Get the mocked API functions
import {
  cancelDownload,
  openDownloadInExplorer,
  pauseDownload,
  purgeDownload,
  removeDownload,
  resumeDownload,
} from "../../lib/tauri/download-api";

const mockCancelDownload = vi.mocked(cancelDownload);
const mockOpenDownloadInExplorer = vi.mocked(openDownloadInExplorer);
const mockPauseDownload = vi.mocked(pauseDownload);
const mockPurgeDownload = vi.mocked(purgeDownload);
const mockRemoveDownload = vi.mocked(removeDownload);
const mockResumeDownload = vi.mocked(resumeDownload);

// ── Helpers ───────────────────────────────────────────────────────────────

function createInput(overrides?: Partial<UseDownloadActionsInput>) {
  return {
    downloads: ref<DownloadSummary[]>([]),
    selectedId: ref<string | null>(null),
    selectedSnapshot: ref<DownloadSnapshot | null>(null),
    actionName: ref(""),
    allowAutoSelect: ref(true),
    selectedSummary: computed(() => null),
    selectedDownload: computed(() => null),
    canPause: computed(() => true),
    canResume: computed(() => true),
    canCancel: computed(() => true),
    upsertSummary: vi.fn(),
    removeSummary: vi.fn(),
    refreshStatus: vi.fn().mockResolvedValue(undefined),
    setMessage: vi.fn(),
    setError: vi.fn(),
    clearMessage: vi.fn(),
    ...overrides,
  };
}

// ── Tests ─────────────────────────────────────────────────────────────────

describe("useDownloadActions", () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let input: any;

  beforeEach(() => {
    resetTauriMocks();
    resetMockIds();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // ── selectDownload ──────────────────────────────────────────────────────

  describe("selectDownload", () => {
    it("sets allowAutoSelect and selectedId, calls refreshStatus when given an id", async () => {
      input = createInput();
      const actions = useDownloadActions(input);

      await actions.selectDownload("task-1");

      expect(input.allowAutoSelect.value).toBe(true);
      expect(input.selectedId.value).toBe("task-1");
      expect(input.refreshStatus).toHaveBeenCalledWith("task-1", { silent: true });
    });

    it("clears selection and sets selectedSnapshot to null when given null", async () => {
      const snapshot = createMockDownloadSnapshot({ id: "task-1" });
      input = createInput({
        selectedId: ref("task-1"),
        selectedSnapshot: ref(snapshot),
      });
      const actions = useDownloadActions(input);

      await actions.selectDownload(null);

      expect(input.allowAutoSelect.value).toBe(false);
      expect(input.selectedId.value).toBeNull();
      expect(input.selectedSnapshot.value).toBeNull();
    });
  });

  // ── runPause ────────────────────────────────────────────────────────────

  describe("runPause", () => {
    it("calls pauseDownload with selectedId, upserts summary, and shows message", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        state: "paused",
        fileName: "test.zip",
      });
      mockPauseDownload.mockResolvedValue(snapshot);
      input = createInput({ selectedId: ref("task-1") });
      const actions = useDownloadActions(input);

      await actions.runPause();

      expect(mockPauseDownload).toHaveBeenCalledWith("task-1");
      expect(input.upsertSummary).toHaveBeenCalledTimes(1);
      expect(input.setMessage).toHaveBeenCalledTimes(1);
    });

    it("is a no-op when selectedId is null", async () => {
      input = createInput({ selectedId: ref(null) });
      const actions = useDownloadActions(input);

      await actions.runPause();

      expect(mockPauseDownload).not.toHaveBeenCalled();
      expect(input.upsertSummary).not.toHaveBeenCalled();
      expect(input.setMessage).not.toHaveBeenCalled();
      expect(input.clearMessage).not.toHaveBeenCalled();
    });
  });

  // ── runResume ───────────────────────────────────────────────────────────

  describe("runResume", () => {
    it("calls resumeDownload with selectedId, upserts summary, and shows message", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        state: "downloading",
        fileName: "test.zip",
      });
      mockResumeDownload.mockResolvedValue(snapshot);
      input = createInput({ selectedId: ref("task-1") });
      const actions = useDownloadActions(input);

      await actions.runResume();

      expect(mockResumeDownload).toHaveBeenCalledWith("task-1");
      expect(input.upsertSummary).toHaveBeenCalledTimes(1);
      expect(input.setMessage).toHaveBeenCalledTimes(1);
    });

    it("is a no-op when selectedId is null", async () => {
      input = createInput({ selectedId: ref(null) });
      const actions = useDownloadActions(input);

      await actions.runResume();

      expect(mockResumeDownload).not.toHaveBeenCalled();
    });
  });

  // ── runCancel ───────────────────────────────────────────────────────────

  describe("runCancel", () => {
    it("calls cancelDownload with selectedId, removes from summary, and shows message", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        fileName: "test.zip",
      });
      mockCancelDownload.mockResolvedValue(snapshot);
      input = createInput({ selectedId: ref("task-1") });
      const actions = useDownloadActions(input);

      await actions.runCancel();

      expect(mockCancelDownload).toHaveBeenCalledWith("task-1");
      expect(input.removeSummary).toHaveBeenCalledWith("task-1");
      expect(input.setMessage).toHaveBeenCalledTimes(1);
    });
  });

  // ── runDeleteTask (remove) ──────────────────────────────────────────────

  describe("runDeleteTask", () => {
    it("calls removeDownload with the given id, removes from summary, and shows message", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        fileName: "test.zip",
      });
      mockRemoveDownload.mockResolvedValue(snapshot);
      input = createInput();
      const actions = useDownloadActions(input);

      await actions.runDeleteTask("task-1");

      expect(mockRemoveDownload).toHaveBeenCalledWith("task-1");
      expect(input.removeSummary).toHaveBeenCalledWith("task-1");
      expect(input.setMessage).toHaveBeenCalledTimes(1);
    });
  });

  // ── runDeleteTaskPermanently (purge) ────────────────────────────────────

  describe("runDeleteTaskPermanently", () => {
    it("calls purgeDownload with the given id, removes from summary, and shows message", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        fileName: "test.zip",
      });
      mockPurgeDownload.mockResolvedValue(snapshot);
      input = createInput();
      const actions = useDownloadActions(input);

      await actions.runDeleteTaskPermanently("task-1");

      expect(mockPurgeDownload).toHaveBeenCalledWith("task-1");
      expect(input.removeSummary).toHaveBeenCalledWith("task-1");
      expect(input.setMessage).toHaveBeenCalledTimes(1);
    });
  });

  // ── runOpenInExplorer ───────────────────────────────────────────────────

  describe("runOpenInExplorer", () => {
    it("calls openDownloadInExplorer with the given id and shows a message", async () => {
      mockOpenDownloadInExplorer.mockResolvedValue(undefined);
      input = createInput();
      const actions = useDownloadActions(input);

      await actions.runOpenInExplorer("task-1");

      expect(mockOpenDownloadInExplorer).toHaveBeenCalledWith("task-1");
      expect(input.setMessage).toHaveBeenCalledTimes(1);
    });
  });

  // ── Error handling ──────────────────────────────────────────────────────

  describe("error handling", () => {
    it("calls setError when pauseDownload throws", async () => {
      mockPauseDownload.mockRejectedValue(new Error("Backend unavailable"));
      input = createInput({ selectedId: ref("task-1") });
      const actions = useDownloadActions(input);

      await actions.runPause();

      expect(input.setError).toHaveBeenCalledWith("Backend unavailable");
    });

    it("calls setError when resumeDownload throws", async () => {
      mockResumeDownload.mockRejectedValue(new Error("Backend unavailable"));
      input = createInput({ selectedId: ref("task-1") });
      const actions = useDownloadActions(input);

      await actions.runResume();

      expect(input.setError).toHaveBeenCalledWith("Backend unavailable");
    });

    it("calls setError when cancelDownload throws", async () => {
      mockCancelDownload.mockRejectedValue(new Error("Backend unavailable"));
      input = createInput({ selectedId: ref("task-1") });
      const actions = useDownloadActions(input);

      await actions.runCancel();

      expect(input.setError).toHaveBeenCalledWith("Backend unavailable");
    });

    it("calls setError when openDownloadInExplorer throws", async () => {
      mockOpenDownloadInExplorer.mockRejectedValue(new Error("Explorer error"));
      input = createInput();
      const actions = useDownloadActions(input);

      await actions.runOpenInExplorer("task-1");

      expect(input.setError).toHaveBeenCalledWith("Explorer error");
    });

    it("resets actionName to empty after pause error", async () => {
      mockPauseDownload.mockRejectedValue(new Error("Backend unavailable"));
      input = createInput({ selectedId: ref("task-1") });
      const actions = useDownloadActions(input);

      await actions.runPause();

      expect(input.actionName.value).toBe("");
    });

    it("resets actionName to empty after resume error", async () => {
      mockResumeDownload.mockRejectedValue(new Error("Backend unavailable"));
      input = createInput({ selectedId: ref("task-1") });
      const actions = useDownloadActions(input);

      await actions.runResume();

      expect(input.actionName.value).toBe("");
    });

    it("resets actionName to empty after successful action", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        state: "paused",
        fileName: "test.zip",
      });
      mockPauseDownload.mockResolvedValue(snapshot);
      input = createInput({ selectedId: ref("task-1") });
      const actions = useDownloadActions(input);

      await actions.runPause();

      expect(input.actionName.value).toBe("");
    });

    it("converts non-Error throws to string messages", async () => {
      mockPauseDownload.mockRejectedValue("String error");
      input = createInput({ selectedId: ref("task-1") });
      const actions = useDownloadActions(input);

      await actions.runPause();

      expect(input.setError).toHaveBeenCalledWith("String error");
    });
  });

  // ── actionName lifecycle ────────────────────────────────────────────────

  describe("actionName lifecycle", () => {
    it("sets actionName during pause and resets after", async () => {
      let resolvePromise!: (snapshot: DownloadSnapshot) => void;
      const pendingPromise = new Promise<DownloadSnapshot>((resolve) => {
        resolvePromise = resolve;
      });
      mockPauseDownload.mockReturnValue(pendingPromise);
      input = createInput({ selectedId: ref("task-1") });
      const actions = useDownloadActions(input);

      const promise = actions.runPause();

      // During the action
      expect(input.actionName.value).toBe("Pause");

      resolvePromise(createMockDownloadSnapshot({ id: "task-1" }));
      await promise;

      // After the action
      expect(input.actionName.value).toBe("");
    });

    it("sets actionName during deleteTask and resets after", async () => {
      let resolvePromise!: (snapshot: DownloadSnapshot) => void;
      const pendingPromise = new Promise<DownloadSnapshot>((resolve) => {
        resolvePromise = resolve;
      });
      mockRemoveDownload.mockReturnValue(pendingPromise);
      input = createInput();
      const actions = useDownloadActions(input);

      const promise = actions.runDeleteTask("task-1");

      expect(input.actionName.value).toBe("Delete");

      resolvePromise(createMockDownloadSnapshot({ id: "task-1" }));
      await promise;

      expect(input.actionName.value).toBe("");
    });
  });

  // ── Returned values ─────────────────────────────────────────────────────

  describe("returned values", () => {
    it("exposes key refs and computeds from input", () => {
      input = createInput({
        selectedId: ref("abc-123"),
        allowAutoSelect: ref(false),
      });
      const actions = useDownloadActions(input);

      expect(actions.selectedId.value).toBe("abc-123");
      expect(actions.allowAutoSelect.value).toBe(false);
      expect(actions.selectedSnapshot.value).toBeNull();
      expect(actions.selectedSummary.value).toBeNull();
      expect(actions.selectedDownload.value).toBeNull();
    });
  });
});
