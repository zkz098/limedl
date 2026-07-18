# UI Component Guide — flareget

> Complete catalog of shared UI components. All imports are relative to `src/components/`.

---

## Primitive Components (`src/components/ui/`)

### UiBadge

Small label for status, state, or category display.

| Prop | Type | Default | Description |
|---|---|---|---|
| `size` | `"sm" \| "md"` | `"md"` | Badge size |
| `tone` | `"neutral" \| "info" \| "success" \| "warning" \| "danger"` | `"neutral"` | Color tone |

```html
<UiBadge tone="success" size="sm">Completed</UiBadge>
<UiBadge tone="danger">Failed</UiBadge>
<!-- Default slot: badge text -->
```

**Use `toneForState(state)` from `../../composables/downloadHelpers`** to map download state to badge tone.

---

### UiButton

Primary action button with variants, loading state, and icon support.

| Prop | Type | Default | Description |
|---|---|---|---|
| `type` | `"button" \| "submit" \| "reset"` | `"button"` | HTML button type |
| `variant` | `"primary" \| "secondary" \| "ghost" \| "danger"` | `"primary"` | Visual style |
| `size` | `"sm" \| "md"` | `"md"` | Button size |
| `loading` | `boolean` | `false` | Show spinner, disable interaction |
| `disabled` | `boolean` | `false` | Disable interaction |
| `icon` | `string` | `""` | Left icon (UnoCSS class, e.g. `"i-ri-add-line"`) |
| `iconRight` | `string` | `""` | Right icon |
| `block` | `boolean` | `false` | Full-width button |

**Emits**: `click` (MouseEvent)

```html
<UiButton variant="primary" icon="i-ri-download-line" :loading="isDownloading" @click="startDownload">
  Download
</UiButton>

<UiButton variant="danger" size="sm" icon="i-ri-delete-bin-line" @click="deleteItem" />
```

**Variant usage**:
- `primary` — Main actions (save, start, confirm)
- `secondary` — Cancel, secondary actions, toolbar actions
- `ghost` — Subtle actions (refresh, close)
- `danger` — Destructive actions (delete, cancel download)

---

### UiCard

Generic card container with optional header, body, and footer.

**Slots**:
- `header` — Card header (title, actions)
- `default` — Card body content
- `footer` — Card footer

```html
<UiCard>
  <template #header>
    <h3>Section Title</h3>
  </template>
  <!-- body content -->
  <template #footer>
    <UiButton variant="primary">Save</UiButton>
  </template>
</UiCard>
```

**Note**: For settings/labs panel sections, prefer `SettingsSection` which wraps card visual styling with title/icon/summary props.

---

### UiDialog

Modal dialog with teleport, overlay, Escape-to-close, and transition animation.

| Prop | Type | Default | Description |
|---|---|---|---|
| `modelValue` | `boolean` | required | Show/hide dialog |
| `title` | `string` | `""` | Dialog title (or use `#title` slot) |
| `width` | `string` | `"min(42rem, calc(100vw - 1.5rem))"` | Dialog width |
| `closeOnOverlay` | `boolean` | `true` | Close on overlay click |

**Emits**: `update:modelValue` (boolean)

**Slots**:
- `title` — Custom title content
- `default` — Dialog body

```html
<UiDialog v-model="showDialog" title="Settings" @update:model-value="showDialog = $event">
  <p>Dialog content here</p>
</UiDialog>
```

---

### InfoTooltip

Information icon with hover tooltip for supplementary explanations. Designed for settings fields and other contexts where brief help text is needed.

| Prop | Type | Default | Description |
|---|---|---|---|
| `text` | `string` | required | Tooltip message (supports line breaks via `\n`) |

```html
<InfoTooltip :text="t('settings.concurrencyHint')" />
```

**Behavior**:
- Desktop: hover triggers tooltip after 300ms delay
- Mobile/touch: tap to toggle tooltip open/closed
- Single instance — only one tooltip open at a time
- Dismisses on click-outside, Escape key, or re-tapping the icon

**Positioning**: Uses `@floating-ui/dom` with `top` placement, auto-flipping to `bottom` when near viewport edge. Arrow points to the trigger icon.

**Design tokens**: Uses `--color-tooltip-bg` (dark background) and `--color-tooltip-text` (light text) — both defined in `styles.css` and consistent across light/dark themes.

---

### UiEmptyState

Empty container placeholder with icon, title, and optional description.

| Prop | Type | Default | Description |
|---|---|---|---|
| `icon` | `string` | `undefined` | UnoCSS icon class |
| `title` | `string` | required | Main text |
| `description` | `string` | `undefined` | Subtitle text |

```html
<UiEmptyState
  icon="i-ri-inbox-line"
  :title="t('common.noResults')"
  :description="t('common.noResultsHint')"
/>
```

