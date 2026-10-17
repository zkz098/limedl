use limedl_core::types::ChecksumMode;

/// One task parsed out of the batch textarea.
///
/// A plain line contributes only `url`; an `aria2c` command line also carries
/// `--out`, `--dir`, `--header`, `--user-agent`, `--checksum` and, when it
/// names several targets, the mirror list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchEntry {
    pub url: String,
    pub mirror_urls: Vec<String>,
    pub file_name: Option<String>,
    pub dir: Option<String>,
    pub headers: Vec<String>,
    pub user_agent: Option<String>,
    pub checksum: Option<(ChecksumMode, String)>,
}

impl BatchEntry {
    fn plain(url: String) -> Self {
        Self {
            url,
            mirror_urls: Vec::new(),
            file_name: None,
            dir: None,
            headers: Vec::new(),
            user_agent: None,
            checksum: None,
        }
    }
}

/// Parse batch text into tasks. Blank lines and lines starting with `#` are
/// skipped; a line may be either a plain URL or a full `aria2c` command.
pub fn parse_batch_entries(text: &str) -> Vec<BatchEntry> {
    let mut entries = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match crate::aria2_cli::parse(trimmed) {
            Some(command) => entries.extend(entries_from_aria2(&command)),
            // An `aria2c` line with no download target (a stray `--help`) is a
            // malformed command, not a URL to fetch.
            None if crate::aria2_cli::looks_like_aria2_command(trimmed) => {}
            None => {
                for url in expand_url_ranges(trimmed) {
                    entries.push(BatchEntry::plain(url));
                }
            }
        }
    }
    entries
}

/// Turn one parsed aria2 command into batch tasks.
///
/// `aria2c url1 url2` is one download from two mirrors, while `-Z` (and the
/// limedl-only `[01-20]` range expansion) means separate downloads. `--out` is
/// only kept when the command yields exactly one task, since two tasks cannot
/// share a file name.
fn entries_from_aria2(command: &crate::aria2_cli::Aria2Command) -> Vec<BatchEntry> {
    let expanded: Vec<String> = command
        .urls
        .iter()
        .flat_map(|url| expand_url_ranges(url))
        .collect();
    if expanded.is_empty() {
        return Vec::new();
    }

    let separate = command.force_sequential || expanded.len() != command.urls.len();
    let file_name = (expanded.len() == 1).then(|| command.out.clone()).flatten();
    let make = |url: String, mirror_urls: Vec<String>, file_name: Option<String>| BatchEntry {
        url,
        mirror_urls,
        file_name,
        dir: command.dir.clone(),
        headers: command.headers.clone(),
        user_agent: command.user_agent.clone(),
        checksum: command.checksum.clone(),
    };

    if separate {
        expanded
            .into_iter()
            .map(|url| make(url, Vec::new(), file_name.clone()))
            .collect()
    } else {
        let mut urls = expanded.into_iter();
        let url = urls.next().unwrap_or_default();
        vec![make(url, urls.collect(), file_name)]
    }
}

/// Expand the first `[start-end]` numeric range in a URL (first occurrence
/// only, zero-padded to the width of the start token). A safety cap of 1000
/// expansions guards against accidental giant ranges.
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
/// segment of an HTTP(S) URL.
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
        if bytes[i] == b'%'
            && i + 3 <= bytes.len()
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
        // Only the first range expands
        assert_eq!(
            expand_url_ranges("https://host/a[1-2]b[3-4].zip"),
            vec!["https://host/a1b[3-4].zip", "https://host/a2b[3-4].zip"]
        );
        // Non-range inputs pass through unchanged
        assert_eq!(
            expand_url_ranges("https://host/a.zip"),
            vec!["https://host/a.zip"]
        );
        assert_eq!(
            expand_url_ranges("https://host/a[-1].zip"),
            vec!["https://host/a[-1].zip"]
        );
        assert_eq!(
            expand_url_ranges("https://host/a[x-y].zip"),
            vec!["https://host/a[x-y].zip"]
        );
        // Reversed range: no expansion
        assert_eq!(
            expand_url_ranges("https://host/a[3-1].zip"),
            vec!["https://host/a[3-1].zip"]
        );
        // Giant range guard
        assert_eq!(
            expand_url_ranges("https://host/a[1-1001].zip"),
            vec!["https://host/a[1-1001].zip"]
        );
    }

    #[test]
    fn test_parse_batch_urls() {
        let text = "\
https://host/a.zip\n\
\n\
# comment line\n\
  https://host/b[1-2].zip  \n\
magnet:?xt=urn:btih:abcdef\n";
        let urls: Vec<String> = parse_batch_entries(text)
            .into_iter()
            .map(|entry| entry.url)
            .collect();
        assert_eq!(
            urls,
            vec![
                "https://host/a.zip",
                "https://host/b1.zip",
                "https://host/b2.zip",
                "magnet:?xt=urn:btih:abcdef",
            ]
        );
        assert!(parse_batch_entries("# only comments\n\n").is_empty());
    }

    #[test]
    fn test_parse_batch_entries_understands_aria2_commands() {
        let text = "\
https://host/a.zip\n\
aria2c \"https://host/b.zip\" --out \"b.zip\" --header \"Cookie: a=1\" --dir /out\n";
        let entries = parse_batch_entries(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url, "https://host/a.zip");
        assert!(entries[0].file_name.is_none());
        assert_eq!(entries[1].url, "https://host/b.zip");
        assert_eq!(entries[1].file_name.as_deref(), Some("b.zip"));
        assert_eq!(entries[1].headers, vec!["Cookie: a=1".to_string()]);
        assert_eq!(entries[1].dir.as_deref(), Some("/out"));

        // An `aria2c` line with no target is a malformed command, not a URL.
        assert!(parse_batch_entries("aria2c --help").is_empty());
    }

    #[test]
    fn test_aria2_mirrors_versus_sequential() {
        // Without -Z the extra targets are mirrors of one download.
        let mirrors = parse_batch_entries("aria2c https://host/a.zip https://mirror/a.zip");
        assert_eq!(mirrors.len(), 1);
        assert_eq!(mirrors[0].url, "https://host/a.zip");
        assert_eq!(mirrors[0].mirror_urls, vec!["https://mirror/a.zip"]);

        // With -Z each target is its own task.
        let sequential = parse_batch_entries("aria2c -Z https://host/a.zip https://host/b.zip");
        assert_eq!(sequential.len(), 2);
        assert!(sequential.iter().all(|e| e.mirror_urls.is_empty()));
    }

    #[test]
    fn test_aria2_range_expansion_makes_separate_tasks() {
        let entries = parse_batch_entries("aria2c \"https://host/f[01-02].zip\" --out f.zip");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url, "https://host/f01.zip");
        assert_eq!(entries[1].url, "https://host/f02.zip");
        assert!(
            entries.iter().all(|e| e.file_name.is_none()),
            "two tasks cannot share --out"
        );
    }

    #[test]
    fn test_extract_batch_file_name() {
        assert_eq!(
            extract_batch_file_name("https://host.com/path/to/file%20name.zip").as_deref(),
            Some("file name.zip")
        );
        assert_eq!(
            extract_batch_file_name("https://host.com/dir/").as_deref(),
            None
        );
        assert_eq!(
            extract_batch_file_name("magnet:?xt=urn:btih:ab").as_deref(),
            None
        );
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
