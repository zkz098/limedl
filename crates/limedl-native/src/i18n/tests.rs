use super::*;

#[test]
fn test_language_parsing() {
    assert_eq!(Language::from_code("zh"), Language::ZhCn);
    assert_eq!(Language::from_code("zh-CN"), Language::ZhCn);
    assert_eq!(Language::from_code("zh_CN"), Language::ZhCn);
    assert_eq!(Language::from_code("zh-TW"), Language::ZhTw);
    assert_eq!(Language::from_code("zh_TW"), Language::ZhTw);
    assert_eq!(Language::from_code("zh-HK"), Language::ZhTw);
    assert_eq!(Language::from_code("en"), Language::EnUs);
    assert_eq!(Language::from_code("en-US"), Language::EnUs);
    assert_eq!(Language::from_code("en_GB"), Language::EnUs);
}

#[test]
fn test_format_eta_localized() {
    assert_eq!(format_eta(Some(45), Language::ZhCn), "剩余 45秒");
    assert_eq!(format_eta(Some(45), Language::ZhTw), "剩餘 45秒");
    assert_eq!(format_eta(Some(45), Language::EnUs), "45s left");
    assert_eq!(format_eta(Some(125), Language::ZhCn), "剩余 2分5秒");
    assert_eq!(format_eta(Some(125), Language::ZhTw), "剩餘 2分5秒");
    assert_eq!(format_eta(Some(125), Language::EnUs), "2m 5s left");
    assert_eq!(format_eta(Some(3665), Language::ZhCn), "剩余 1小时1分");
    assert_eq!(format_eta(Some(3665), Language::ZhTw), "剩餘 1小時1分");
    assert_eq!(format_eta(Some(3665), Language::EnUs), "1h 1m left");
    assert_eq!(format_eta(Some(90000), Language::ZhCn), "剩余 1天1小时");
    assert_eq!(format_eta(Some(90000), Language::ZhTw), "剩餘 1天1小時");
    assert_eq!(format_eta(Some(90000), Language::EnUs), "1d 1h left");
}

#[test]
fn test_state_labels() {
    assert_eq!(
        format_state_label(&DownloadState::Downloading, Language::ZhCn),
        "下载中"
    );
    assert_eq!(
        format_state_label(&DownloadState::Downloading, Language::ZhTw),
        "下載中"
    );
    assert_eq!(
        format_state_label(&DownloadState::Downloading, Language::EnUs),
        "Downloading"
    );
    assert_eq!(
        format_state_label(&DownloadState::Completed, Language::ZhCn),
        "已完成"
    );
    assert_eq!(
        format_state_label(&DownloadState::Completed, Language::ZhTw),
        "已完成"
    );
    assert_eq!(
        format_state_label(&DownloadState::Completed, Language::EnUs),
        "Completed"
    );
}

#[test]
fn test_settings_field_labels_localized() {
    assert_eq!(
        SettingsField::ListenPort.label(Language::ZhCn),
        "BT 监听端口"
    );
    assert_eq!(
        SettingsField::ListenPort.label(Language::ZhTw),
        "BT 監聽連接埠"
    );
    assert_eq!(
        SettingsField::ListenPort.label(Language::EnUs),
        "BT listen port"
    );
    // Every label must be non-empty in all languages.
    let fields = [
        SettingsField::MaxRetries,
        SettingsField::IoBufferLimitMb,
        SettingsField::Aria2Port,
    ];
    for field in fields {
        assert!(!field.label(Language::ZhCn).is_empty());
        assert!(!field.label(Language::ZhTw).is_empty());
        assert!(!field.label(Language::EnUs).is_empty());
    }
}

#[test]
fn test_validation_messages_localized() {
    let zh = format_validation_integer(Language::ZhCn, SettingsField::ListenPort, "abc");
    assert!(zh.contains("BT 监听端口") && zh.contains("'abc'"));
    let zh_tw = format_validation_integer(Language::ZhTw, SettingsField::ListenPort, "abc");
    assert!(zh_tw.contains("BT 監聽連接埠") && zh_tw.contains("'abc'"));
    let en = format_validation_integer(Language::EnUs, SettingsField::ListenPort, "abc");
    assert!(en.contains("BT listen port") && en.contains("'abc'"));
    // No CJK characters may leak into the English message.
    assert!(!en.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)));

    let port_en = format_validation_port(Language::EnUs, SettingsField::Aria2Port, "70000");
    assert!(port_en.contains("0-65535"));
    let required_en = format_proxy_url_required(Language::EnUs);
    assert!(!required_en.is_empty());
    assert!(
        !required_en
            .chars()
            .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
    );
}

