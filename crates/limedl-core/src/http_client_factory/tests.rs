use super::*;
use crate::types::{DownloadDefaultsSettings, ProxySettings};

// -----------------------------------------------------------------------
// normalize_user_agent
// -----------------------------------------------------------------------
#[test]
fn normalize_user_agent_empty_returns_default() {
    let ua = normalize_user_agent("").unwrap();
    assert_eq!(ua, default_http_user_agent());
}

#[test]
fn normalize_user_agent_whitespace_only_returns_default() {
    let ua = normalize_user_agent("   \t  \n  ").unwrap();
    assert_eq!(ua, default_http_user_agent());
}

#[test]
fn normalize_user_agent_valid_short_returns_trimmed() {
    let ua = normalize_user_agent("MyAgent/1.0").unwrap();
    assert_eq!(ua, "MyAgent/1.0");
}

#[test]
fn normalize_user_agent_leading_trailing_whitespace_trimmed() {
    let ua = normalize_user_agent("  CustomAgent/2.0  ").unwrap();
    assert_eq!(ua, "CustomAgent/2.0");
}

#[test]
fn normalize_user_agent_exactly_512_chars_ok() {
    let input = "A".repeat(512);
    let ua = normalize_user_agent(&input).unwrap();
    assert_eq!(ua.len(), 512);
    assert_eq!(ua, input);
}

#[test]
fn normalize_user_agent_over_512_chars_err() {
    let input = "A".repeat(513);
    let err = normalize_user_agent(&input).unwrap_err();
    assert!(matches!(err, DownloadError::InvalidResponse(_)));
}

#[test]
fn normalize_user_agent_invalid_header_chars_err() {
    // Null byte is not valid in an HTTP header value
    let input = "MyAgent\0/1.0";
    let err = normalize_user_agent(input).unwrap_err();
    assert!(matches!(err, DownloadError::InvalidResponse(_)));
}

#[test]
fn normalize_user_agent_newline_in_value_err() {
    // Newline is not valid in an HTTP header value
    let input = "MyAgent\n/1.0";
    let err = normalize_user_agent(input).unwrap_err();
    assert!(matches!(err, DownloadError::InvalidResponse(_)));
}

// -----------------------------------------------------------------------
// configure_client_builder — proxy modes
// -----------------------------------------------------------------------
#[test]
fn configure_builder_disabled_proxy_ok() {
    let settings = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::Disabled,
            ..Default::default()
        },
        ..Default::default()
    };
    let builder = Client::builder();
    let builder = configure_client_builder(builder, &settings).unwrap();
    let client = builder.build();
    assert!(client.is_ok());
}

#[test]
fn configure_builder_system_proxy_ok() {
    let settings = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::System,
            ..Default::default()
        },
        ..Default::default()
    };
    let builder = Client::builder();
    let builder = configure_client_builder(builder, &settings).unwrap();
    let client = builder.build();
    assert!(client.is_ok());
}

#[test]
fn configure_builder_manual_proxy_valid_url_ok() {
    let settings = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: "http://proxy:8080".into(),
        },
        ..Default::default()
    };
    let builder = Client::builder();
    let builder = configure_client_builder(builder, &settings).unwrap();
    let client = builder.build();
    assert!(client.is_ok());
}

#[test]
fn configure_builder_manual_proxy_socks5_url_ok() {
    let settings = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: "socks5://127.0.0.1:1080".into(),
        },
        ..Default::default()
    };
    let builder = Client::builder();
    let builder = configure_client_builder(builder, &settings).unwrap();
    let client = builder.build();
    assert!(client.is_ok());
}

#[test]
fn configure_builder_manual_proxy_empty_url_err() {
    let settings = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: String::new(),
        },
        ..Default::default()
    };
    let builder = Client::builder();
    let err = configure_client_builder(builder, &settings).unwrap_err();
    assert!(matches!(err, DownloadError::InvalidProxy(_)));
}

#[test]
fn configure_builder_manual_proxy_invalid_url_err() {
    // URL with space fails Url::parse (even after adding http:// scheme)
    let settings = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: "http://invalid proxy:8080".into(),
        },
        ..Default::default()
    };
    let builder = Client::builder();
    let err = configure_client_builder(builder, &settings).unwrap_err();
    assert!(matches!(err, DownloadError::InvalidProxy(_)));
}

