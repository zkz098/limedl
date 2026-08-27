import { test, expect } from "vitest";
import os from "node:os";
import path from "node:path";
import { promises as fs } from "node:fs";
import { spawnSync } from "node:child_process";

import {
  detectArch,
  isWaylandLibraryName,
  parseArgs,
  patchGdkBackendHook,
  pruneAppImageWaylandLibraries,
  removeWaylandLibraries,
} from "./prune-appimage-wayland-libs.mjs";

async function withTempDir(run) {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "appimage-wayland-prune-"));
  try {
    await run(tempDir);
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }
}

async function writeFile(filePath, content = "") {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, content, "utf8");
}

test("matches only bundled Wayland library names", () => {
  expect(isWaylandLibraryName("libwayland-client.so.0")).toBe(true);
  expect(isWaylandLibraryName("libwayland-egl.so.1")).toBe(true);
  expect(isWaylandLibraryName("libwayland-cursor.so.0.22.0")).toBe(true);
  expect(isWaylandLibraryName("libEGL_mesa.so.0")).toBe(false);
  expect(isWaylandLibraryName("libxkbcommon.so.0")).toBe(false);
  expect(isWaylandLibraryName("wayland-protocols")).toBe(false);
});

test("removeWaylandLibraries prunes only usr/lib/libwayland-*", async () => {
  await withTempDir(async (appDir) => {
    const libDir = path.join(appDir, "usr", "lib");
    await writeFile(path.join(libDir, "libwayland-client.so.0"), "remove");
    await writeFile(path.join(libDir, "libwayland-egl.so.1"), "remove");
    await writeFile(path.join(libDir, "libEGL_mesa.so.0"), "keep");
    await writeFile(path.join(appDir, "usr", "share", "libwayland-note.txt"), "keep");

    const removed = await removeWaylandLibraries(appDir);

    expect(removed.map((entry) => path.basename(entry))).toEqual([
      "libwayland-client.so.0",
      "libwayland-egl.so.1",
    ]);
    await expect(fs.stat(path.join(libDir, "libwayland-client.so.0"))).rejects.toThrow();
    expect(await fs.readFile(path.join(libDir, "libEGL_mesa.so.0"), "utf8")).toBe("keep");
    expect(
      await fs.readFile(path.join(appDir, "usr", "share", "libwayland-note.txt"), "utf8"),
    ).toBe("keep");
  });
});

test("patchGdkBackendHook removes the GDK_BACKEND=x11 export from the AppRun hook", async () => {
  await withTempDir(async (appDir) => {
    const hookPath = path.join(appDir, "apprun-hooks", "linuxdeploy-plugin-gtk.sh");
    await writeFile(
      hookPath,
      [
        "#!/bin/sh",
        "",
        "export GDK_BACKEND=x11 # Crash with Wayland backend on Wayland - We tested it without it",
        'export GTK_PATH="${APPDIR}/usr/lib/gtk-3.0"',
        'export GSETTINGS_SCHEMA_DIR="${APPDIR}/usr/share/glib-2.0/schemas"',
      ].join("\n"),
    );

    const patched = await patchGdkBackendHook(appDir);

    expect(patched).toBe(true);
    const content = await fs.readFile(hookPath, "utf8");
    expect(content).not.toMatch(/GDK_BACKEND/);
    // Unrelated exports must survive untouched.
    expect(content).toMatch(/export GTK_PATH=/);
    expect(content).toMatch(/export GSETTINGS_SCHEMA_DIR=/);
  });
});

test("patchGdkBackendHook is a no-op when the hook is absent or has no GDK_BACKEND line", async () => {
  await withTempDir(async (appDir) => {
    expect(await patchGdkBackendHook(appDir)).toBe(false);

    await writeFile(
      path.join(appDir, "apprun-hooks", "linuxdeploy-plugin-gtk.sh"),
      'export GTK_PATH="${APPDIR}/usr/lib/gtk-3.0"\n',
    );
    expect(await patchGdkBackendHook(appDir)).toBe(false);
  });
});

