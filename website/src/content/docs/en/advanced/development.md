---
title: Build & Contributing
description: Setting up a local limedl Rust and Slint development environment, compiling release profiles, and passing mandatory CI verification gates
---

# Build & Contributing

limedl is licensed under **GPL-3.0-or-later**. Contributions, issue reports, and pull requests from developers are warmly welcomed!

---

## Local Development Setup

After cloning the repository, configure the necessary toolchains for your platform:

### 1. Base Toolchains
- **Rust Toolchain**: Install the latest stable release (Rust 2024 edition support required).
  ```bash
  rustup update stable
  ```
- **Nextest Test Runner**: limedl uses `cargo-nextest` for isolated, process-per-test parallel test execution:
  ```bash
  cargo install cargo-nextest --locked --version 0.9.144
  ```

### 2. Operating System Prerequisites
- **Windows**:
  - Install Visual Studio 2022 / 2026 or MSVC C++ x64 Build Tools.
  - Before running cargo commands, initialize the MSVC development environment:
    ```cmd
    cmd.exe /k "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
    ```
- **Linux (Ubuntu / Debian)**:
  - System tray and desktop integration require GTK-3 header packages:
    ```bash
    sudo apt install build-essential pkg-config libgtk-3-dev libayatana-appindicator3-dev
    ```
- **macOS**:
  - Install Xcode Command Line Tools: `xcode-select --install`.

### 3. Fetch UI Font (Mandatory Step)
Due to licensing constraints, the MiSans Variable Font is not stored in Git. Before building `limedl-native`, download the font via PowerShell:
```powershell
pwsh scripts/fetch-misans.ps1
```

---

## Building & Running

```powershell
# 1. Run the desktop application in debug mode
cargo run -p limedl-native

# 2. Pass CLI parameters (e.g. silent tray launch)
cargo run -p limedl-native -- --hidden

# 3. Compile optimized release binary (optimized for distribution size)
cargo build --release -p limedl-native

# 4. Compile high-performance native profile (opt-level 3 + codegen-units 1 for Skia rendering)
cargo build -p limedl-native --profile native-release
```

---

## Pre-Commit CI Gate Verification

limedl enforces strict CI rules: any compiler warning, Clippy lint, or test failure will fail the pull request pipeline.

Before committing and pushing your code, run the full verification gate locally:

```powershell
# 1. Enforce zero-warning policy
$env:RUSTFLAGS = "-D warnings"
$env:CARGO_REGISTRIES_CRATES_IO_PROTOCOL = "sparse"

# 2. Workspace-wide Clippy static analysis
cargo clippy --workspace --all-targets

# 3. Unit and integration tests for download engine and RPC
cargo nextest run --manifest-path crates/limedl-core/Cargo.toml --features "test-utils,aria2-rpc"

# 4. Desktop client test suite
cargo nextest run --manifest-path crates/limedl-native/Cargo.toml
```

All four checks must pass cleanly (green) before committing.

---

## Slint Native UI Architectural Guidelines

When working on user interface code in `crates/limedl-native/`, adhere to the following conventions:

### 1. Tokenized Theme Colors
Never hardcode hex color literals in `.slint` files. Reference tokens defined in `ui/theme.slint`:
```rust
import { Theme } from "theme.slint";

Rectangle {
    background: Theme.card-bg;
    border-color: Theme.border-color;
}
```

### 2. Dual-Track Internationalization (i18n)
- **Static `.slint` Strings**: All user-visible strings in declarative `.slint` files must be wrapped with `@tr(...)`:
  ```rust
  Text {
      text: @tr("Start Download");
  }
  ```
- **Dynamic Rust Strings**: Tray titles, error toasts, and validation errors must use the `i18n::format_*` helper functions provided in `crates/limedl-native/src/i18n.rs`. Never hardcode raw strings.

### 3. Handlers and UI Bridge Separation
Slint callbacks should be encapsulated in their respective `handlers/` submodules and interact with the UI through `handlers/common.rs` helper wrappers (`with_ui`, `mutate_store`).

---

## Commit Message Conventions

This project uses **git-cliff** to automatically generate GitHub Release changelogs from commit messages.

Commits must follow the Conventional Commits specification:
- `feat(manager): support dynamic AIMD backoff coefficients`
- `fix(cdn): resolve timeout filtering on edge IP candidates`
- `perf(buffer-pool): optimize BTreeMap allocation during HDD flushes`
- `refactor(native): split settings dialog into modular components`
- `docs(website): detail system architecture and buffer pool mechanics`
