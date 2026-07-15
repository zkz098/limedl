# UI Design Guide — downloader

> Design tokens, patterns, and conventions for the downloader frontend (Vue 3 + TypeScript + UnoCSS).

## Tech Stack

- **Framework**: Vue 3 Composition API (`<script setup lang="ts">`)
- **CSS engine**: UnoCSS (with `presetUno` + `presetIcons`)
- **Icons**: Remix Icon via UnoCSS (`i-ri-*` classes)
- **Styling**: Scoped CSS in `.vue` SFCs, CSS custom properties for theming

## Design Tokens

All tokens are defined as CSS custom properties in `src/styles.css`. Always reference tokens by variable name — never hardcode values.

### Colors

| Token | Light value | Role |
|---|---|---|
| `--color-bg-base` | `oklch(0.995 0 0)` | Page background |
| `--color-panel` | `oklch(1 0 0)` | Card/panel background |
| `--color-panel-muted` | `oklch(0.985 0 0)` | Subdued panel (readonly inputs, empty states) |
| `--color-surface-muted` | `oklch(0.97 0 0)` | Table headers, hover backgrounds |
| `--color-surface-hover` | `oklch(0.96 0 0)` | Stronger hover |
| `--color-heading` | `oklch(0.21 0 0)` | Headings and labels |
| `--color-text-main` | `oklch(0.32 0 0)` | Body text |
| `--color-text-muted` | `oklch(0.55 0 0)` | Secondary text, hints |
| `--color-text-soft` | `oklch(0.75 0 0)` | Placeholder text |
| `--color-input-bg` | `oklch(1 0 0)` | Input field background |
| `--color-border` | `oklch(0.92 0 0)` | Default borders |
| `--color-border-strong` | `oklch(0.82 0 0)` | Hover/active borders |
| `--border-width-thin` | `1px` | Standard border width |

**Accent colors** (green-lime default, amber/sky variants available via `data-theme`):

| Token | Role |
|---|---|
| `--color-accent` | Primary accent (buttons, progress bar) |
| `--color-accent-strong` | Accent hover / active / switch-on |
| `--color-accent-soft` | Accent background (selected items) |
| `--color-accent-soft-border` | Subtle accent border |
| `--color-accent-border` | Medium accent border |
| `--color-accent-alt` | Alternative accent hue |
| `--color-accent-contrast` | Text on accent backgrounds |
| `--color-focus-ring` | Focus ring color |
| `--color-progress-track` | Progress bar background |

**Semantic colors** (info/success/warning/danger — each has `-bg`, `-border`, `-text` variants):

| Token family | Use |
|---|---|
| `--color-info-*` | Informational banners, info badges |
| `--color-success-*` | Completed state, success toasts |
| `--color-warning-*` | Paused/queued state, CDN badge |
| `--color-danger-*` | Failed/canceled state, delete buttons, error toasts |

**Dark mode**: All colors have dark variants under `:root[data-color-mode="dark"]`. Never define foreground colors without a corresponding background.

### Typography

| Token | Value | Use |
|---|---|---|
| `--font-body` | Inter, SF Pro Text, Segoe UI, system-ui, sans-serif | Default text |
| `--font-display` | Inter, SF Pro Text, Segoe UI, system-ui, sans-serif | Headings |
| `--font-mono` | SF Mono, Cascadia Code, Consolas, monospace | Numbers, speeds, sizes |
| `--font-weight-display` | `600` | Display heading weight |
| `--font-weight-semibold` | `600` | Button labels, stat values |
| `--font-size-micro` | `0.75rem` | Table header labels |
| `--font-size-label` | `0.8rem` | Badges, section kickers |
| `--font-size-small` | `0.875rem` | Form hints, secondary text |
| `--font-size-body` | `1rem` | Body text (default) |
| `--font-size-metric` | `1.25rem` | Stat values |
| `--font-size-hero` | `1.8rem` | Large hero headings |
| `--line-height-display` | `1.2` | Headings |
| `--line-height-tight` | `1.4` | Compact text |
| `--letter-spacing-tight` | `-0.02em` | Headings |
| `--letter-spacing-wide` | `0.04em` | Uppercase kickers |

### Spacing

Use the spacing scale. Prefer `var(--space-N)` in CSS, `gap-2` / `p-3` etc. in UnoCSS.

| Token | rem | px ~ |
|---|---|---|
| `--space-1` | 0.25 | 4 |
| `--space-2` | 0.5 | 8 |
| `--space-3` | 0.75 | 12 |
| `--space-4` | 1.0 | 16 |
| `--space-5` | 1.5 | 24 |
| `--space-6` | 2.0 | 32 |
| `--space-7` | 2.5 | 40 |

### Radii

| Token | Use |
|---|---|
| `--radius-sm` (`0.25rem`) | Small elements (badges) |
| `--radius-md` (`0.5rem`) | Inputs, buttons, dialogs |
| `--radius-lg` (`0.75rem`) | Cards, panels |
| `--radius-xl` (`0.75rem`) | Modal overlay panels |
| `--radius-pill` (`999px`) | Switches, progress bars, pill buttons |

### Shadows

| Token | Use |
|---|---|
| `--shadow-soft` | Subtle elevation (switch thumb) |
| `--shadow-card` | Default card/panel elevation |
| `--shadow-card-hover` | Elevated card/panel on hover, dialogs |
| `--shadow-accent` | Alias for `--shadow-card` |
| `--shadow-accent-soft` | Alias for `--shadow-soft` |

