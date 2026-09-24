---
title: 源码编译与参与开发
description: 搭建 limedl 本地 Rust 与 Slint 开发环境，运行测试并参与开源贡献
---

# 源码编译与参与开发

limedl 遵循 GPL-3.0 开源协议，欢迎所有开发者参与共建！

## 环境准备

- **Rust 工具链**：需要 Rust 2024 edition（推荐最新 Stable 版本）
- **C++ 编译环境 (Windows)**：Visual Studio 2022 / 2026 MSVC C++ x64 Build Tools
- **UI 字体准备**：
  ```powershell
  # 下载 Slint UI 编译所需的 MiSans 字体
  pwsh scripts/fetch-misans.ps1
  ```

## 运行与编译

```powershell
# 本地调试运行原生桌面客户端
cargo run -p limedl-native

# 编译优化发布包
cargo build --release -p limedl-native
```

## 测试与代码质量门禁 (CI Gate)

提交 PR 前，请确保全套门禁通过：

```powershell
# 代码静态分析
cargo clippy --workspace --all-targets -- -D warnings

# 单元测试与集成测试
cargo nextest run --manifest-path crates/limedl-core/Cargo.toml --features "test-utils,aria2-rpc"
cargo nextest run --manifest-path crates/limedl-native/Cargo.toml
```
