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

/// Decode the C-style escapes gettext and Slint/Rust literals use.
fn unescape_c_escapes(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
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
                current_id = Some(unescape_c_escapes(s));
                mode = Some("id");
            }
        } else if let Some(rest) = trimmed.strip_prefix("msgstr \"") {
            if let Some(s) = rest.strip_suffix('"') {
                if let Some(id) = &current_id {
                    entries.insert(id.clone(), unescape_c_escapes(s));
                }
                mode = Some("str");
            }
        } else if let Some(stripped) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            let val = unescape_c_escapes(stripped);
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
                results.push(unescape_c_escapes(msg));
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

/// Visit every string literal in `text`, skipping comments and char literals.
///
/// `callee` is the identifier immediately before the opening parenthesis when
/// the literal is a call argument (`tr("…")`). Whitespace — including newlines
/// — between the callee, `(` and the literal is ignored, so the multi-line
/// `tr_fmt(\n "…", …)` form is not silently exempted from the checks.
fn visit_string_literals(text: &str, mut visit: impl FnMut(&str, Option<&str>)) {
    let chars: Vec<char> = text.chars().collect();
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

        // Read the string literal, keeping the escapes for the shared decoder.
        let mut j = i + 1;
        let mut raw = String::new();
        while j < chars.len() {
            match chars[j] {
                '\\' => {
                    raw.push('\\');
                    if let Some(next) = chars.get(j + 1) {
                        raw.push(*next);
                        j += 2;
                    } else {
                        j += 1;
                    }
                }
                '"' => break,
                other => {
                    raw.push(other);
                    j += 1;
                }
            }
        }

        // Identifier immediately before the opening parenthesis, if any.
        let mut k = i;
        while k > 0 && chars[k - 1].is_whitespace() {
            k -= 1;
        }
        let callee = if k > 0 && chars[k - 1] == '(' {
            let mut m = k - 1;
            while m > 0 && chars[m - 1].is_whitespace() {
                m -= 1;
            }
            let mut ident = String::new();
            while m > 0 && (chars[m - 1].is_alphanumeric() || chars[m - 1] == '_') {
                ident.insert(0, chars[m - 1]);
                m -= 1;
            }
            (!ident.is_empty()).then_some(ident)
        } else {
            None
        };

        visit(&unescape_c_escapes(&raw), callee.as_deref());
        i = j + 1;
    }
}

/// Every `msgid` literal that is the first argument of an `i18n::tr` /
/// `i18n::tr_fmt` call.
///
/// The module docs and this very file mention `tr("…")` as an example, and
/// `@tr(` appears in the Slint scan above; comments and non-key literals are
/// skipped by the shared state machine.
fn extract_tr_literals(text: &str) -> Vec<String> {
    let mut results = Vec::new();
    visit_string_literals(text, |content, callee| {
        if matches!(callee, Some("tr" | "tr_fmt")) {
            results.push(content.to_string());
        }
    });
    results
}