### Transitions

Duration: `var(--duration-fast)` = `150ms`. Use `0.2s ease` for most interactive transitions.

## Icon System

All icons come from Remix Icon via UnoCSS `presetIcons`. Use the `i-ri-*` class pattern:

```html
<span class="i-ri-download-line" aria-hidden="true" />
```

**Convention**: Always add `aria-hidden="true"` to decorative icons. Use semantic elements (`<button>`, `<i>`) as appropriate.

**Common icons used in the project**:
- Navigation: `i-ri-arrow-down-s-line`, `i-ri-arrow-up-line`, `i-ri-arrow-right-s-line`
- Actions: `i-ri-add-line`, `i-ri-close-line`, `i-ri-refresh-line`, `i-ri-delete-bin-line`
- Download states: `i-ri-download-line`, `i-ri-pause-line`, `i-ri-play-line`, `i-ri-stop-line`, `i-ri-close-circle-line`
- Status: `i-ri-checkbox-circle-line`, `i-ri-error-warning-line`, `i-ri-information-line`, `i-ri-alert-line`
- Features: `i-ri-flashlight-fill` (CDN), `i-ri-settings-3-line`, `i-ri-palette-line`

Search Remix Icon: https://remixicon.com/

## Component Patterns

### Form Fields

Each form field follows this structure:
```
[Label] → [Input/Select/Switch] → [Hint text (optional)]
```

Use `SettingsField` component in settings panels. In other contexts, use the `settings-field` CSS class pattern directly:
```html
<label class="settings-field">
  <span class="settings-field__label">Label</span>
  <UiTextField v-model="value" />
  <p class="settings-field__hint">Optional hint text</p>
</label>
```

### Card Sections

Use `SettingsSection` for settings/labs panel sections, or `UiCard` for generic cards. Both provide border, background, shadow, hover effect.

**When to use SettingsSection vs UiCard**:
- `SettingsSection`: Settings/labs configuration panels (has title, icon, summary props)
- `UiCard`: Generic content cards with optional header/footer slots

### Dialogs and Overlays

Use `UiDialog` for modal dialogs (confirmation, form prompts). Use `ModalOverlay` for fullscreen overlays (settings page, labs page).

`ConfirmDialog` wraps `UiDialog` with standard confirm/cancel button layout.

### Empty States

Always use `UiEmptyState` for empty containers. Never create ad-hoc empty state markup.

### Data Tables

For simple read-only tables (like peer/tracker lists), use `DataTable`. For complex interactive tables (like the download queue), build a dedicated component.

## Shared Layout Classes

These are non-scoped global classes in `src/styles.css` available everywhere:

| Class | Use |
|---|---|
| `.section-kicker` | Small uppercase muted label above a heading |
| `.panel-title` | Standard panel/section heading style |
| `.status-banner` | Info/error banner with border and padding |
| `.status-banner--info` | Blue info variant |
| `.status-banner--error` | Red error variant |
| `.desk-panel__header` | Flex header with space-between layout |
| `.theme-color-button` | Accent color selector button |

## CSS Conventions

1. **Scoped styles**: Always use `<style scoped>`. Never rely on non-scoped styles except for shared layout classes in page-level components.
2. **CSS variables**: Always reference tokens via `var(--token)`. Never hardcode colors or spacing.
3. **Class naming**: BEM-like: `.component-name__element--modifier` (e.g., `.detail-panel__header`, `.ui-badge--success`).
4. **UnoCSS**: Use utility classes in templates for minor adjustments (`flex`, `gap-2`, `text-sm`). Do not use UnoCSS for primary component styling — keep that in scoped CSS.
5. **Transitions**: Use `<Transition>` + CSS for enter/leave animations. Name transitions after the component (e.g., `overlay-fade`, `dialog-fade`, `toast`).
6. **Focus**: Every interactive element must have `:focus-visible` styling using `var(--color-focus-ring)`.
7. **Disabled**: Use `opacity: 0.5; cursor: not-allowed` consistently.

## Accessibility Minimums

- All interactive elements must be keyboard accessible
- Icons must have `aria-hidden="true"`
- Dialogs/overlays close on Escape and overlay click
- Form fields have visible labels
- Color is never the sole indicator of state (always pair with text or icon)

## When to Create New UI Components

Before creating a new UI component, check:
1. Does a similar pattern already exist in `src/components/ui/`?
2. Is this pattern used in 2+ places?
3. Can it be composed from existing components?

If yes to all three, extract a shared component. If only used once, keep it inline.

## Responsive Design

- Mobile breakpoint: `680px`
- Use `clamp()` / `min()` / `max()` for fluid sizing
- Dialogs and overlays must fit within `calc(100vh - 1.5rem)` and `calc(100vw - 1.5rem)`
- Test at narrow viewports (Tauri window can be resized arbitrarily)

## Dark Mode

All components must work in both light and dark modes. Test by toggling `data-color-mode="dark"` on `<html>`. The token system handles most variables automatically — just avoid hardcoding colors.

## Theme Accent Colors

Three accent themes available: lime (default), amber, sky. Toggled via `data-theme` attribute. Components that use `--color-accent-*` tokens adapt automatically.
