/// Split batch text into expanded download URLs. Blank lines and lines
/// starting with `#` are skipped; `[01-20]` style ranges are expanded.
pub fn parse_batch_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        urls.extend(expand_url_ranges(trimmed));
    }
    urls
}

/// Expand the first `[start-end]` numeric range in a URL (same semantics as
/// the Vue composer's `expandUrlRanges`: first occurrence only, zero-padded
/// to the width of the start token). A safety cap of 1000 expansions guards
/// against accidental giant ranges.
pub fn expand_url_ranges(url: &str) -> Vec<String> {
    let Some(open) = url.find('[') else {
        return vec![url.to_string()];
    };
    let after_open = &url[open + 1..];
    let Some(close) = after_open.find(']') else {
        return vec![url.to_string()];
    };
    let inner = &after_open[..close];
    let Some((start_raw, end_raw)) = inner.split_once('-') else {
        return vec![url.to_string()];
    };
    if start_raw.is_empty()
        || end_raw.is_empty()
        || !start_raw.chars().all(|c| c.is_ascii_digit())
        || !end_raw.chars().all(|c| c.is_ascii_digit())
    {
        return vec![url.to_string()];
    }
    let Ok(start) = start_raw.parse::<u64>() else {
        return vec![url.to_string()];
    };
    let Ok(end) = end_raw.parse::<u64>() else {
        return vec![url.to_string()];
    };
    if start > end || end - start >= 1000 {
        return vec![url.to_string()];
    }

    let pattern = format!("[{inner}]");
    (start..=end)
        .map(|i| {
            let mut replacement = i.to_string();
            while replacement.len() < start_raw.len() {
                replacement.insert(0, '0');
            }
            url.replacen(&pattern, &replacement, 1)
        })
        .collect()
}

/// Best-effort filename for a batch entry: the last percent-decoded path
/// segment of an HTTP(S) URL (matching the Vue composer's per-entry fileName).
pub fn extract_batch_file_name(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return None;
    }
    let segment = reqwest::Url::parse(trimmed)
        .ok()
        .and_then(|u| {
            u.path_segments()
                .and_then(|mut segments| segments.next_back().map(ToOwned::to_owned))
        })
        .map(|seg| percent_decode(&seg))
        .unwrap_or_default();
    (!segment.is_empty()).then_some(segment)
}

/// Minimal percent-decoding for URL path segments (UTF-8 lossy).
pub fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len()
            && let Ok(value) = u8::from_str_radix(&input[i + 1..i + 3], 16)
        {
            out.push(value);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_url_ranges() {
        assert_eq!(
            expand_url_ranges("https://host/file[01-03].zip"),
            vec![
                "https://host/file01.zip",
                "https://host/file02.zip",
                "https://host/file03.zip",
            ]
        );
        // No padding when the start token is not zero-padded
        assert_eq!(
            expand_url_ranges("https://host/file[1-2].zip"),
            vec!["https://host/file1.zip", "https://host/file2.zip"]
        );
        // Only the first range expands (mirrors the Vue composer)
        assert_eq!(
            expand_url_ranges("https://host/a[1-2]b[3-4].zip"),
            vec!["https://host/a1b[3-4].zip", "https://host/a2b[3-4].zip"]
        );
        // Non-range inputs pass through unchanged
        assert_eq!(expand_url_ranges("https://host/a.zip"), vec!["https://host/a.zip"]);
        assert_eq!(expand_url_ranges("https://host/a[-1].zip"), vec!["https://host/a[-1].zip"]);
        assert_eq!(expand_url_ranges("https://host/a[x-y].zip"), vec!["https://host/a[x-y].zip"]);
        // Reversed range: no expansion
        assert_eq!(expand_url_ranges("https://host/a[3-1].zip"), vec!["https://host/a[3-1].zip"]);
        // Giant range guard
        assert_eq!(expand_url_ranges("https://host/a[1-1001].zip"), vec!["https://host/a[1-1001].zip"]);
    }

    #[test]
    fn test_parse_batch_urls() {
        let text = "\
https://host/a.zip\n\
\n\
# comment line\n\
  https://host/b[1-2].zip  \n\
magnet:?xt=urn:btih:abcdef\n";
        assert_eq!(
            parse_batch_urls(text),
            vec![
                "https://host/a.zip",
                "https://host/b1.zip",
                "https://host/b2.zip",
                "magnet:?xt=urn:btih:abcdef",
            ]
        );
        assert!(parse_batch_urls("# only comments\n\n").is_empty());
    }

    #[test]
    fn test_extract_batch_file_name() {
        assert_eq!(
            extract_batch_file_name("https://host.com/path/to/file%20name.zip").as_deref(),
            Some("file name.zip")
        );
        assert_eq!(extract_batch_file_name("https://host.com/dir/").as_deref(), None);
        assert_eq!(extract_batch_file_name("magnet:?xt=urn:btih:ab").as_deref(), None);
        assert_eq!(extract_batch_file_name("not a url").as_deref(), None);
    }

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("a%20b+c"), "a b+c"); // '+' is not decoded in paths
        assert_eq!(percent_decode("a+b"), "a+b");
        assert_eq!(percent_decode("%E4%B8%AD%E6%96%87.zip"), "中文.zip");
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode("bad%zz"), "bad%zz");
        assert_eq!(percent_decode("trunc%2"), "trunc%2");
    }
}