/// Every string literal in a file. Used for the dynamic-key tables, where the
/// key is passed to `tr` through a variable.
fn extract_all_string_literals(text: &str) -> Vec<String> {
    let mut results = Vec::new();
    visit_string_literals(text, |content, _| results.push(content.to_string()));
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

/// A catalog entry nothing references is dead weight: it ships in every build,
/// gets translated into every locale, and is no longer reachable.
///
/// Keys resolved through a variable — `SettingsField::msgid` and the priority
/// label — cannot be seen by the `tr("…")` literal scan, so every string
/// literal under `src/i18n/` except the generated catalog and this test file is
/// treated as a possible key. That over-approximation can only hide an orphan;
/// it cannot mark a live key as dead.
#[test]
fn every_catalog_entry_is_referenced() {
    let base_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut used = std::collections::HashSet::new();

    let mut slint_files = Vec::new();
    collect_slint_files(&base_dir.join("ui"), &mut slint_files);
    for file in &slint_files {
        let content = std::fs::read_to_string(file).expect("read slint file");
        used.extend(extract_tr_strings(&content));
    }

    let mut rust_files = Vec::new();
    collect_rs_files(&base_dir.join("src"), &mut rust_files);
    for file in &rust_files {
        let content = std::fs::read_to_string(file).expect("read rust file");
        used.extend(extract_tr_literals(&content));
    }

    // The dynamic-key tables and the local three-language tables live here.
    let mut i18n_files = Vec::new();
    collect_rs_files(&base_dir.join("src/i18n"), &mut i18n_files);
    for file in &i18n_files {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name == "catalog.rs" || name == "tests.rs" {
            continue;
        }
        let content = std::fs::read_to_string(file).expect("read i18n source");
        used.extend(extract_all_string_literals(&content));
    }

    let mut orphans: Vec<&str> = super::catalog::CATALOG
        .iter()
        .map(|entry| entry.msgid)
        .filter(|msgid| !used.contains(*msgid))
        .collect();
    orphans.sort_unstable();
    assert!(
        orphans.is_empty(),
        "catalog entries no longer referenced from .slint or Rust (remove them from the \
         .po files and run `cargo xtask i18n generate`): {orphans:#?}"
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

#[test]
fn test_toast_formatters_are_localized_and_carry_details() {
    for lang in [Language::ZhCn, Language::ZhTw, Language::EnUs] {
        let added = format_toast_task_added("file.bin", lang);
        assert!(added.contains("file.bin"), "task name missing for {lang:?}");

        let failed = format_toast_task_add_failed("boom", lang);
        assert!(failed.contains("boom"), "error missing for {lang:?}");

        assert!(!format_toast_batch_done(1, 3, lang).is_empty());
        assert!(!format_toast_settings_saved(lang).is_empty());
        assert!(format_toast_settings_save_failed("disk full", lang).contains("disk full"));
        assert!(format_toast_settings_invalid("bad value", lang).contains("bad value"));
        assert!(!format_toast_setup_finished(lang).is_empty());
        assert!(!format_toast_legacy_bt_resume(3, lang).is_empty());
        assert!(!format_toast_tracker_synced(2, lang).is_empty());
        assert!(format_toast_tracker_sync_failed("timeout", lang).contains("timeout"));
        assert!(!format_toast_link_copied(lang).is_empty());
        assert!(!format_toast_filename_copied(lang).is_empty());
        assert!(!format_toast_labs_saved(lang).is_empty());
        assert!(format_toast_labs_save_failed("readonly", lang).contains("readonly"));
        assert!(format_toast_cdn_applied("1.2.3.4", lang).contains("1.2.3.4"));
        assert!(format_toast_cdn_test_done(Some("1.2.3.4"), lang).contains("1.2.3.4"));
        assert!(!format_toast_cdn_test_done(None, lang).is_empty());
        assert!(format_toast_cdn_test_failed("refused", lang).contains("refused"));
        assert!(format_toast_aria2_rpc_started(6800, lang).contains("6800"));
        assert!(format_toast_aria2_token_failed("missing", lang).contains("missing"));
        assert!(format_toast_autostart_failed("denied", lang).contains("denied"));
        assert!(format_toast_priority_failed("bad", lang).contains("bad"));
        assert!(!format_toast_bt_files_keep_one(lang).is_empty());
        assert!(format_toast_bt_files_failed("io", lang).contains("io"));
        assert!(!format_toast_clear_completed(2, lang).is_empty());
        assert!(!format_toast_clear_completed_none(lang).is_empty());
        assert!(format_toast_factory_reset_failed("locked", lang).contains("locked"));
        assert!(format_toast_update_downloading("9.9.9", lang).contains("9.9.9"));
        assert!(format_toast_update_ready("9.9.9", lang).contains("9.9.9"));
        assert!(format_toast_update_failed("offline", lang).contains("offline"));
        assert!(format_toast_update_available("9.9.9", lang).contains("9.9.9"));
        assert!(format_toast_update_check_failed("offline", lang).contains("offline"));
        assert!(format_toast_update_restart_failed("denied", lang).contains("denied"));
        assert!(!format_toast_invalid_checksum(lang).is_empty());
        assert!(format_toast_integrity_checking("file.bin", lang).contains("file.bin"));
        assert!(format_toast_integrity_passed("file.bin", "sha256", lang).contains("file.bin"));
        assert!(format_toast_integrity_mismatch("file.bin", "sha256", lang).contains("file.bin"));
        assert!(format_toast_integrity_computed("file.bin", "sha256", lang).contains("file.bin"));
        assert!(format_toast_integrity_failed("corrupt", lang).contains("corrupt"));
    }
}
