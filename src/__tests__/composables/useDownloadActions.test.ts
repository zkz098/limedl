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
import { createMockDownloadSnapshot, DownloadPresets, resetMockIds } from "../fixtures/downloads";
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

  // ── runPauseAll ─────────────────────────────────────────────────────

  describe("runPauseAll", () => {
    it("pauses only downloading tasks", async () => {
      const downloading1 = DownloadPresets.downloading({
        id: "task-1",
        fileName: "alpha.zip",
      });
      const paused1 = DownloadPresets.paused({ id: "task-2", fileName: "beta.zip" });
      const completed1 = DownloadPresets.completed({
        id: "task-3",
        fileName: "gamma.zip",
      });
      const downloading2 = DownloadPresets.downloading({
        id: "task-4",
        fileName: "delta.zip",
      });

      const snap1 = createMockDownloadSnapshot({ id: "task-1", state: "paused" });
      const snap2 = createMockDownloadSnapshot({ id: "task-4", state: "paused" });
      mockPauseDownload.mockResolvedValueOnce(snap1).mockResolvedValueOnce(snap2);

      input = createInput({
        downloads: ref([downloading1, paused1, completed1, downloading2]),
      });
      const actions = useDownloadActions(input);

      await actions.runPauseAll();

      expect(mockPauseDownload).toHaveBeenCalledTimes(2);
      expect(mockPauseDownload).toHaveBeenCalledWith("task-1");
      expect(mockPauseDownload).toHaveBeenCalledWith("task-4");
      expect(input.upsertSummary).toHaveBeenCalledTimes(2);
      expect(input.upsertSummary).toHaveBeenCalledWith(
        expect.objectContaining({ id: "task-1", state: "paused" }),
      );
      expect(input.upsertSummary).toHaveBeenCalledWith(
        expect.objectContaining({ id: "task-4", state: "paused" }),
      );
      expect(input.setMessage).toHaveBeenCalledWith(
        expect.stringContaining("messages.pausedAll"),
      );
      expect(input.actionName.value).toBe("");
    });

    it("does nothing when no tasks are downloading", async () => {
      input = createInput({
        downloads: ref([
          DownloadPresets.paused({ id: "task-1" }),
          DownloadPresets.completed({ id: "task-2" }),
          DownloadPresets.failed({ id: "task-3" }),
        ]),
      });
      const actions = useDownloadActions(input);

      await actions.runPauseAll();

      expect(mockPauseDownload).not.toHaveBeenCalled();
      expect(input.setMessage).toHaveBeenCalledWith(
        expect.stringContaining('"count":0'),
      );
      expect(input.upsertSummary).not.toHaveBeenCalled();
      expect(input.actionName.value).toBe("");
    });
  });

  // ── runResumeAll ────────────────────────────────────────────────────

  describe("runResumeAll", () => {
    it("resumes only paused tasks", async () => {
      const paused1 = DownloadPresets.paused({
        id: "task-1",
        fileName: "alpha.zip",
      });
      const downloading1 = DownloadPresets.downloading({
        id: "task-2",
        fileName: "beta.zip",
      });
      const paused2 = DownloadPresets.paused({
        id: "task-3",
        fileName: "gamma.zip",
      });
      const completed1 = DownloadPresets.completed({
        id: "task-4",
        fileName: "delta.zip",
      });

      const snap1 = createMockDownloadSnapshot({
        id: "task-1",
        state: "downloading",
      });
      const snap2 = createMockDownloadSnapshot({
        id: "task-3",
        state: "downloading",
      });
      mockResumeDownload.mockResolvedValueOnce(snap1).mockResolvedValueOnce(snap2);

      input = createInput({
        downloads: ref([paused1, downloading1, paused2, completed1]),
      });
      const actions = useDownloadActions(input);

      await actions.runResumeAll();

      expect(mockResumeDownload).toHaveBeenCalledTimes(2);
      expect(mockResumeDownload).toHaveBeenCalledWith("task-1");
      expect(mockResumeDownload).toHaveBeenCalledWith("task-3");
      expect(input.upsertSummary).toHaveBeenCalledTimes(2);
      expect(input.upsertSummary).toHaveBeenCalledWith(
        expect.objectContaining({ id: "task-1", state: "downloading" }),
      );
      expect(input.upsertSummary).toHaveBeenCalledWith(
        expect.objectContaining({ id: "task-3", state: "downloading" }),
      );
      expect(input.setMessage).toHaveBeenCalledWith(
        expect.stringContaining("messages.resumedAll"),
      );
      expect(input.actionName.value).toBe("");
    });

    it("does nothing when no tasks are paused", async () => {
      input = createInput({
        downloads: ref([
          DownloadPresets.downloading({ id: "task-1" }),
          DownloadPresets.completed({ id: "task-2" }),
          DownloadPresets.queued({ id: "task-3" }),
        ]),
      });
      const actions = useDownloadActions(input);

      await actions.runResumeAll();

      expect(mockResumeDownload).not.toHaveBeenCalled();
      expect(input.setMessage).toHaveBeenCalledWith(
        expect.stringContaining('"count":0'),
      );
      expect(input.upsertSummary).not.toHaveBeenCalled();
      expect(input.actionName.value).toBe("");
    });
  });

  // ── runClearCompleted ───────────────────────────────────────────────

  describe("runClearCompleted", () => {
    it("clears only completed tasks", async () => {
      const completed1 = DownloadPresets.completed({
        id: "task-1",
        fileName: "alpha.zip",
      });
      const downloading1 = DownloadPresets.downloading({
        id: "task-2",
        fileName: "beta.zip",
      });
      const completed2 = DownloadPresets.completed({
        id: "task-3",
        fileName: "gamma.zip",
      });
      const paused1 = DownloadPresets.paused({
        id: "task-4",
        fileName: "delta.zip",
      });

      const snap1 = createMockDownloadSnapshot({ id: "task-1" });
      const snap2 = createMockDownloadSnapshot({ id: "task-3" });
      mockRemoveDownload.mockResolvedValueOnce(snap1).mockResolvedValueOnce(snap2);

      input = createInput({
        downloads: ref([completed1, downloading1, completed2, paused1]),
      });
      const actions = useDownloadActions(input);

      await actions.runClearCompleted();

      expect(mockRemoveDownload).toHaveBeenCalledTimes(2);
      expect(mockRemoveDownload).toHaveBeenCalledWith("task-1");
      expect(mockRemoveDownload).toHaveBeenCalledWith("task-3");
      expect(input.removeSummary).toHaveBeenCalledTimes(2);
      expect(input.removeSummary).toHaveBeenCalledWith("task-1");
      expect(input.removeSummary).toHaveBeenCalledWith("task-3");
      expect(input.setMessage).toHaveBeenCalledWith(
        expect.stringContaining("messages.clearedCompleted"),
      );
      expect(input.actionName.value).toBe("");
    });

    it("does nothing when no tasks are completed", async () => {
      input = createInput({
        downloads: ref([
          DownloadPresets.downloading({ id: "task-1" }),
          DownloadPresets.paused({ id: "task-2" }),
          DownloadPresets.failed({ id: "task-3" }),
        ]),
      });
      const actions = useDownloadActions(input);

      await actions.runClearCompleted();

      expect(mockRemoveDownload).not.toHaveBeenCalled();
      expect(input.setMessage).toHaveBeenCalledWith(
        expect.stringContaining('"count":0'),
      );
      expect(input.removeSummary).not.toHaveBeenCalled();
      expect(input.actionName.value).toBe("");
    });
  });

  // ── runBatchDelete ──────────────────────────────────────────────────

  describe("runBatchDelete", () => {
    it("removes all specified tasks", async () => {
      const snap1 = createMockDownloadSnapshot({ id: "task-1" });
      const snap2 = createMockDownloadSnapshot({ id: "task-2" });
      mockRemoveDownload.mockResolvedValueOnce(snap1).mockResolvedValueOnce(snap2);

      input = createInput({
        downloads: ref([
          DownloadPresets.downloading({ id: "task-1", fileName: "alpha.zip" }),
          DownloadPresets.paused({ id: "task-2", fileName: "beta.zip" }),
          DownloadPresets.completed({ id: "task-3", fileName: "gamma.zip" }),
        ]),
      });
      const actions = useDownloadActions(input);

      await actions.runBatchDelete(["task-1", "task-2"]);

      expect(mockRemoveDownload).toHaveBeenCalledTimes(2);
      expect(mockRemoveDownload).toHaveBeenCalledWith("task-1");
      expect(mockRemoveDownload).toHaveBeenCalledWith("task-2");
      expect(input.removeSummary).toHaveBeenCalledTimes(2);
      expect(input.removeSummary).toHaveBeenCalledWith("task-1");
      expect(input.removeSummary).toHaveBeenCalledWith("task-2");
      expect(input.setMessage).toHaveBeenCalledWith(
        expect.stringContaining("messages.batchDeleted"),
      );
      expect(input.actionName.value).toBe("");
    });

    it("falls back to id as fileName when task not in list", async () => {
      mockRemoveDownload.mockRejectedValue(new Error("Not found"));

      input = createInput({
        downloads: ref([DownloadPresets.downloading({ id: "task-1" })]),
      });
      const actions = useDownloadActions(input);

      await actions.runBatchDelete(["unknown-id"]);

      expect(mockRemoveDownload).toHaveBeenCalledWith("unknown-id");
      expect(input.setError).toHaveBeenCalledWith(
        expect.stringContaining("unknown-id: Not found"),
      );
      expect(input.actionName.value).toBe("");
    });

    it("returns early when given an empty array", async () => {
      input = createInput();
      const actions = useDownloadActions(input);

      await actions.runBatchDelete([]);

      expect(mockRemoveDownload).not.toHaveBeenCalled();
      expect(input.removeSummary).not.toHaveBeenCalled();
      expect(input.setMessage).not.toHaveBeenCalled();
      expect(input.setError).not.toHaveBeenCalled();
      expect(input.actionName.value).toBe("");
    });
  });

  // ── runCopyLink ─────────────────────────────────────────────────────

  describe("runCopyLink", () => {
    let mockClipboardWriteText: ReturnType<typeof vi.fn>;

    beforeEach(() => {
      mockClipboardWriteText = vi.fn().mockResolvedValue(undefined);
      vi.stubGlobal("navigator", {
        ...navigator,
        clipboard: { writeText: mockClipboardWriteText },
      });
    });

    afterEach(() => {
      vi.unstubAllGlobals();
    });

    it("copies link from selectedSnapshot when ids match", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        url: "https://example.com/file.zip",
      });

      input = createInput({
        selectedSnapshot: ref(snapshot),
      });
      const actions = useDownloadActions(input);

      await actions.runCopyLink("task-1");

      expect(mockClipboardWriteText).toHaveBeenCalledWith(
        "https://example.com/file.zip",
      );
      expect(input.setMessage).toHaveBeenCalledWith(
        expect.stringContaining("messages.linkCopied"),
      );
    });

    it("copies link from downloads list when selectedSnapshot ids do not match", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-999",
        url: "https://example.com/other.zip",
      });
      const task = DownloadPresets.downloading({
        id: "task-1",
        url: "https://example.com/myfile.zip",
      });

      input = createInput({
        selectedSnapshot: ref(snapshot),
        downloads: ref([task]),
      });
      const actions = useDownloadActions(input);

      await actions.runCopyLink("task-1");

      expect(mockClipboardWriteText).toHaveBeenCalledWith(
        "https://example.com/myfile.zip",
      );
      expect(input.setMessage).toHaveBeenCalledWith(
        expect.stringContaining("messages.linkCopied"),
      );
    });

    it("sets error when target has no url", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        url: "",
      });

      input = createInput({
        selectedSnapshot: ref(snapshot),
      });
      const actions = useDownloadActions(input);

      await actions.runCopyLink("task-1");

      expect(mockClipboardWriteText).not.toHaveBeenCalled();
      expect(input.setError).toHaveBeenCalledWith(
        expect.stringContaining("messages.copyLinkFailed"),
      );
    });

    it("sets error when clipboard write fails", async () => {
      const snapshot = createMockDownloadSnapshot({
        id: "task-1",
        url: "https://example.com/file.zip",
      });
      mockClipboardWriteText.mockRejectedValue(
        new Error("Clipboard access denied"),
      );

      input = createInput({
        selectedSnapshot: ref(snapshot),
      });
      const actions = useDownloadActions(input);

      await actions.runCopyLink("task-1");

      expect(mockClipboardWriteText).toHaveBeenCalledWith(
        "https://example.com/file.zip",
      );
      expect(input.setError).toHaveBeenCalledWith("Clipboard access denied");
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
