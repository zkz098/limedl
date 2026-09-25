/// Result of parsing text copied to the clipboard for download links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardPayload {
    Empty,
    SingleUrl(String),
    BatchUrls(Vec<String>),
}

/// Parse clipboard text and determine if it represents a single download URL
/// or a multi-line batch of download URLs.
pub fn parse_clipboard_download_text(text: &str) -> ClipboardPayload {
    let lines: Vec<String> = text
        .lines()
        .map(|l| l.trim().trim_matches('"').trim_matches('\'').trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.is_empty() {
        return ClipboardPayload::Empty;
    }

    let is_download_link = |s: &str| {
        s.starts_with("http://")
            || s.starts_with("https://")
            || s.starts_with("magnet:?")
            || (s.contains('[') && s.contains(']') && (s.contains("http://") || s.contains("https://")))
    };

    let valid_links: Vec<String> = lines.into_iter().filter(|l| is_download_link(l)).collect();

    if valid_links.is_empty() {
        ClipboardPayload::Empty
    } else if valid_links.len() == 1 {
        ClipboardPayload::SingleUrl(valid_links[0].clone())
    } else {
        ClipboardPayload::BatchUrls(valid_links)
    }
}
