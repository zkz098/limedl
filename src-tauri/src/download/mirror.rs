use urlencoding::encode as url_encode;

use super::types::{GitHubMirrorSettings, MirrorEntry};

/// Check whether the given URL targets GitHub (github.com, www.github.com, or
/// subdomains like api.github.com).  Case-insensitive host matching.
pub fn is_github_url(url: &str) -> bool {
    let parsed = match reqwest::Url::parse(url) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let host = match parsed.host_str() {
        Some(h) => h,
        None => return false,
    };
    let host = host.to_lowercase();
    host == "github.com" || host == "www.github.com" || host.ends_with(".github.com")
}

/// Return the list of enabled, non-empty mirrors sorted by `order`.
pub fn active_mirrors(settings: &GitHubMirrorSettings) -> Vec<&MirrorEntry> {
    let mut mirrors: Vec<&MirrorEntry> = settings
        .mirrors
        .iter()
        .filter(|m| m.enabled && !m.url.trim().is_empty())
        .collect();
    mirrors.sort_by_key(|m| m.order);
    mirrors
}

/// Produce the list of URLs to try for a download.
///
/// If mirroring is disabled or the URL is not a GitHub URL, returns a single-element
/// vector containing the original URL.  Otherwise returns the mirror URLs (in priority
/// order with the original URL appended as the final fallback).
///
/// Each mirror URL is produced as `{base}/{url_encoded_original}` — trailing slashes
/// on `base` are stripped before joining.
pub fn rewrite(url: &str, settings: &GitHubMirrorSettings) -> Vec<String> {
    if !settings.enabled || !is_github_url(url) {
        return vec![url.to_string()];
    }

    let mirrors = active_mirrors(settings);
    if mirrors.is_empty() {
        return vec![url.to_string()];
    }

    let encoded = url_encode(url);
    let mut result: Vec<String> = mirrors
        .iter()
        .map(|entry| {
            let base = entry.url.trim_end_matches('/');
            format!("{base}/{encoded}")
        })
        .collect();

    // Original URL as the final fallback
    result.push(url.to_string());
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::types::MirrorEntry;

    #[test]
    fn disabled_returns_original() {
        let settings = GitHubMirrorSettings::default(); // enabled = false
        let urls = rewrite("https://github.com/user/repo/releases/v1.0/file.zip", &settings);
        assert_eq!(urls, vec!["https://github.com/user/repo/releases/v1.0/file.zip"]);
    }

    #[test]
    fn non_github_url_returns_original() {
        let settings = GitHubMirrorSettings {
            enabled: true,
            ..Default::default()
        };
        let urls = rewrite("https://example.com/file.zip", &settings);
        assert_eq!(urls, vec!["https://example.com/file.zip"]);
    }

    #[test]
    fn single_mirror() {
        let settings = GitHubMirrorSettings {
            enabled: true,
            mirrors: vec![MirrorEntry {
                url: "https://mirror.example.com".into(),
                enabled: true,
                order: 0,
            }],
        };
        let urls = rewrite("https://github.com/user/repo/releases/v1.0/file.zip", &settings);
        assert_eq!(urls.len(), 2);
        assert_eq!(
            urls[0],
            "https://mirror.example.com/https%3A%2F%2Fgithub.com%2Fuser%2Frepo%2Freleases%2Fv1.0%2Ffile.zip"
        );
        assert_eq!(urls[1], "https://github.com/user/repo/releases/v1.0/file.zip");
    }

    #[test]
    fn multi_mirror_order() {
        let settings = GitHubMirrorSettings {
            enabled: true,
            mirrors: vec![
                MirrorEntry {
                    url: "https://mirror-b.example.com".into(),
                    enabled: true,
                    order: 2,
                },
                MirrorEntry {
                    url: "https://mirror-a.example.com".into(),
                    enabled: true,
                    order: 1,
                },
            ],
        };
        let urls = rewrite("https://github.com/user/repo", &settings);
        assert_eq!(urls.len(), 3);
        assert!(urls[0].contains("mirror-a"));
        assert!(urls[1].contains("mirror-b"));
        assert_eq!(urls[2], "https://github.com/user/repo");
    }

    #[test]
    fn disabled_mirror_skipped() {
        let settings = GitHubMirrorSettings {
            enabled: true,
            mirrors: vec![
                MirrorEntry {
                    url: "https://mirror-enabled.example.com".into(),
                    enabled: true,
                    order: 0,
                },
                MirrorEntry {
                    url: "https://mirror-disabled.example.com".into(),
                    enabled: false,
                    order: 1,
                },
            ],
        };
        let urls = rewrite("https://github.com/user/repo", &settings);
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("mirror-enabled"));
        assert_eq!(urls[1], "https://github.com/user/repo");
    }

    #[test]
    fn trailing_slash_stripped() {
        let settings = GitHubMirrorSettings {
            enabled: true,
            mirrors: vec![MirrorEntry {
                url: "https://mirror.example.com/".into(),
                enabled: true,
                order: 0,
            }],
        };
        let urls = rewrite("https://github.com/user/repo", &settings);
        assert!(!urls[0].contains("//https"));
        assert!(!urls[0].contains("//%"));
        // The expected form: base stripped of trailing / then /encoded
        assert_eq!(
            urls[0],
            "https://mirror.example.com/https%3A%2F%2Fgithub.com%2Fuser%2Frepo"
        );
    }

    #[test]
    fn empty_url_filtered() {
        let settings = GitHubMirrorSettings {
            enabled: true,
            mirrors: vec![
                MirrorEntry {
                    url: "".into(),
                    enabled: true,
                    order: 0,
                },
                MirrorEntry {
                    url: "  ".into(),
                    enabled: true,
                    order: 0,
                },
                MirrorEntry {
                    url: "https://mirror.example.com".into(),
                    enabled: true,
                    order: 0,
                },
            ],
        };
        let urls = rewrite("https://github.com/user/repo", &settings);
        // Only the valid mirror + original fallback
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("mirror.example.com"));
        assert_eq!(urls[1], "https://github.com/user/repo");
    }

    #[test]
    fn www_subdomain_matched() {
        let settings = GitHubMirrorSettings {
            enabled: true,
            mirrors: vec![MirrorEntry {
                url: "https://mirror.example.com".into(),
                enabled: true,
                order: 0,
            }],
        };
        let urls = rewrite("https://www.github.com/user/repo", &settings);
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("mirror.example.com"));
    }

    #[test]
    fn api_subdomain_matched() {
        let settings = GitHubMirrorSettings {
            enabled: true,
            mirrors: vec![MirrorEntry {
                url: "https://mirror.example.com".into(),
                enabled: true,
                order: 0,
            }],
        };
        let urls = rewrite("https://api.github.com/repos/user/repo", &settings);
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("mirror.example.com"));
    }
}
