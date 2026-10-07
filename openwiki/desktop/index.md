# Files

- [Native Desktop UI (Slint)](native-ui-architecture.md) - How the limedl Slint desktop client is assembled and driven — the main/ui_boot split, AppContext, the pure bridge mapping layer, handler plumbing, the EventBus subscriber, i18n rules, and platform integration for tray, autostart, single instance, power and window geometry.
- [Self-Update and Distribution Channels](self-update-and-distribution.md) - How the limedl desktop client detects its install channel, verifies and installs hybrid Minisign + ML-DSA-65 signed updates across portable/NSIS/MSIX/macOS/Linux channels, and how the release pipeline signs artifacts and builds the single latest-native.json manifest.