test("pruneAppImageWaylandLibraries reports patchedGdkBackend on success", async () => {
  await withTempDir(async (root) => {
    const appImagePath = path.join(root, "limedl_0.1.8_amd64.AppImage");
    const workingRoot = path.join(root, "work");
    await fs.mkdir(workingRoot);
    await writeFile(appImagePath, "original-appimage");

    const commandRunner = async (command, args, options) => {
      if (command === appImagePath && args[0] === "--appimage-extract") {
        const appDir = path.join(options.cwd, "squashfs-root");
        await writeFile(path.join(appDir, "usr", "lib", "libwayland-client.so.0"), "remove");
        await writeFile(
          path.join(appDir, "apprun-hooks", "linuxdeploy-plugin-gtk.sh"),
          "export GDK_BACKEND=x11\n",
        );
        return;
      }
      if (command === "appimagetool") {
        return;
      }
      throw new Error(`unexpected command: ${command}`);
    };

    const result = await pruneAppImageWaylandLibraries({
      appImagePath,
      appImageToolPath: "appimagetool",
      commandRunner,
      workingRoot,
    });

    expect(result.patchedGdkBackend).toBe(true);
    expect(result.removedLibraries).toEqual(["usr/lib/libwayland-client.so.0"]);
  });
});

test("parseArgs fails fast for missing appimage value", () => {
  expect(() => parseArgs(["--appimage", "--appimagetool", "tool"])).toThrow(
    /Missing value for --appimage/,
  );
  expect(() => parseArgs([])).toThrow(/Provide --appimage <path>/);
});

test("detectArch derives AppImage arch from filename", () => {
  expect(detectArch("limedl_0.1.8_amd64.AppImage")).toBe("x86_64");
  expect(detectArch("limedl_0.1.8_x86_64.AppImage")).toBe("x86_64");
  expect(detectArch("limedl_0.1.8_aarch64.AppImage")).toBe("aarch64");
  expect(detectArch("limedl_0.1.8_arm64.AppImage")).toBe("aarch64");
});

test("pruneAppImageWaylandLibraries restores original AppImage when repack fails", async () => {
  await withTempDir(async (root) => {
    const appImagePath = path.join(root, "limedl_0.1.8_amd64.AppImage");
    const workingRoot = path.join(root, "work");
    await fs.mkdir(workingRoot);
    await writeFile(appImagePath, "original-appimage");

    const commandRunner = async (command, args, options) => {
      if (command === appImagePath && args[0] === "--appimage-extract") {
        const appDir = path.join(options.cwd, "squashfs-root");
        await writeFile(path.join(appDir, "usr", "lib", "libwayland-client.so.0"), "remove");
        return;
      }
      if (command === "appimagetool") {
        throw new Error("synthetic repack failure");
      }
      throw new Error(`unexpected command: ${command}`);
    };

    await expect(
      pruneAppImageWaylandLibraries({
        appImagePath,
        appImageToolPath: "appimagetool",
        commandRunner,
        workingRoot,
      }),
    ).rejects.toThrow(/synthetic repack failure/);

    expect(await fs.readFile(appImagePath, "utf8")).toBe("original-appimage");
  });
});

test("pruneAppImageWaylandLibraries reports appimagetool start failures clearly", async () => {
  await withTempDir(async (root) => {
    const appImagePath = path.join(root, "limedl_0.1.8_amd64.AppImage");
    const workingRoot = path.join(root, "work");
    await fs.mkdir(workingRoot);
    await writeFile(appImagePath, "original-appimage");

    const commandRunner = async (command, args, options) => {
      if (command === appImagePath && args[0] === "--appimage-extract") {
        const appDir = path.join(options.cwd, "squashfs-root");
        await writeFile(path.join(appDir, "usr", "lib", "libwayland-client.so.0"), "remove");
        return;
      }
      const error = new Error("spawn appimagetool ENOENT");
      error.code = "ENOENT";
      throw new Error(`${command} ${args.join(" ")} failed to start: ${error.message}`);
    };

    await expect(
      pruneAppImageWaylandLibraries({
        appImagePath,
        appImageToolPath: "appimagetool",
        commandRunner,
        workingRoot,
      }),
    ).rejects.toThrow(/failed to start: spawn appimagetool ENOENT/);
    expect(await fs.readFile(appImagePath, "utf8")).toBe("original-appimage");
  });
});

test("cli reports missing appimage argument", () => {
  const result = spawnSync(
    process.execPath,
    ["scripts/prune-appimage-wayland-libs.mjs", "--appimage"],
    {
      cwd: process.cwd(),
      encoding: "utf8",
    },
  );

  expect(result.status).not.toBe(0);
  expect(`${result.stdout}\n${result.stderr}`).toMatch(/Missing value for --appimage/);
});
