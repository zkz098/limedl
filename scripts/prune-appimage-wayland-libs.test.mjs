import test from "node:test";
import assert from "node:assert/strict";
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
  assert.equal(isWaylandLibraryName("libwayland-client.so.0"), true);
  assert.equal(isWaylandLibraryName("libwayland-egl.so.1"), true);
  assert.equal(isWaylandLibraryName("libwayland-cursor.so.0.22.0"), true);
  assert.equal(isWaylandLibraryName("libEGL_mesa.so.0"), false);
  assert.equal(isWaylandLibraryName("libxkbcommon.so.0"), false);
  assert.equal(isWaylandLibraryName("wayland-protocols"), false);
});

test("removeWaylandLibraries prunes only usr/lib/libwayland-*", async () => {
  await withTempDir(async (appDir) => {
    const libDir = path.join(appDir, "usr", "lib");
    await writeFile(path.join(libDir, "libwayland-client.so.0"), "remove");
    await writeFile(path.join(libDir, "libwayland-egl.so.1"), "remove");
    await writeFile(path.join(libDir, "libEGL_mesa.so.0"), "keep");
    await writeFile(path.join(appDir, "usr", "share", "libwayland-note.txt"), "keep");

    const removed = await removeWaylandLibraries(appDir);

    assert.deepEqual(
      removed.map((entry) => path.basename(entry)),
      ["libwayland-client.so.0", "libwayland-egl.so.1"],
    );
    await assert.rejects(() => fs.stat(path.join(libDir, "libwayland-client.so.0")));
    assert.equal(await fs.readFile(path.join(libDir, "libEGL_mesa.so.0"), "utf8"), "keep");
    assert.equal(await fs.readFile(path.join(appDir, "usr", "share", "libwayland-note.txt"), "utf8"), "keep");
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
        "export GTK_PATH=\"${APPDIR}/usr/lib/gtk-3.0\"",
        "export GSETTINGS_SCHEMA_DIR=\"${APPDIR}/usr/share/glib-2.0/schemas\"",
      ].join("\n"),
    );

    const patched = await patchGdkBackendHook(appDir);

    assert.equal(patched, true);
    const content = await fs.readFile(hookPath, "utf8");
    assert.doesNotMatch(content, /GDK_BACKEND/);
    // Unrelated exports must survive untouched.
    assert.match(content, /export GTK_PATH=/);
    assert.match(content, /export GSETTINGS_SCHEMA_DIR=/);
  });
});

test("patchGdkBackendHook is a no-op when the hook is absent or has no GDK_BACKEND line", async () => {
  await withTempDir(async (appDir) => {
    assert.equal(await patchGdkBackendHook(appDir), false);

    await writeFile(
      path.join(appDir, "apprun-hooks", "linuxdeploy-plugin-gtk.sh"),
      "export GTK_PATH=\"${APPDIR}/usr/lib/gtk-3.0\"\n",
    );
    assert.equal(await patchGdkBackendHook(appDir), false);
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

    assert.equal(result.patchedGdkBackend, true);
    assert.deepEqual(result.removedLibraries, ["usr/lib/libwayland-client.so.0"]);
  });
});

test("parseArgs fails fast for missing appimage value", () => {
  assert.throws(() => parseArgs(["--appimage", "--appimagetool", "tool"]), /Missing value for --appimage/);
  assert.throws(() => parseArgs([]), /Provide --appimage <path>/);
});

test("detectArch derives AppImage arch from filename", () => {
  assert.equal(detectArch("limedl_0.1.8_amd64.AppImage"), "x86_64");
  assert.equal(detectArch("limedl_0.1.8_x86_64.AppImage"), "x86_64");
  assert.equal(detectArch("limedl_0.1.8_aarch64.AppImage"), "aarch64");
  assert.equal(detectArch("limedl_0.1.8_arm64.AppImage"), "aarch64");
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

    await assert.rejects(
      () =>
        pruneAppImageWaylandLibraries({
          appImagePath,
          appImageToolPath: "appimagetool",
          commandRunner,
          workingRoot,
        }),
      /synthetic repack failure/,
    );

    assert.equal(await fs.readFile(appImagePath, "utf8"), "original-appimage");
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

    await assert.rejects(
      () =>
        pruneAppImageWaylandLibraries({
          appImagePath,
          appImageToolPath: "appimagetool",
          commandRunner,
          workingRoot,
        }),
      /failed to start: spawn appimagetool ENOENT/,
    );
    assert.equal(await fs.readFile(appImagePath, "utf8"), "original-appimage");
  });
});

test("cli reports missing appimage argument", () => {
  const result = spawnSync(process.execPath, ["scripts/prune-appimage-wayland-libs.mjs", "--appimage"], {
    cwd: process.cwd(),
    encoding: "utf8",
  });

  assert.notEqual(result.status, 0);
  assert.match(`${result.stdout}\n${result.stderr}`, /Missing value for --appimage/);
});