#[test]
fn test_tray_menu_strings() {
    let zh = get_tray_strings(Language::ZhCn);
    assert_eq!(zh.show_window, "显示主窗口");
    let zh_tw = get_tray_strings(Language::ZhTw);
    assert_eq!(zh_tw.show_window, "顯示主視窗");
    let en = get_tray_strings(Language::EnUs);
    assert_eq!(en.show_window, "Show Main Window");
    assert!(en.speed_limit_toggle.contains("Speed Limit"));
    assert!(zh.speed_limit_toggle.contains("限速"));
    assert!(zh_tw.speed_limit_toggle.contains("限速"));
}

#[test]
fn test_pick_torrent_filter() {
    assert_eq!(pick_torrent_filter(Language::ZhCn), "种子文件 (*.torrent)");
    assert_eq!(pick_torrent_filter(Language::ZhTw), "種子檔案 (*.torrent)");
    assert_eq!(
        pick_torrent_filter(Language::EnUs),
        "Torrent Files (*.torrent)"
    );
}

#[test]
fn test_all_slint_tr_strings_in_po_catalogs() {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    let base_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ui_dir = base_dir.join("ui");
    let zh_po = base_dir.join("lang/zh_CN/LC_MESSAGES/limedl-native.po");
    let zh_tw_po = base_dir.join("lang/zh_TW/LC_MESSAGES/limedl-native.po");
    let en_po = base_dir.join("lang/en/LC_MESSAGES/limedl-native.po");

    fn parse_po(path: &Path) -> HashMap<String, String> {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read PO file {:?}: {e}", path));
        let mut entries = HashMap::new();
        let mut current_id: Option<String> = None;
        let mut mode = None;

        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("msgid \"") {
                if let Some(s) = rest.strip_suffix('"') {
                    current_id = Some(s.replace("\\\"", "\""));
                    mode = Some("id");
                }
            } else if let Some(rest) = trimmed.strip_prefix("msgstr \"") {
                if let Some(s) = rest.strip_suffix('"') {
                    if let Some(id) = &current_id {
                        entries.insert(id.clone(), s.replace("\\\"", "\""));
                    }
                    mode = Some("str");
                }
            } else if let Some(stripped) =
                trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"'))
            {
                let val = stripped.replace("\\\"", "\"");
                match mode {
                    Some("id") => {
                        if let Some(id) = &mut current_id {
                            id.push_str(&val);
                        }
                    }
                    Some("str") => {
                        if let Some(str_val) =
                            current_id.as_ref().and_then(|id| entries.get_mut(id))
                        {
                            str_val.push_str(&val);
                        }
                    }
                    _ => {}
                }
            } else if trimmed.is_empty() {
                current_id = None;
                mode = None;
            }
        }
        entries
    }

    let zh_entries = parse_po(&zh_po);
    let zh_tw_entries = parse_po(&zh_tw_po);
    let en_entries = parse_po(&en_po);

    let mut slint_files = Vec::new();
    fn collect_slint(dir: &Path, list: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    collect_slint(&p, list);
                } else if p.extension().is_some_and(|ext| ext == "slint") {
                    list.push(p);
                }
            }
        }
    }
    collect_slint(&ui_dir, &mut slint_files);

    fn extract_tr_strings(text: &str) -> Vec<String> {
        let mut results = Vec::new();
        let mut rest = text;
        while let Some(pos) = rest.find("@tr(") {
            rest = &rest[pos + 4..];
            let trimmed = rest.trim_start();
            if let Some(inner) = trimmed.strip_prefix('"') {
                let mut escaped = false;
                let mut end_idx = None;
                for (idx, ch) in inner.char_indices() {
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        end_idx = Some(idx);
                        break;
                    }
                }
                if let Some(idx) = end_idx {
                    let msg = &inner[..idx];
                    results.push(msg.replace("\\\"", "\"").replace("\\n", "\n"));
                    rest = &inner[idx + 1..];
                }
            }
        }
        results
    }

    let mut missing_zh = Vec::new();
    let mut missing_zh_tw = Vec::new();
    let mut missing_en = Vec::new();

    for file in &slint_files {
        let content = std::fs::read_to_string(file).expect("read slint file");
        for msgid in extract_tr_strings(&content) {
            if !zh_entries.contains_key(&msgid) {
                missing_zh.push((file.clone(), msgid.clone()));
            }
            if !zh_tw_entries.contains_key(&msgid) {
                missing_zh_tw.push((file.clone(), msgid.clone()));
            }
            if !en_entries.contains_key(&msgid) {
                missing_en.push((file.clone(), msgid));
            }
        }
    }

    assert!(
        missing_zh.is_empty(),
        "Missing translations in zh_CN PO catalog: {missing_zh:#?}"
    );
    assert!(
        missing_zh_tw.is_empty(),
        "Missing translations in zh_TW PO catalog: {missing_zh_tw:#?}"
    );
    assert!(
        missing_en.is_empty(),
        "Missing translations in en PO catalog: {missing_en:#?}"
    );
}