// -----------------------------------------------------------------------
// configure_client_builder — user-agent error propagation
// -----------------------------------------------------------------------
#[test]
fn configure_builder_user_agent_invalid_chars_err() {
    // Null byte user-agent should fail normalize_user_agent
    let settings = AppSettings {
        download: DownloadDefaultsSettings {
            default_user_agent: "Bad\0Agent".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let builder = Client::builder();
    let err = configure_client_builder(builder, &settings).unwrap_err();
    assert!(matches!(err, DownloadError::InvalidResponse(_)));
}

#[test]
fn configure_builder_user_agent_too_long_err() {
    let settings = AppSettings {
        download: DownloadDefaultsSettings {
            default_user_agent: "X".repeat(600),
            ..Default::default()
        },
        ..Default::default()
    };
    let builder = Client::builder();
    let err = configure_client_builder(builder, &settings).unwrap_err();
    assert!(matches!(err, DownloadError::InvalidResponse(_)));
}

// -----------------------------------------------------------------------
// build_http_client
// -----------------------------------------------------------------------
#[test]
fn build_http_client_defaults_succeeds() {
    let client = build_http_client(&AppSettings::default());
    assert!(client.is_ok());
}

#[test]
fn build_http_client_disabled_proxy_succeeds() {
    let settings = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::Disabled,
            ..Default::default()
        },
        ..Default::default()
    };
    let client = build_http_client(&settings);
    assert!(client.is_ok());
}

#[test]
fn build_http_client_system_proxy_succeeds() {
    let settings = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::System,
            ..Default::default()
        },
        ..Default::default()
    };
    let client = build_http_client(&settings);
    assert!(client.is_ok());
}

#[test]
fn build_http_client_manual_proxy_succeeds() {
    let settings = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: "http://proxy:8080".into(),
        },
        ..Default::default()
    };
    let client = build_http_client(&settings);
    assert!(client.is_ok());
}

#[test]
fn build_http_client_manual_proxy_empty_url_err() {
    let settings = AppSettings {
        proxy: ProxySettings {
            mode: ProxyMode::Manual,
            manual_url: String::new(),
        },
        ..Default::default()
    };
    let err = build_http_client(&settings).unwrap_err();
    assert!(matches!(err, DownloadError::InvalidProxy(_)));
}

#[test]
fn build_http_client_invalid_user_agent_err() {
    let settings = AppSettings {
        download: DownloadDefaultsSettings {
            default_user_agent: "X".repeat(600),
            ..Default::default()
        },
        ..Default::default()
    };
    let err = build_http_client(&settings).unwrap_err();
    assert!(matches!(err, DownloadError::InvalidResponse(_)));
}

// -----------------------------------------------------------------------
// configure_client_builder — chainability with custom DNS
// -----------------------------------------------------------------------
#[test]
fn configure_builder_chainable_with_custom_dns() {
    use std::net::SocketAddr;

    let settings = AppSettings::default();
    let builder = Client::builder();
    let builder = configure_client_builder(builder, &settings).unwrap();
    // Chain custom DNS resolution after configure_client_builder
    let addr: SocketAddr = "127.0.0.1:80".parse().unwrap();
    let builder = builder.resolve_to_addrs("example.com", &[addr]);
    let client = builder.build();
    assert!(client.is_ok());
}

// -----------------------------------------------------------------------
// configure_client_builder — ProxyMode::System prerequisite
// -----------------------------------------------------------------------
/// `ProxyMode::System` delegates to reqwest's `auto_sys_proxy`, whose OS readers
/// (Windows registry, macOS system configuration) only exist when reqwest's
/// `system-proxy` feature is enabled. The workspace turns reqwest's default
/// features off, so dropping that feature is easy and *silent*: the crate still
/// compiles and every other proxy test below still passes because they only
/// assert that a client builds.
///
/// Keep the feature pinned in the workspace manifest so "System Proxy" really
/// reads the OS configuration instead of degenerating to `HTTP_PROXY` env vars.
#[test]
fn workspace_enables_reqwest_system_proxy() {
    use std::path::Path;

    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", manifest_path.display()));
    assert!(
        manifest
            .lines()
            .map(str::trim)
            .any(|line| line == "\"system-proxy\"," || line == "\"system-proxy\""),
        "reqwest's `system-proxy` feature must stay enabled in the workspace 
         Cargo.toml: without it `ProxyMode::System` ignores the Windows registry 
         and the macOS system proxy entirely"
    );
}
