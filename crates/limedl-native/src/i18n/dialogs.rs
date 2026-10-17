//! New-task dialog, batch and detected-link strings.

use super::{Language, tr, tr_fmt};

/// Checksum probe status line for the new-task dialog.
/// state: "probing" | "found" | "missing" | "not_http"
pub fn format_probe_status(state: &str, hash: &str, lang: Language) -> String {
    match state {
        "probing" => tr("Detecting checksum...", lang).to_string(),
        "found" => format!("SHA-256: {hash}"),
        "missing" => tr("No checksum file found", lang).to_string(),
        "not_http" => tr(
            "Checksum detection is only available for HTTP links",
            lang,
        )
        .to_string(),
        _ => String::new(),
    }
}

/// Torrent preview section header/status for the new-task dialog.
/// state: "loading" | "error" | "summary"
pub fn format_preview_status(state: &str, detail: &str, lang: Language) -> String {
    match state {
        "loading" => tr("Parsing torrent...", lang).to_string(),
        "error" => tr_fmt("Preview failed: {}", lang, &[&detail]),
        _ => String::new(),
    }
}

/// Torrent file selection summary, e.g. "24 files · 12.3 GB".
pub fn format_preview_summary(file_count: usize, size_text: &str, lang: Language) -> String {
    tr_fmt("{} files · {}", lang, &[&file_count, &size_text])
}

/// "Select at least one file" error shown when all torrent files are unchecked.
pub fn no_files_selected_text(lang: Language) -> &'static str {
    tr("Select at least one file", lang)
}

/// Status line for a pasted `aria2c` command line that was recognized.
///
/// Names the number of links and headers that were applied, plus any option the
/// engine cannot honor (`--load-cookies`, `--input-file`, …) so the user is not
/// left believing a cookie jar or an input file was used.
///
/// English distinguishes "1 link" from "2 links"; the Chinese catalogs hold the
/// same translation for both msgids, so the singular/plural selection is
/// structural here rather than a separate translated unit.
pub fn format_aria2_hint(
    url_count: usize,
    header_count: usize,
    ignored: &[String],
    lang: Language,
) -> String {
    let mut base = if url_count == 1 {
        tr_fmt("Parsed aria2c command: {} link", lang, &[&url_count])
    } else {
        tr_fmt("Parsed aria2c command: {} links", lang, &[&url_count])
    };
    if header_count > 0 {
        base = if header_count == 1 {
            tr_fmt("{} · {} header", lang, &[&base, &header_count])
        } else {
            tr_fmt("{} · {} headers", lang, &[&base, &header_count])
        };
    }
    if ignored.is_empty() {
        return base;
    }
    tr_fmt("{} (ignored: {})", lang, &[&base, &ignored.join(", ")])
}

/// Batch link count label under the batch textarea.
pub fn format_batch_count(count: usize, lang: Language) -> String {
    if count == 0 {
        return tr("No valid links detected", lang).to_string();
    }
    tr_fmt("Will add {} tasks", lang, &[&count])
}

/// Batch submit progress/result line.
pub fn format_batch_status(done: usize, total: usize, lang: Language) -> String {
    if done < total {
        return tr_fmt("Submitting... {}/{}", lang, &[&done, &total]);
    }
    if done == 0 {
        return tr("No valid links detected", lang).to_string();
    }
    tr_fmt("Batch submitted: {}/{} succeeded", lang, &[&done, &total])
}

/// Clipboard monitor toast for a single detected download link.
pub fn format_detected_link(url: &str, lang: Language) -> String {
    tr_fmt("Download link detected: {}", lang, &[&url])
}

/// Clipboard monitor toast for multiple detected download links.
pub fn format_detected_batch(count: usize, lang: Language) -> String {
    tr_fmt("Detected {} batch download links", lang, &[&count])
}

/// Title of the native file picker used to choose a .torrent file.
pub fn pick_torrent_title(lang: Language) -> &'static str {
    tr("Select Torrent File", lang)
}

/// Filter label for the native file picker when choosing a .torrent file.
pub fn pick_torrent_filter(lang: Language) -> &'static str {
    tr("Torrent Files (*.torrent)", lang)
}

/// Title of the native file picker used to choose a download directory.
pub fn pick_download_dir_title(lang: Language) -> &'static str {
    tr("Select Download Folder", lang)
}

/// Title of the native file picker used to choose the log directory.
pub fn pick_log_dir_title(lang: Language) -> &'static str {
    tr("Select Log Folder", lang)
}

/// Title and body of the fatal startup-error dialog.
///
/// Shown before settings are available, so the caller passes the language
/// detected from the OS locale rather than the configured one. `detail` is the
/// underlying error and `log_path` the crash log the same report was written to.
pub fn format_startup_failure(lang: Language, detail: &str, log_path: &str) -> (String, String) {
    let title = tr("limedl could not start", lang);
    let body = tr_fmt(
        "limedl failed to start:\n\n{}\n\nDetails were written to:\n{}",
        lang,
        &[&detail, &log_path],
    );
    (title.to_string(), body)
}
