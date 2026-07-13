# COMPONENTS (Frontend)

**4 subdirectories, ~22 files.** Vue 3 SFCs — reusable design system primitives + domain-specific feature components.

## STRUCTURE

```
components/
├── ui/              # 8 reusable primitives → design system
│   ├── UiButton.vue       # 4 variants, 2 sizes, loading spinner, icon slot
│   ├── UiDialog.vue       # Modal with overlay, Escape close, body scroll lock
│   ├── UiInput.vue        # Wrapped text input
│   ├── UiSelect.vue       # Typed generic select dropdown
│   ├── UiNumberField.vue  # Null-safe number input
│   ├── UiBadge.vue        # 5 color tones, 2 sizes
│   ├── UiProgress.vue     # Progress bar with optional label
│   └── UiCard.vue         # Card container (header/body/footer slots)
├── downloader/      # Download task views
│   ├── DownloadQueueTable.vue    # Paginated table, context menu, column picker (945 lines)
│   ├── DownloadInspector.vue     # Floating detail panel with metrics grid
│   └── DownloadComposer.vue      # New-task form with source tabs (URL/torrent/metalink)
├── settings/        # SettingsPage.vue (644 lines — largest SFC, down from 965 after useSettingsForm extraction)
└── sidebar/         # SidebarBtStatus.vue — BT runtime status display
```

## WHERE TO LOOK

| Task             | Location                            | Notes                                                                  |
| ---------------- | ----------------------------------- | ---------------------------------------------------------------------- |
| Button API       | `ui/UiButton.vue`                   | Props: `variant`, `size`, `icon`, `loading`, `block`                   |
| Dialog behavior  | `ui/UiDialog.vue`                   | `v-model` binding, `closeOnOverlay`, `width`, named slots              |
| Select pattern   | `ui/UiSelect.vue`                   | `defineProps<{ options: { value: T; label: string }[] }>()`            |
| Queue actions    | `downloader/DownloadQueueTable.vue` | Emits: `pauseOrResume`, `deleteTask`, `openInExplorer`, `select`       |
| Inspector layout | `downloader/DownloadInspector.vue`  | Accepts `selectedOverview` (snapshot or summary), computed detail rows |
| Settings form    | `settings/SettingsPage.vue`         | 7 panels, reactive draft with dirty tracking via JSON compare          |
| Composer tabs    | `downloader/DownloadComposer.vue`   | Source type switching (URL/bt/metalink), conditional form fields       |

## CONVENTIONS

- **All `<script setup lang="ts">`** — Composition API exclusively. No Options API.
- **Props**: `defineProps<T>()` with type-only generics. `withDefaults()` for optional props.
- **Emits**: `defineEmits<{ event: [arg: Type] }>()` — type-only generics, never runtime arrays.
- **Expose**: `defineExpose()` for parent-accessible methods (e.g., `SettingsPage.persistSettings()`).
- **CSS**: Scoped styles (`<style scoped>`). BEM-like naming: `block__element--modifier`.
- **CSS tokens**: Use `var(--color-*)`, `var(--space-*)`, `var(--radius-*)` from `styles.css`. Never hardcode colors.
- **Icons**: UnoCSS `i-ri-*` classes (Remix Icons via `@iconify-json/ri`).
- **No Pinia imports** — state comes from `useDownloader()` composable passed via props/emits.

## ANTI-PATTERNS

- **SettingsPage.vue is monolithic** — 644 lines in one SFC (down from 965 after `useSettingsForm` composable extraction). Split into `AppearancePanel.vue`, `ProxyPanel.vue`, `SchedulerPanel.vue`, etc.
- **DownloadQueueTable.vue at 945 lines** — mix of rendering, context menu, column management. Extract composables for column state and context menu logic.
- **Minimal component tests** — 2 vitest files exist (smoke + type shape) but cover ~0% of business logic. Zero component/composable tests.
- **Raw CSS scoping** — `scoped` attribute used throughout. No CSS Modules or `<style module>` for type-safe class names.