**Always use this component** for empty states — never create ad-hoc empty state markup.

---

### UiProgress

Progress bar with optional label.

| Prop | Type | Default | Description |
|---|---|---|---|
| `value` | `number` | required | Progress 0–100 |
| `indeterminate` | `boolean` | `false` | Indeterminate animation |
| `showLabel` | `boolean` | `false` | Show percentage label |
| `label` | `string` | `undefined` | Custom label text |

```html
<UiProgress :value="75.5" show-label />
<UiProgress indeterminate />
```

Respects `prefers-reduced-motion` for the indeterminate animation.

---

### UiSelect

Accessible dropdown select with keyboard navigation, type-ahead search, and teleported panel.

| Prop | Type | Default | Description |
|---|---|---|---|
| `modelValue` | `T extends string \| number \| null` | required | Selected value |
| `options` | `Array<{ label: string; value: T }>` | required | Options list |
| `disabled` | `boolean` | `false` | Disable select |
| `placeholder` | `string` | `"Select…"` | Placeholder text |

**Emits**: `update:modelValue` (T)

```html
<UiSelect v-model="selectedMode" :options="modeOptions" />
```

Supports: Arrow keys, Home/End, type-to-search, Enter/Space to confirm, Escape to close, click-outside to close.

---

### UiSwitch

Toggle switch with label.

| Prop | Type | Default | Description |
|---|---|---|---|
| `modelValue` | `boolean` | required | Toggle state |
| `label` | `string` | `""` | Label text (or use default slot) |
| `disabled` | `boolean` | `false` | Disable toggle |

**Emits**: `update:modelValue` (boolean)

```html
<UiSwitch v-model="enabled" label="Enable feature" />
<UiSwitch v-model="darkMode">Dark Mode</UiSwitch>
```

---

### UiTextField

Unified text/number input. Replaces three separate components from pre-refactor (UiInput, UiNumberField, UiUnitInput).

| Prop | Type | Default | Description |
|---|---|---|---|
| `modelValue` | `string \| number \| null` | required | Input value |
| `type` | `"text" \| "number" \| "url"` | `"text"` | Input type |
| `placeholder` | `string` | `""` | Placeholder |
| `disabled` | `boolean` | `false` | Disable input |
| `min` | `number` | `undefined` | Minimum (number mode) |
| `max` | `number` | `undefined` | Maximum (number mode) |
| `step` | `number` | `1` | Step (number mode) |
| `unit` | `string` | `undefined` | Unit label ("MB", "MiB/s", etc.) |
| `unitPosition` | `"prefix" \| "suffix"` | `"suffix"` | Unit placement |

**Emits**: `update:modelValue` (string | number | null)

```html
<!-- Text input -->
<UiTextField v-model="name" placeholder="Enter name" />

<!-- Number input -->
<UiTextField type="number" v-model="port" :min="1" :max="65535" />

<!-- Number with unit -->
<UiTextField type="number" v-model="speedLimit" :min="0" :max="32768" unit="MiB/s" />

<!-- Readonly field -->
<UiTextField :model-value="path" readonly />

<!-- Disabled field -->
<UiTextField v-model="value" disabled />
```

**Readonly state**: Automatically styled with muted text/background. Pass `readonly` as a fallthrough attribute.

**Number mode behavior**: Spin buttons hidden. Empty input emits `null`, otherwise emits `Number(raw)`.

---

### ConfirmDialog

Confirmation dialog with customizable kicker, title, message, and action buttons. Composes `UiDialog`.

| Prop | Type | Default | Description |
|---|---|---|---|
| `modelValue` | `boolean` | required | Show/hide |
| `kicker` | `string` | required | Small label above title |
| `title` | `string` | required | Dialog title |
| `message` | `string` | required | Body message |
| `confirmText` | `string` | required | Confirm button text |
| `cancelText` | `string` | required | Cancel button text |
| `width` | `string` | `"min(32rem, ...)"` | Dialog width |
| `icon` | `string` | `undefined` | Title icon |
| `iconDanger` | `boolean` | `false` | Make icon danger-colored |
| `confirmVariant` | `ButtonVariant` | `"danger"` | Confirm button variant |
| `confirmIcon` | `string` | `undefined` | Confirm button icon |
| `confirmLoading` | `boolean` | `false` | Show loading on confirm |
| `confirmDisabled` | `boolean` | `false` | Disable confirm |
| `cancelDisabled` | `boolean` | `false` | Disable cancel |
| `closeOnOverlay` | `boolean` | `true` | Close on overlay click |

**Emits**: `update:modelValue`, `confirm`, `cancel`

**Slots**:
- `default` — Extra content between message and buttons
- `extra-actions` — Buttons inserted between cancel and confirm

