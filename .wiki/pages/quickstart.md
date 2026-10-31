---
id: quickstart
title: Wiki 快速入口
status: verified
generated_at: 2026-10-31T15:59:16Z
commit: b5102b9f00e4d40e84fa3ca5da17fc3e95e4c45b
evidence: []
tags:
- generated
type: index
---
# limedl Wiki —— 快速入口

生成的导航页,不作为源码证据;完整目录见 [[wiki:index]]。

## 推荐阅读路径

### architecture

1. [[wiki:architecture/bootstrap-and-services]] —— Bootstrap, SystemContext and Shared Services
1. [[wiki:architecture/overview]] —— Workspace and System Architecture
1. [[wiki:architecture/protocol-routing-and-dispatcher]] —— Protocol Routing and the Dispatcher Facade

### workflow

1. [[wiki:workflows/bit-torrent-backend]] —— BitTorrent Backend
1. [[wiki:workflows/cdn-acceleration]] —— CDN Acceleration
1. [[wiki:workflows/http-download-lifecycle]] —— HTTP Download Lifecycle
1. [[wiki:workflows/metalink-and-mirror-selection]] —— Metalink Parsing and Mirror Selection
1. [[wiki:workflows/scheduler-and-concurrency]] —— Scheduler, AIMD and Concurrency Control

### system

1. [[wiki:systems/disk-io-and-storage]] —— Disk I/O, Buffer Pool and Storage Detection
1. [[wiki:systems/networking-and-rate-control]] —— Networking, HTTP Clients and Rate Control
1. [[wiki:systems/persistence-and-recovery]] —— SQLite Persistence, Durable Progress and Crash Recovery
1. [[wiki:systems/settings-and-configuration]] —— Settings and Configuration

### integration

1. [[wiki:integrations/aria2-rpc-server]] —— Aria2 JSON-RPC Compatibility Server
1. [[wiki:integrations/headless-server-daemon]] —— Headless Server Daemon (limedl-server)

### testing

1. [[wiki:testing/slint-ui-testing]] —— Slint UI Testing
1. [[wiki:testing/testing-strategy]] —— Testing Strategy

### 其他

- [[wiki:desktop/native-ui-architecture]] —— Native Desktop UI (Slint)
- [[wiki:desktop/self-update-and-distribution]] —— Self-Update and Distribution Channels
- [[wiki:desktop/task-input-and-aria2c-parsing]] —— Task Input Surfaces and aria2c Command Parsing
- [[wiki:operations/build-release-and-ci]] —— Build, Tooling, CI and Release Operations
- [[wiki:operations/server-deployment-and-packaging]] —— Server Deployment and Packaging

## 如何使用本 Wiki(agent 协议)

1. 信任任何页面前先调用 `wiki_check_freshness`(或 `loredex check`);过期页面按未验证对待,直接读源码。
2. 用 `wiki_get_evidence` 核查论断 —— 每条引用都能解析到当前 git HEAD 上的精确文件内容。
3. 保持 wiki 新鲜:`wiki_plan` 列出待写/待修页面;写作时每条论断配一个行内 `[[cite:path:Lx-Ly]]` 标记;漂移的引用用 `wiki_refresh_page` 自愈。
