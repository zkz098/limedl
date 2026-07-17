import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

import { open } from "@tauri-apps/plugin-dialog";
import { pickDirectory, pickTorrentFile } from "../../lib/tauri/dialog-api";

const mockOpen = vi.mocked(open);

beforeEach(() => {
  vi.clearAllMocks();
});

describe("dialog-api", () => {
  describe("pickDirectory", () => {
    it("calls open with directory=true and multiple=false", async () => {
      mockOpen.mockResolvedValue("/chosen/directory");

      await pickDirectory();

      expect(mockOpen).toHaveBeenCalledWith({
        directory: true,
        multiple: false,
        title: "Choose destination folder",
      });
    });

    it("returns string result directly when result is a string", async () => {
      mockOpen.mockResolvedValue("/chosen/path");

      const result = await pickDirectory();

      expect(result).toBe("/chosen/path");
    });

    it("returns null when result is null", async () => {
      mockOpen.mockResolvedValue(null);

      const result = await pickDirectory();

      expect(result).toBeNull();
    });

    it("returns first element when result is a single-element array", async () => {
      mockOpen.mockResolvedValue(["/array/path"]);

      const result = await pickDirectory();

      expect(result).toBe("/array/path");
    });

    it("returns first element when result is a multi-element array", async () => {
      mockOpen.mockResolvedValue(["/first/path", "/second/path"]);

      const result = await pickDirectory();

      expect(result).toBe("/first/path");
    });

    it("returns null when result is an empty array", async () => {
      mockOpen.mockResolvedValue([]);

      const result = await pickDirectory();

      expect(result).toBeNull();
    });
  });

  describe("pickTorrentFile", () => {
    it("calls open with directory=false, torrent filter, and multiple=false", async () => {
      mockOpen.mockResolvedValue("/path/to/file.torrent");

      await pickTorrentFile();

      expect(mockOpen).toHaveBeenCalledWith({
        directory: false,
        multiple: false,
        title: "Choose torrent file",
        filters: [{ name: "Torrent", extensions: ["torrent"] }],
      });
    });

    it("returns first element from array result", async () => {
      mockOpen.mockResolvedValue(["/path/to/file.torrent"]);

      const result = await pickTorrentFile();

      expect(result).toBe("/path/to/file.torrent");
    });

    it("returns string result directly when result is a string", async () => {
      mockOpen.mockResolvedValue("/direct/path.torrent");

      const result = await pickTorrentFile();

      expect(result).toBe("/direct/path.torrent");
    });

    it("returns null when result is null", async () => {
      mockOpen.mockResolvedValue(null);

      const result = await pickTorrentFile();

      expect(result).toBeNull();
    });

    it("returns null when result is an empty array", async () => {
      mockOpen.mockResolvedValue([]);

      const result = await pickTorrentFile();

      expect(result).toBeNull();
    });
  });
});
