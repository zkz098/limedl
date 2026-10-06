---
title: 源码编译与参与开发
description: 搭建 limedl 本地开发环境，掌握 Rust 编译命令、Slint UI 约定与本地 CI 门禁核验
---

# 源码编译与参与开发

limedl 遵循 **GPL-3.0-or-later** 开源协议，欢迎所有开发者参与共建！

---

## 本地开发环境准备

在克隆仓库后，请根据你所使用的操作系统安装基础工具链：

### 1. 基础工具链
- **Rust 工具链**：安装最新的 Rust Stable 版本（必须支持 Rust 2024 edition）。
  ```bash
  rustup update stable
  ```
- **Nextest 测试执行器**：项目使用 `cargo-nextest` 执行隔离并行测试：
  ```bash
  cargo install cargo-nextest --locked --version 0.9.144
  ```

### 2. 操作系统专有依赖
- **Windows**:
  - 需要安装 Visual Studio 2022 / 2026 或 MSVC C++ x64 Build Tools。
  - 在执行任何 Rust 编译前，建议初始化 MSVC 环境变量：
    ```cmd
    cmd.exe /k "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
    ```
- **Linux (Ubuntu / Debian)**:
  - 构建需要 C 工具链、pkg-config 与 fontconfig 开发头文件（Slint 字体栈）：
    ```bash
    sudo apt install build-essential pkg-config libfontconfig1-dev
    ```
- **macOS**:
  - 安装 Xcode 命令行工具：`xcode-select --install`。

### 3. 一键获取 Slint 编译字体（关键步骤）
由于 MiSans 字体许可限制，字体未签入 Git 仓库。在首次编译 `limedl-native` 前，必须通过 xtask 下载字体：
```bash
cargo xtask fetch-font
```

---

## 本地编译与运行

```powershell
# 1. 以 Debug 模式快速启动运行桌面端
cargo run -p limedl-native

# 2. 传递启动参数进行测试 (如隐式托盘启动)
cargo run -p limedl-native -- --hidden

# 3. 编译发布包 (Release Profile，体积优先优化)
cargo build --release -p limedl-native

# 4. 编译极限性能版 (Native-Release Profile，针对 FemtoVG 渲染深度内联优化)
cargo build -p limedl-native --profile native-release
```

---

## 强制质量门禁验证 (Pre-commit CI Gate)

limedl 的 GitHub Actions CI 检查极为严苛：任何编译警告、Clippy 提示或测试失败都会直接导致流水线变红。

在提交 Pull Request 之前，必须在本地完整运行与 CI 完全对齐的验证门禁：

```powershell
# 1. 环境变量配置
$env:CARGO_BUILD_WARNINGS = "deny"
$env:CARGO_REGISTRIES_CRATES_IO_PROTOCOL = "sparse"

# 2. 静态代码分析检查 (全 Workspace，-D warnings)
cargo clippy --workspace --all-targets -- -D warnings

# 3. 执行核心引擎单元与 RPC 测试
cargo nextest run --manifest-path crates/limedl-core/Cargo.toml --features "test-utils,aria2-rpc"

# 4. 执行仓库运维与发布工具测试
cargo nextest run --manifest-path xtask/Cargo.toml

# 5. 执行桌面客户端测试
cargo nextest run --manifest-path crates/limedl-native/Cargo.toml

# 6. 执行无头服务端测试
cargo nextest run --manifest-path crates/limedl-server/Cargo.toml
```

只有当上述命令**全部显示绿色**时，才允许提交代码并推送。

---

## Slint 原生界面开发约定

在修改 `crates/limedl-native/` 界面代码时，请遵循项目架构规范：

### 1. 主题配色 Token 化
严禁在 `.slint` 界面文件中硬编码类似 `#84cc16` 的十六进制颜色值。必须引用 `ui/theme.slint` 中由设计系统统一定义的色彩 Token：
```rust
import { Theme } from "theme.slint";

Rectangle {
    background: Theme.card-bg;
    border-color: Theme.border-color;
}
```

### 2. 双轨国际化 (i18n Dual-Track)
- **.slint 界面静态文案**：所有呈现在 UI 上的文字必须包裹在 `@tr(...)` 宏中：
  ```rust
  Text {
      text: @tr("开始下载");
  }
  ```
- **Rust 侧动态文案**：例如系统托盘菜单、错误通知 Toast、设置校验失败提示等，**严禁硬编码中文字符串**，必须通过 `crates/limedl-native/src/i18n/mod.rs` 提供的 `i18n::format_*` 格式化辅助函数产出，确保中英双语无缝切换。

### 3. Handlers 业务解耦
Slint 的回调函数必须统一定义于 `handlers/` 对应子模块中，并通过 `handlers/common.rs` 暴露的辅助函数（`with_ui`、`mutate_store` 等）安全读写 UI 模型，严禁在单个回调内编写数百行混乱的业务状态。

---

## 提交规范 (Conventional Commits)

本项目通过 **git-cliff** 自动提取 Git 提交历史生成 GitHub Release 变更日志。

提交消息必须符合 Angular / Conventional Commits 规范：
- `feat(manager): 支持动态调整 AIMD 退避系数`
- `fix(cdn): 修复某些海外 IP 握手超时未被正确剔除的缺陷`
- `perf(buffer-pool): 优化 HDD 双缓冲刷盘时的 BTreeMap 内存分配`
- `refactor(native): 拆分设置面板的子组件模块`
- `docs(website): 细化与完善系统架构文档`