```html
<ConfirmDialog
  v-model="showDeleteDialog"
  :kicker="t('common.dangerZone')"
  :title="t('download.permanentDeleteTitle')"
  :message="t('download.permanentDeleteMessage')"
  :confirm-text="t('common.delete')"
  :cancel-text="t('common.cancel')"
  icon="i-ri-alert-line"
  :icon-danger="true"
  :confirm-loading="isDeleting"
  @confirm="performDelete"
  @cancel="showDeleteDialog = false"
>
  <p class="confirm-delete__target">{{ fileName }}</p>
</ConfirmDialog>
```

---

### NotificationToast

Toast notification stack. Managed by `useNotification` composable.

| Prop | Type | Description |
|---|---|---|
| `notifications` | `Notification[]` | List of active notifications |

**Emits**: `dismiss` (id: number)

```html
<NotificationToast :notifications="notifications" @dismiss="dismiss" />
```

The `Notification` type (from `src/types/notification.ts`):
```ts
interface Notification {
  id: number;
  message: string;
  type: "info" | "success" | "error" | "warning";
}
```

Use `useNotification()` composable to manage notifications:
```ts
const { notifications, notify, notifySuccess, notifyError, notifyInfo, notifyWarning, dismiss, clearAll } = useNotification();
notify("Download complete", "success");
// Or use convenience methods:
notifySuccess("Settings saved");
notifyError("Download failed: connection timeout");
```

---

### DataTable

Simple read-only table with column definitions and empty state handling.

| Prop | Type | Default | Description |
|---|---|---|---|
| `columns` | `DataTableColumn[]` | required | Column definitions |
| `rows` | `Array<Record<string, string>>` | required | Data rows |
| `emptyTitle` | `string` | `undefined` | Empty state title |
| `emptyIcon` | `string` | `undefined` | Empty state icon |
| `rowKey` | `string` | `undefined` | Key field in row data for `:key` |

```ts
interface DataTableColumn {
  key: string;           // maps to row[key]
  label: string;         // header text
  width?: string;        // CSS width (e.g. "6rem")
  align?: "left" | "right" | "center";  // text alignment
}
```

```html
<DataTable
  :columns="[
    { key: 'ip', label: 'IP', width: '10rem' },
    { key: 'client', label: 'Client' },
    { key: 'progress', label: 'Progress', width: '6rem', align: 'right' },
  ]"
  :rows="peerRows"
  empty-title="No peers connected"
  empty-icon="i-ri-user-unfollow-line"
/>
```

**Used by**: `BtPeerTable`, `BtTrackerTable` (refactored from manual `<table>` markup).

---

## Composite Components

### SettingsSection (`src/components/settings/`)

Card section wrapper for settings/labs configuration panels. Self-contained visual styling (border, shadow, hover, overflow clipping).

| Prop | Type | Default | Description |
|---|---|---|---|
| `title` | `string` | `undefined` | Section heading |
| `icon` | `string` | `undefined` | UnoCSS icon class |
| `summary` | `string` | `undefined` | Summary text below heading |

```html
<SettingsSection :title="t('settings.btTitle')" icon="i-ri-download-cloud-2-line" :summary="btSummary">
  <SettingsField :label="t('settings.btPort')" :hint="t('settings.btPortHint')">
    <UiTextField type="number" v-model="draft.bt.port" :min="1024" :max="65535" />
  </SettingsField>
</SettingsSection>
```

**Design**: Self-contained — renders correctly outside `.settings-page` context. Has card visual identity (background, border, shadow, hover effect, `overflow: hidden`).

---

### SettingsField (`src/components/settings/`)

Form field wrapper for consistent label + input + hint layout in settings panels.

| Prop | Type | Default | Description |
|---|---|---|---|
| `label` | `string` | `undefined` | Field label |
| `hint` | `string` | `undefined` | Hint text below input |
| `wide` | `boolean` | `false` | Use full-width layout variant |

```html
<SettingsField :label="t('settings.defaultDir')" :hint="t('settings.defaultDirHint')" wide>
  <div class="settings-directory-field">
    <UiTextField :model-value="draft.defaultDir" readonly />
    <UiButton variant="secondary" @click="pickDirectory">{{ t('common.browse') }}</UiButton>
  </div>
</SettingsField>
```

**No scoped CSS** — relies on `.settings-field` parent classes from SettingsPage.vue's non-scoped stylesheet. The `wide` variant uses `settings-field--wide`.

---

### ModalOverlay (`src/components/layout/`)

Fullscreen overlay with centered panel, close button, and transition animation. Replaces the duplicated settings/labs overlay code that was previously inline in App.vue.

| Prop | Type | Default | Description |
|---|---|---|---|
| `modelValue` | `boolean` | required | Show/hide overlay |

