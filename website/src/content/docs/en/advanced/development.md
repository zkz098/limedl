---
title: Building from Source & Contributing
description: Set up your local Rust and Slint development environment for limedl
---

# Building from Source & Contributing

limedl is licensed under GPL-3.0. We warmly welcome community contributions!

## Prerequisites

- **Rust**: Rust 2024 edition (latest stable channel)
- **C++ Compiler (Windows)**: Visual Studio MSVC C++ x64 Build Tools
- **UI Fonts**:
  ```powershell
  # Fetch MiSans VF embedded font required for Slint
  pwsh scripts/fetch-misans.ps1
  ```

## Running the App

```powershell
# Run the Slint native desktop client in debug mode
cargo run -p limedl-native

# Build optimized release binary
cargo build --release -p limedl-native
```

## Mandatory Pre-commit Verification

Before submitting pull requests, ensure all checks pass:

```powershell
# Lint check
cargo clippy --workspace --all-targets -- -D warnings

# Tests with cargo-nextest
cargo nextest run --manifest-path crates/limedl-core/Cargo.toml --features "test-utils,aria2-rpc"
cargo nextest run --manifest-path crates/limedl-native/Cargo.toml
```
