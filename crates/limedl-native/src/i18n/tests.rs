use super::*;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
fn test_startup_failure_message_is_localized_and_carries_details() {
    for lang in [Language::ZhCn, Language::ZhTw, Language::EnUs] {
        let (title, body) = format_startup_failure(lang, "db is locked", "/tmp/limedl/crash.log");
        assert!(!title.is_empty(), "missing title for {lang:?}");
        assert!(body.contains("db is locked"), "missing detail for {lang:?}");
        assert!(
            body.contains("/tmp/limedl/crash.log"),
            "missing log path for {lang:?}"
        );
    }
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
fn test_aria2_hint_counts_headers_and_names_ignored_options() {
    let ignored = vec!["--load-cookies".to_string()];
    let en = format_aria2_hint(2, 1, &ignored, Language::EnUs);
    assert!(en.contains("2 links"), "{en}");
    assert!(en.contains("1 header"), "{en}");
    assert!(en.contains("--load-cookies"), "{en}");
    assert!(
        !en.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
        "no CJK may leak into the English hint: {en}"
    );

    // Singular English and a clean command (nothing ignored).
    let en_one = format_aria2_hint(1, 0, &[], Language::EnUs);
    assert!(en_one.contains("1 link") && !en_one.contains("links"));
    assert!(!en_one.contains("ignored"));

    assert!(!format_aria2_hint(1, 2, &[], Language::ZhCn).is_empty());
    assert!(!format_aria2_hint(1, 2, &[], Language::ZhTw).is_empty());
}

/// Parse a gettext `.po` catalog into `msgid -> msgstr` entries.
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
        } else if let Some(stripped) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            let val = stripped.replace("\\\"", "\"");
            match mode {
                Some("id") => {
                    if let Some(id) = &mut current_id {
                        id.push_str(&val);
                    }
                }
                Some("str") => {
                    if let Some(str_val) = current_id.as_ref().and_then(|id| entries.get_mut(id)) {
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

/// Recursively collect `.slint` files below `dir`.
fn collect_slint_files(dir: &Path, list: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_slint_files(&p, list);
            } else if p.extension().is_some_and(|ext| ext == "slint") {
                list.push(p);
            }
        }
    }
}

/// Extract every literal passed to `@tr(...)` in a `.slint` source.
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

/// `(file, msgid)` pairs from `files` whose `msgid` is missing from `catalog`.
fn missing_translations(
    catalog: &HashMap<String, String>,
    files: &[PathBuf],
) -> Vec<(PathBuf, String)> {
    let mut missing = Vec::new();
    for file in files {
        let content = std::fs::read_to_string(file).expect("read slint file");
        for msgid in extract_tr_strings(&content) {
            if !catalog.contains_key(&msgid) {
                missing.push((file.clone(), msgid));
            }
        }
    }
    missing
}

#[test]
fn test_all_slint_tr_strings_in_po_catalogs() {
    let base_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let catalogs = [
        (
            "zh_CN",
            base_dir.join("lang/zh_CN/LC_MESSAGES/limedl-native.po"),
        ),
        (
            "zh_TW",
            base_dir.join("lang/zh_TW/LC_MESSAGES/limedl-native.po"),
        ),
        ("en", base_dir.join("lang/en/LC_MESSAGES/limedl-native.po")),
    ];

    let mut slint_files = Vec::new();
    collect_slint_files(&base_dir.join("ui"), &mut slint_files);

    for (name, path) in &catalogs {
        let missing = missing_translations(&parse_po(path), &slint_files);
        assert!(
            missing.is_empty(),
            "Missing translations in {name} PO catalog: {missing:#?}"
        );
    }
}

/// Recursively collect `.rs` files below `dir`.
fn collect_rs_files(dir: &Path, list: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_rs_files(&p, list);
            } else if p.extension().is_some_and(|ext| ext == "rs") {
                list.push(p);
            }
        }
    }
}

/// Every `msgid` literal that is the first argument of an `i18n::tr` /
/// `i18n::tr_fmt` call.
///
/// Comments and string literals are skipped by a small state machine: the module
/// docs and this very file mention `tr("…")` as an example, and `@tr(` appears in
/// the Slint scan above. The callee name is read back from the source so a call
/// like `attr("…")` is not mistaken for `tr("…")`.
fn extract_tr_literals(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut results = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        if c == '\'' {
            // A char literal is three characters (`'x'`); anything else is a
            // lifetime or a stray quote and is skipped one character at a time.
            i += if chars.get(i + 2) == Some(&'\'') {
                3
            } else {
                1
            };
            continue;
        }
        if c != '"' {
            i += 1;
            continue;
        }

        // Read the string literal.
        let mut j = i + 1;
        let mut content = String::new();
        while j < chars.len() {
            match chars[j] {
                '\\' => {
                    if let Some(escaped) = chars.get(j + 1) {
                        content.push(*escaped);
                        j += 2;
                    } else {
                        j += 1;
                    }
                }
                '"' => break,
                other => {
                    content.push(other);
                    j += 1;
                }
            }
        }

        // Only a `tr(`/`tr_fmt(` call directly before the literal counts.
        if chars.get(i.wrapping_sub(1)) == Some(&'(') {
            let mut m = i - 1;
            let mut callee = String::new();
            while m > 0 && (chars[m - 1].is_alphanumeric() || chars[m - 1] == '_') {
                callee.insert(0, chars[m - 1]);
                m -= 1;
            }
            if callee == "tr" || callee == "tr_fmt" {
                results.push(content);
            }
        }
        i = j + 1;
    }
    results
}

/// A `tr`/`tr_fmt` key that is not in the generated catalog would silently
/// render English forever; fail instead.
#[test]
fn every_rust_message_exists_in_the_catalogs() {
    let base_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut rust_files = Vec::new();
    collect_rs_files(&base_dir.join("src"), &mut rust_files);

    let keys: std::collections::HashSet<&'static str> = super::catalog::CATALOG
        .iter()
        .map(|entry| entry.msgid)
        .collect();

    let mut missing = Vec::new();
    for file in &rust_files {
        let content = std::fs::read_to_string(file).expect("read rust file");
        for msgid in extract_tr_literals(&content) {
            if !keys.contains(msgid.as_str()) {
                missing.push((file.clone(), msgid));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "tr/tr_fmt keys missing from the .po catalogs (run `cargo xtask i18n generate` \
         after adding them): {missing:#?}"
    );
}

/// Every catalog entry has a translation in all three columns, and no Chinese
/// reaches the English column.
///
/// The two language self-names are the only messages whose *source* string is
/// Chinese — they are shown in their own language in the language picker — so
/// they are the only keys allowed to contain CJK.
#[test]
fn catalog_columns_are_complete_and_english_is_cjk_free() {
    const CJK_SOURCE_ALLOWLIST: &[&str] = &["简体中文", "繁體中文"];

    for entry in super::catalog::CATALOG {
        for (index, translation) in entry.translations.iter().enumerate() {
            assert!(
                !translation.is_empty(),
                "empty translation in column {index} for {:?}",
                entry.msgid
            );
        }
        if CJK_SOURCE_ALLOWLIST.contains(&entry.msgid) {
            continue;
        }
        let english = entry.translations[2];
        assert!(
            !english
                .chars()
                .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
            "CJK leaked into the English column of {:?}",
            entry.msgid
        );
    }
}
