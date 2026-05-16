# PROJECT KNOWLEDGE BASE

**Generated:** 2026-05-16
**Commit:** 7af4058
**Branch:** main

## OVERVIEW
Tauri 2 desktop download manager. Vue 3 frontend, Rust backend with HTTP/BitTorrent/SFTP engines.

## STRUCTURE
```
downloader/
├── src/                    # Vue 3 + TypeScript frontend (no router, no Pinia)
│   ├── components/         # → AGENTS.md (design system + domain components)
│   │   ├── ui/             # 8 reusable primitives (UiButton, UiDialog, etc.)
│   │   ├── downloader/     # Queue table, inspector, composer
│   │   ├── settings/       # SettingsPage.vue (1464 lines — largest SFC)
│   │   └── sidebar/        # BT runtime status
│   ├── composables/        # useDownloader (singleton state manager), usePolling, useFileDialog
│   ├── lib/tauri/          # Thin invoke() wrappers — no business logic
│   ├── types/              # TypeScript mirrors of Rust types (manual duplication)
│   ├── i18n/               # i18next — zh-CN + en-US in single resources.ts (828 lines)
│   └── styles.css          # CSS custom properties — design token hub
├── src-tauri/              # Rust backend
│   └── src/download/       # → AGENTS.md (core download engine — 12 files, 6400+ lines)
├── package.json            # Scripts: dev/build/lint(oxlint)/format(oxfmt)/tauri
├── vite.config.ts          # Port 1420, UnoCSS, no src-tauri watch
└── uno.config.ts           # presetUno + presetIcons (Remix)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Download lifecycle | `src-tauri/src/download/manager.rs` | 3414-line god object — HTTP scheduling + AIMD + persistence |
| Tauri commands | `src-tauri/src/download/commands.rs` | Thin dispatch layer — routes by task ID prefix |
| Frontend state | `src/composables/useDownloader.ts` | Singleton composable — 38 exports, replaces Pinia |
| UI components | `src/components/ui/` | UiButton, UiDialog, UiInput, UiSelect, UiBadge, UiCard, UiProgress, UiNumberField |
| IPC bridge | `src/lib/tauri/download-api.ts` | Plain async functions wrapping invoke() |
| Settings | `src/components/settings/SettingsPage.vue` | 1464 lines — reactive form, dirty tracking, 7 panels |
| i18n strings | `src/i18n/resources.ts` | Both languages in one file (828 lines) |
| TypeScript types | `src/types/download.ts`, `src/types/settings.ts` | Manual mirrors of Rust serde structs |
| Rust types | `src-tauri/src/download/types.rs` | All structs/enums — `#[serde(rename_all = "camelCase")]` |
| BitTorrent | `src-tauri/src/download/torrent.rs` | librqbit wrapper — 2 impl blocks, pending/resolved states |
| SFTP | `src-tauri/src/download/sftp.rs` | ssh2 wrapper — CONNECT_TIMEOUT 20s, IO_TIMEOUT 45s |

## CONVENTIONS
- **Vue SFC**: `<script setup lang="ts">` exclusively. `defineProps<T>()`, `defineEmits<{}>()`, type-only generics.
- **State**: No Pinia/Vuex. `useDownloader()` composable is the single state source.
- **Routing**: No Vue Router. `currentView` ref in App.vue with `v-if` switching.
- **Imports**: Relative only — no `@/` or `~/` aliases configured.
- **Error handling (TS)**: Try/catch → `toMessage(error)` → `setError()` → `finally` resets loading.
- **Error handling (Rust)**: Commands return `Result<T, String>`. `into_command_result()` wraps `anyhow::Result`. `.context()` for enrichment.
- **Lint/format**: `bunx oxlint --type-aware --type-check . --fix` / `bunx oxfmt .`
- **TypeScript**: Strict mode, `noUnusedLocals`, `noUnusedParameters`. TypeScript 6.0.
- **Rust edition**: 2024. Mimalloc global allocator. `windows_subsystem = "windows"` in release.
- **Theming**: CSS custom properties + `data-theme`/`data-color-mode`/`data-surface` attributes. No class-based theming.
- **Task IDs**: Prefixed strings — `http:{uuid}`, `bt:{rqbit_id}`, `bt:pending:{uuid}`, `sftp:{uuid}`.

## ANTI-PATTERNS (THIS PROJECT)
- **manager.rs is a god object** — 3414 lines, 108 functions. Do NOT add more to it. Split into scheduler/aimd/persistence modules before extending.
- **commands.rs has copy-paste dispatch** — all 13 commands repeat the same 3-branch pattern. Extract to a macro or router function instead of duplicating.
- **String-based task ID routing** — fragile prefix matching (`is_bt_task_id`, `is_sftp_task_id`). Prefer an enum-based approach.
- **useDownloader.ts is overloaded** — 38 exported values. Split into `useDownloadList`, `useDownloadActions`, `useDownloadForm`.
- **SettingsPage.vue is monolithic** — 1464 lines in one SFC. Split into 7 panel sub-components + settingsComposables.ts.
- **No bare `.unwrap()` calls** — all 67 usages are safe `.unwrap_or()` / `.unwrap_or_else()` / `.unwrap_or_default()` variants with fallbacks. This anti-pattern was a false alarm.
- **Dual lockfiles** — both `bun.lock` and `package-lock.json`. Standardize on one.
- **No tests** — despite dev-dependencies (ntest, tempfile, axum), zero test files exist.
- **No CI** — no `.github/workflows/`.
- **i18n resources.ts** — 828 lines, both languages in one file. Split into per-language files.

## COMMANDS
```bash
bun run dev          # Vite dev server (port 1420)
bun run build        # vue-tsc --noEmit && vite build
bun run tauri dev    # Tauri dev mode (Rust + frontend)
bun run lint         # oxlint --type-aware --type-check . --fix
bun run format       # oxfmt .
```

## NOTES
- `vite.config.ts` uses `@ts-expect-error` for `process.env.TAURI_DEV_HOST` — justified (Node.js global in config file).
- `index.html` title still says "Tauri + Vue + Typescript App" — template default. **Fixed 2026-05-16.**
- `librqbit` integration is unusual for Tauri — test thoroughly on all platforms.
- Tauri CSP is `null` (disabled). No security headers.
- Types are manually duplicated between Rust (`serde`) and TypeScript — no auto-generation.
- `autoRefreshIntervalMs = 1500` in useDownloader — UI polls backend every 1.5s.
- **manager.rs split plan** (deferred to dedicated session): Extract `scheduler.rs` (AIMD rate control, thread allocation, `SchedulerMode`, `AimdState`), `persistence.rs` (manifest read/write, settings serialization), `http_client.rs` (reqwest builder, response handling, range negotiation). Keep `manager.rs` as the orchestrator (~1200 lines).
- All 67 `.unwrap_*()` calls in the codebase are safe `.unwrap_or()` / `.unwrap_or_else()` / `.unwrap_or_default()` variants with fallbacks. No bare panicking `.unwrap()` calls exist.
