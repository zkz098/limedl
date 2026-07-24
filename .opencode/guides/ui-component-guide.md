# UI Component Guide — limedl

## 模块职责

共享 UI 组件的完整目录。所有组件路径相对于 `src/components/`。

## 涉及文件

** 基础组件 `src/components/ui/` **：

- `UiBadge.vue` — 状态/类别标签。Props: size(sm|md), tone(neutral|info|success|warning|danger)。使用 `toneForState()` helper 映射下载状态到 badge 色调。
- `UiButton.vue` — 操作按钮。Props: type, variant(primary|secondary|ghost|danger), size(sm|md), loading, disabled, icon, iconRight, block。Emits: click。
- `UiCard.vue` — 通用卡片容器。Slots: header, default, footer。
- `UiDialog.vue` — 模态对话框。Props: modelValue, title, width, closeOnOverlay。Emits: update:modelValue。Slots: title, default。
- `InfoTooltip.vue` — 信息图标悬停提示。Props: text。使用 `@floating-ui/dom` 定位，支持移动端 tap 切换。
- `UiEmptyState.vue` — 空容器占位。Props: icon, title, description。所有空状态必须使用此组件。
- `UiProgress.vue` — 进度条。Props: value(0-100), indeterminate, showLabel, label。尊重 prefers-reduced-motion。
- `UiSelect.vue` — 下拉选择。Props: modelValue, options, disabled, placeholder。Emits: update:modelValue。支持键盘导航、类型查找。
- `UiSwitch.vue` — 开关。Props: modelValue, label, disabled。Emits: update:modelValue。
- `UiTextField.vue` — 统一文本/数字/URL 输入。Props: modelValue, type(text|number|url), placeholder, disabled, min, max, step, unit, unitPosition。Emits: update:modelValue。数字模式无 spin button，空值 emit null。
- `ConfirmDialog.vue` — 确认对话框（基于 UiDialog）。Props: modelValue, kicker, title, message, confirmText, cancelText, confirmVariant, icon, confirmLoading 等。Emits: confirm, cancel。
- `NotificationToast.vue` — Toast 通知栈。Props: notifications(Notification[])。Emits: dismiss。通过 `useNotification()` composable 管理。
- `DataTable.vue` — 只读数据表。Props: columns(DataTableColumn[]), rows, emptyTitle, emptyIcon, rowKey。用于 BT 对等节点/tracker 等简单表格。

** 组合组件 **：

- `SettingsSection.vue` — 设置/实验室面板的卡片式段落。Props: title, icon, summary。自包含视觉样式。
- `SettingsField.vue` — 设置面板的表单项包装。Props: label, hint, wide。依赖 `.settings-field` 父类样式。
- `ModalOverlay.vue` — 全屏覆盖层。Props: modelValue。Emits: close。含背景模糊、关闭按钮、缩放淡入过渡。
- `StatRow.vue` — 键值统计行。Props: label, value, mono。用于侧边栏面板。

** 领域组件 **：

- `DetailPanel.vue` — 选中下载任务的详情面板。Props: selectedOverview, selectedSnapshot, selectedId, canPause, canResume, canCancel 等。Emits: close, refresh, pause, resume, cancel。
- `DownloadInspector.vue` — 下载详情内容体。

** 布局组件 `src/components/layout/` **：

- `TopToolbar.vue` — 顶栏（搜索、排序、视图选项、批量操作、游戏/超频切换）。
- `CategorySidebar.vue` — 左侧分类侧边栏（下载过滤器 + BT 状态）。使用 StatRow。

## 数据流向

```
App.vue → 布局组件 (TopToolbar / CategorySidebar / ModalOverlay)
  ├─ CategorySidebar → StatRow（BT 状态数据）
  ├─ 主内容区 → 页面视图 → 领域组件 → 基础组件
  └─ ModalOverlay → SettingsPage / LabsPage → SettingsSection → SettingsField → 基础组件

组件选择流程图:
  按钮 → UiButton
  状态标签 → UiBadge
  输入框 → UiTextField
  开关 → UiSwitch
  下拉 → UiSelect
  进度条 → UiProgress
  模态对话框 → UiDialog / ConfirmDialog
  全屏覆盖 → ModalOverlay
  内容卡片 → UiCard
  设置段落 → SettingsSection + SettingsField
  空状态 → UiEmptyState
  统计行 → StatRow
  只读表格 → DataTable
  通知 → useNotification() + NotificationToast
  下载详情 → DetailPanel
```

## 设计决策与约定

- 所有基础组件在 `src/components/ui/` 中，组合组件在 `src/components/settings/`、`src/components/layout/`、`src/components/sidebar/`，领域组件在 `src/components/limedl/`。
- 组件 Props 命名采用 camelCase，emit 事件名采用 kebab-case。
- 表单字段结构：[标签] → [输入组件] → [提示文字]。使用 SettingsField 或 `.settings-field` 类。
- SettingsSection 用于设置/实验室面板（有 title/icon/summary props），UiCard 用于通用卡片（slots）。
- 空状态强制使用 UiEmptyState，禁止手写空状态标记。
- UiTextField 统一了重构前的 UiInput / UiNumberField / UiUnitInput 三个组件。
- DataTable 替代了 BtPeerTable 和 BtTrackerTable 中的手写 `<table>` 标记。
