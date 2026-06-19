
## Task 14: SettingsCdnAccelerationPanel.vue
- Created 2025-06-19
- Pattern: Follows SettingsAria2RpcPanel.vue structure with SettingsBtPanel.vue toggle pattern
- Toggle: button element with settings-toggle / settings-toggle--active CSS classes (NOT input checkbox)
- Status computed: derived from draft.cdnAcceleration fields (lastError ¡ú error, activeIp+activeSpeedMbps ¡ú ready, else idle)
- Local testing ref for transient UI-only test-in-progress state
- Direct draft mutation (no emit) ¡ª matches existing panel pattern
- All i18n keys under settings.cdnAcceleration.* namespace
- vue-tsc: zero errors in new file (4 pre-existing errors in other files)
