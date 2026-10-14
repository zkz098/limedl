/// Result of parsing text copied to the clipboard for download links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardPayload {
    Empty,
    SingleUrl(String),
    BatchUrls(Vec<String>),
}

/// Parse clipboard text and determine if it represents a single download URL
/// or a multi-line batch of download URLs.
///
/// An `aria2c` command line counts as a download entry too. It is kept verbatim
/// (surrounding double quotes are *not* stripped) because trimming them would
/// cut the closing quote off the last option.
pub fn parse_clipboard_download_text(text: &str) -> ClipboardPayload {
    let mut valid_links: Vec<String> = Vec::new();

    let is_download_link = |s: &str| {
        s.starts_with("http://")
            || s.starts_with("https://")
            || s.starts_with("magnet:?")
            || (s.contains('[') && s.contains(']') && (s.contains("http://") || s.contains("https://")))
    };

    for raw in text.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if crate::aria2_cli::looks_like_aria2_command(trimmed) {
            // Only a command that actually names a target is worth handing on;
            // `aria2c --help` on the clipboard is not a download.
            if crate::aria2_cli::parse(trimmed).is_some() {
                valid_links.push(trimmed.to_string());
            }
            continue;
        }
        let line = trimmed.trim_matches('"').trim_matches('\'').trim();
        if is_download_link(line) {
            valid_links.push(line.to_string());
        }
    }

    if valid_links.is_empty() {
        ClipboardPayload::Empty
    } else if valid_links.len() == 1 {
        ClipboardPayload::SingleUrl(valid_links[0].clone())
    } else {
        ClipboardPayload::BatchUrls(valid_links)
    }
}
