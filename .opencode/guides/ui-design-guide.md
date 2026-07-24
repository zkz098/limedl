# UI Design Guide — limedl

## 模块职责

设计令牌（Design Tokens）、图标系统、布局类、CSS 约定和可访问性标准的定义。所有视觉相关的决策在此文档中描述。

## 涉及文件

- `src/styles.css` — 所有 CSS 自定义属性定义（design tokens、全局布局类、过渡动画）
- `src/` 下各 `.vue` SFC 的 `<style scoped>` — 组件级样式
- `vite.config.ts` — UnoCSS 配置（presetUno + presetIcons）

## Design Tokens

所有令牌以 CSS 自定义属性定义在 `src/styles.css` 中，组件中通过 `var(--token)` 引用，严禁硬编码值。

### 颜色

背景层：`--color-bg-base`（页面背景）、`--color-panel`（卡片/面板背景）、`--color-panel-muted`（次级面板）、`--color-surface-muted`（表头/悬停背景）、`--color-surface-hover`（强悬停）。

文字层：`--color-heading`（标题）、`--color-text-main`（正文）、`--color-text-muted`（次要文字）、`--color-text-soft`（占位符）。

控件：`--color-input-bg`、`--color-border`、`--color-border-strong`、`--border-width-thin`。

强调色系（--color-accent-\*）：主色、悬停、背景、边框、替代色、对比色、聚焦环、进度条轨道。默认为 green-lime，可通过 `data-theme` 属性切换为 amber / sky。

语义色系（--color-info-_ / --color-success-_ / --color-warning-_ / --color-danger-_）：各含 `-bg` 背景、`-border` 边框、`-text` 文字三个变体。

暗色模式：所有颜色在 `:root[data-color-mode="dark"]` 下有对应变体。

### 排版

字体：`--font-body` / `--font-display`（Inter Fallback 栈）、`--font-mono`（等宽字体）。字号从 `--font-size-micro`(0.75rem) 到 `--font-size-hero`(1.8rem)。字重 `--font-weight-display` 和 `--font-weight-semibold` 均为 600。

### 间距

间距标尺：`--space-1`(0.25rem/4px) 到 `--space-7`(2.5rem/40px)。CSS 中使用 `var(--space-N)`，UnoCSS 中使用 `gap-2`、`p-3` 等。

### 圆角

`--radius-sm`(0.25rem, 徽标)、`--radius-md`(0.5rem, 输入框/按钮)、`--radius-lg`(0.75rem, 卡片)、`--radius-xl`(0.75rem, 模态面板)、`--radius-pill`(999px, 开关/进度条)。

### 阴影

`--shadow-soft` / `--shadow-card` / `--shadow-card-hover`。开关滑块用 soft，卡片用 card，弹窗用 card-hover。

### 过渡动画

默认时长 `--duration-fast`(150ms)。交互过渡使用 `0.2s ease`。

## 图标系统

所有图标来自 Remix Icon，通过 UnoCSS presetIcons 加载。使用 `i-ri-*` class 模式：`<span class="i-ri-download-line" aria-hidden="true" />`。装饰性图标必须加 `aria-hidden="true"`。

## 全局布局类

定义在 `src/styles.css`：`.section-kicker`（小号大写标签）、`.panel-title`（面板标题）、`.status-banner`（信息/错误横幅）、`.desk-panel__header`（flex space-between 布局）、`.theme-color-button`（强调色选择按钮）。

## CSS 约定

- 作用域样式：始终使用 `<style scoped>`，非作用域样式仅用于页面级组件的共享布局类。
- CSS 变量：始终通过 `var(--token)` 引用，不硬编码颜色或间距。
- 类命名：BEM-like 风格（`.component-name__element--modifier`）。
- UnoCSS：模板中用于微调（flex、gap-2、text-sm），组件主样式使用 scoped CSS。
- 过渡：使用 `<Transition>` + CSS，命名以组件为准（如 dialog-fade）。
- 焦点：所有交互元素必须有 `:focus-visible` 样式，使用 `var(--color-focus-ring)`。
- 禁用态：`opacity: 0.5; cursor: not-allowed` 一致使用。

## 可访问性

所有交互元素必须键盘可达；图标必须 `aria-hidden="true"`；弹窗/覆盖层支持 Escape 关闭和覆盖层点击关闭；表单字段有可见标签；颜色不作为状态的唯一指示器。

## 响应式设计

移动断点 680px。使用 `clamp()` / `min()` / `max()` 做流式尺寸。弹窗必须纳入 `calc(100vh - 1.5rem)` 和 `calc(100vw - 1.5rem)`。Tauri 窗口可任意调整大小，需在窄视口下测试。

## 数据流向

```
src/styles.css（design tokens 定义）
  ↓
各 .vue SFC 的 <style scoped> → var(--token) 引用
  ├─ 组件内部样式（scoped CSS）
  ├─ UnoCSS 工具类（模板中微调，不覆盖主样式）
  └─ 全局布局类（页面级组件的非 scoped 样式）
  ↓
浏览器渲染 → 设计令牌驱动所有可视属性（颜色、间距、圆角、阴影、字体）
```

强调色切换：`data-theme` 属性（lime / amber / sky）→ `--color-accent-*` 变体 → 组件自动适配。
暗色模式：`data-color-mode="dark"` → 各 color token 暗色变体 → 组件自动适配。

## 设计决策与约定

- 所有颜色、间距、圆角、阴影、字体以 CSS 自定义属性定义，严禁硬编码值。添加新组件时先在 `src/styles.css` 检查是否有对应 token，没有则新增。
- 使用 oklch 色彩空间定义颜色值，确保跨亮度和饱和度的感知均匀性。
- 强调色系（`--color-accent-*`）用于按钮、进度条、选中项、开关等交互元素。语义色系（`--color-{info|success|warning|danger}-*`）用于徽标、toast、横幅。
- 图标强制使用 Remix Icon（UnoCSS 的 `i-ri-*`），添加 `aria-hidden="true"`。不使用内联 SVG 或其他图标集。
- 组件样式始终用 `<style scoped>`，非作用域样式仅限页面级组件的共享布局类。类名使用 BEM-like 约定。
- 所有交互元素必须有 `:focus-visible` 样式。禁用态统一 `opacity: 0.5; cursor: not-allowed`。
- 创建新组件前必须检查：是否存在类似模式、是否在 2+ 处使用、能否从现有组件组合。

以上三点都满足时才提取为共享组件。