**Emits**: `update:modelValue` (boolean), `close`

```html
<ModalOverlay :model-value="currentView === 'settings'" @close="navigateTo('home')">
  <SettingsPage ... />
</ModalOverlay>
```

**Features**: Backdrop blur, overlay click to close, top-right close button, scale+fade enter/leave transition, responsive at 680px breakpoint.

---

### StatRow (`src/components/sidebar/`)

Key-value stat display row for sidebar panels.

| Prop | Type | Default | Description |
|---|---|---|---|
| `label` | `string` | required | Stat label |
| `value` | `string \| number` | required | Stat value |
| `mono` | `boolean` | `false` | Use monospace font for value |

```html
<StatRow :label="t('sidebar.dhtNodes')" :value="btStatus.dhtNodes" mono />
<StatRow :label="t('sidebar.activeTasks')" :value="activeTasks" />
```

**Used by**: `CategorySidebar`, `SidebarBtStatus`.

---

## Domain Components

### DetailPanel (`src/components/flareget/`)

Collapsible detail panel for the selected download task. Shows filename, state badge, CDN badge, action buttons (refresh/pause/resume/cancel/close), and the `DownloadInspector` body.

| Prop | Type | Description |
|---|---|---|
| `selectedOverview` | `DownloadSummary \| null` | Selected task summary |
| `selectedSnapshot` | `DownloadSnapshot \| null` | Full snapshot for inspector |
| `selectedId` | `string \| null` | Selected task ID |
| `canPause` | `boolean` | Whether pause is available |
| `canResume` | `boolean` | Whether resume is available |
| `canCancel` | `boolean` | Whether cancel is available |
| `actionName` | `string` | Current action name (for loading state) |
| `isRefreshingStatus` | `boolean` | Whether status is refreshing |
| `showDetailInfo` | `boolean` | Show detailed info section |

**Emits**: `close`, `refresh`, `pause`, `resume`, `cancel`

---

## Component Selection Flowchart

```
Need a...                          → Use this component
─────────────────────────────────────────────────────
Button action                      → UiButton
Status/category label              → UiBadge
Text / number / unit input         → UiTextField
Toggle switch                      → UiSwitch
Dropdown select                    → UiSelect
Progress display                   → UiProgress
Modal dialog                       → UiDialog or ConfirmDialog
Fullscreen overlay                 → ModalOverlay
Content card                       → UiCard
Settings panel section             → SettingsSection (+ SettingsField for fields)
Empty/placeholder display          → UiEmptyState
Key-value stat row                 → StatRow
Simple read-only table             → DataTable
Toast notification                 → useNotification() + NotificationToast
Download detail panel              → DetailPanel
```

## Layout Components

### TopToolbar (`src/components/layout/`)

Top navigation bar with search, sort controls, view options, multi-select batch actions, and game/overclock toggles. Consumed by `App.vue` directly.

### CategorySidebar (`src/components/layout/`)

Left sidebar with download category filters and BT status. Uses `StatRow` for stat displays.

### SidebarBtStatus (`src/components/sidebar/`)

BitTorrent status panel showing DHT nodes, upload speed, peer count, and torrent count. Uses `StatRow`.

---

## Import Paths

Paths below assume importing from a page or composable at depth 2 from `src/` (e.g., `src/composables/useFoo.ts` or `src/views/BarPage.vue`). Adjust the `../` prefix based on your file's location.

```ts
// Primitive UI components
import UiBadge from "../components/ui/UiBadge.vue"
import UiButton from "../components/ui/UiButton.vue"
import UiCard from "../components/ui/UiCard.vue"
import UiDialog from "../components/ui/UiDialog.vue"
import UiEmptyState from "../components/ui/UiEmptyState.vue"
import UiProgress from "../components/ui/UiProgress.vue"
import UiSelect from "../components/ui/UiSelect.vue"
import UiSwitch from "../components/ui/UiSwitch.vue"
import UiTextField from "../components/ui/UiTextField.vue"
import ConfirmDialog from "../components/ui/ConfirmDialog.vue"
import NotificationToast from "../components/ui/NotificationToast.vue"
import DataTable from "../components/ui/DataTable.vue"

// Composite components
import SettingsSection from "../components/settings/SettingsSection.vue"
import SettingsField from "../components/settings/SettingsField.vue"
import ModalOverlay from "../components/layout/ModalOverlay.vue"
import StatRow from "../components/sidebar/StatRow.vue"

// Composables
import { useNotification } from "../composables/useNotification"
import { toneForState } from "../composables/downloadHelpers"
import { useFloatingClose } from "../composables/useFloatingClose"
import { useAsyncGuard } from "../composables/useAsyncGuard"
```
