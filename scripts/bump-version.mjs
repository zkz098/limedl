#!/usr/bin/env node

import { execSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

// ── Parse args ──────────────────────────────────────────────────────────

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(scriptDir, "..");

function parseArgs(argv) {
  const level = argv[0];
  if (!level || !["patch", "minor", "major"].includes(level)) {
    console.error(
      "Usage: node scripts/bump-version.mjs <patch|minor|major> [--no-push] [--dry-run]",
    );
    process.exit(1);
  }
  return {
    level,
    noPush: argv.includes("--no-push"),
    dryRun: argv.includes("--dry-run"),
  };
}

// ── Read current version ────────────────────────────────────────────────

// Run a shell command in the repo root, returning its stdout.
function exec(cmd) {
  return execSync(cmd, { cwd: root, encoding: "utf8", stdio: "pipe" });
}

function readCurrentVersion() {
  const cargoPath = path.join(root, "Cargo.toml");
  const cargoContent = readFileSync(cargoPath, "utf8");
  const match = cargoContent.match(/^version\s*=\s*"(\d+\.\d+\.\d+)"/m);
  if (!match) {
    throw new Error("Could not parse version from Cargo.toml");
  }
  return match[1];
}

// ── Bump ────────────────────────────────────────────────────────────────

function bumpVersion(current, level) {
  const parts = current.split(".").map(Number);
  switch (level) {
    case "major":
      parts[0]++;
      parts[1] = 0;
      parts[2] = 0;
      break;
    case "minor":
      parts[1]++;
      parts[2] = 0;
      break;
    case "patch":
      parts[2]++;
      break;
  }
  return parts.join(".");
}

// ── Update files ────────────────────────────────────────────────────────

/**
 * Cargo.lock mirrors workspace member versions. Replace them by name so
 * third-party crates that happen to share the same version string are
 * never touched (they exist in the lockfile today).
 */
function updateLockVersions(lockContent, newVersion) {
  const updated = lockContent.replace(
    /^(name = "limedl(?:-core|-server)?"\n)version = "[^"]+"/gm,
    (match, prefix) => `${prefix}version = "${newVersion}"`,
  );
  if (updated === lockContent) {
    throw new Error("Cargo.lock: workspace member versions not found");
  }
  return updated;
}

// ── Main ────────────────────────────────────────────────────────────────

function main() {
  const args = parseArgs(process.argv.slice(2));
  const currentVersion = readCurrentVersion();
  const newVersion = bumpVersion(currentVersion, args.level);

  const files = ["Cargo.toml", "package.json", "src-tauri/tauri.conf.json", "Cargo.lock"];

  console.log(`\x1b[36m${currentVersion} → ${newVersion} (${args.level})\x1b[0m`);

  if (args.dryRun) {
    console.log("\x1b[33m[dry-run] Would update:\x1b[0m");
    for (const f of files) {
      console.log(`  ${f} : ${currentVersion} → ${newVersion}`);
    }
    process.exit(0);
  }

  // Update files
  for (const f of files) {
    const filePath = path.join(root, f);
    const content = readFileSync(filePath, "utf8");
    // Cargo.lock holds workspace member versions; a plain replaceAll would
    // also rewrite third-party crates that share the version string.
    const updated =
      f === "Cargo.lock"
        ? updateLockVersions(content, newVersion)
        : content.replaceAll(currentVersion, newVersion);
    writeFileSync(filePath, updated, "utf8");
    console.log(`\x1b[32m  Updated: ${f}\x1b[0m`);
  }

  if (args.noPush) {
    process.exit(0);
  }

  // Git commit, tag, push
  exec(`git add ${files.join(" ")}`);
  exec(`git commit -m "chore: bump version to ${newVersion}"`);
  exec("git push origin main");
  exec(`git tag "v${newVersion}" -m "v${newVersion}"`);
  exec(`git push origin "v${newVersion}"`);

  console.log(`\x1b[32mPushed commit + tag v${newVersion}\x1b[0m`);
}

main();
