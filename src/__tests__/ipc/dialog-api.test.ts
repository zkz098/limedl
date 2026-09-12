import { describe, it, expect, vi, beforeEach } from "vitest";

// The WebUI cannot open a native picker (see src/lib/platform/dialog.ts), so
// the platform boundary is mocked here to exercise the result normalization in
// dialog-api itself.
vi.mock("../../lib/platform/dialog", () => ({ openDialog: vi.fn() }));

import { openDialog } from "../../lib/platform/dialog";
import { pickDirectory, pickTorrentFile } from "../../lib/ipc/dialog-api";

const mockOpenDialog = vi.mocked(openDialog);

beforeEach(() => {
  vi.clearAllMocks();
});

describe("dialog-api", () => {
  describe("pickDirectory", () => {
    it("asks for a single directory", async () => {
      mockOpenDialog.mockResolvedValue("/chosen/directory");

      await pickDirectory();

      expect(mockOpenDialog).toHaveBeenCalledWith({
        directory: true,
        multiple: false,
        title: "Choose destination folder",
      });
    });

    it("returns string result directly when result is a string", async () => {
      mockOpenDialog.mockResolvedValue("/chosen/path");

      const result = await pickDirectory();

      expect(result).toBe("/chosen/path");
    });

    it("returns null when result is null", async () => {
      mockOpenDialog.mockResolvedValue(null);

      const result = await pickDirectory();

      expect(result).toBeNull();
    });

    it("returns first element when result is a single-element array", async () => {
      mockOpenDialog.mockResolvedValue(["/array/path"]);

      const result = await pickDirectory();

      expect(result).toBe("/array/path");
    });

    it("returns first element when result is a multi-element array", async () => {
      mockOpenDialog.mockResolvedValue(["/first/path", "/second/path"]);

      const result = await pickDirectory();

      expect(result).toBe("/first/path");
    });

    it("returns null when result is an empty array", async () => {
      mockOpenDialog.mockResolvedValue([]);

      const result = await pickDirectory();

      expect(result).toBeNull();
    });
  });

  describe("pickTorrentFile", () => {
    it("asks for a single .torrent file", async () => {
      mockOpenDialog.mockResolvedValue("/path/to/file.torrent");

      await pickTorrentFile();

      expect(mockOpenDialog).toHaveBeenCalledWith({
        directory: false,
        multiple: false,
        title: "Choose torrent file",
        filters: [{ name: "Torrent", extensions: ["torrent"] }],
      });
    });

    it("returns first element from array result", async () => {
      mockOpenDialog.mockResolvedValue(["/path/to/file.torrent"]);

      const result = await pickTorrentFile();

      expect(result).toBe("/path/to/file.torrent");
    });

    it("returns string result directly when result is a string", async () => {
      mockOpenDialog.mockResolvedValue("/direct/path.torrent");

      const result = await pickTorrentFile();

      expect(result).toBe("/direct/path.torrent");
    });

    it("returns null when result is null", async () => {
      mockOpenDialog.mockResolvedValue(null);

      const result = await pickTorrentFile();

      expect(result).toBeNull();
    });

    it("returns null when result is an empty array", async () => {
      mockOpenDialog.mockResolvedValue([]);

      const result = await pickTorrentFile();

      expect(result).toBeNull();
    });
  });
});
