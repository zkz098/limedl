use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use reqwest::{Client, Proxy, redirect::Policy};

use crate::download::types::{AppSettings, ProxyMode};

/// Build a separate reqwest Client that resolves `domain` to a specific IPv4 address.
///
/// All TCP/TLS settings mirror `build_http_client()` in manager.rs (lines 2449-2470).
/// The DNS override via `resolve_to_addrs` sends `domain` as the TLS SNI hostname
/// so certificate validation works correctly against the original domain name.
/// Port 0 in the socket address means "use URL scheme default" (443 for https, 80 for http).
pub(crate) fn build_accelerated_client(
    domain: &str,
    ip: Ipv4Addr,
    settings: &AppSettings,
) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
    let mut builder = Client::builder()
        .redirect(Policy::limited(10))
        .tcp_nodelay(true)
        .read_timeout(Duration::from_secs(15))
        .user_agent(settings.download.default_user_agent.clone());

    match settings.proxy.mode {
        ProxyMode::Disabled => {
            builder = builder.no_proxy();
        }
        ProxyMode::System => {
            // Use reqwest default (environment variables).
        }
        ProxyMode::Manual => {
            let proxy = Proxy::all(&settings.proxy.manual_url)?;
            builder = builder.proxy(proxy);
        }
    }

    // DNS override: resolve the domain to the specified IP.
    // Port 0 means "use URL scheme default" (443 for https, 80 for http).
    builder = builder.resolve_to_addrs(domain, &[SocketAddr::new(IpAddr::V4(ip), 0)]);

    let client = builder.build()?;
    Ok(client)
}

/// Check whether a URL targets a Cloudflare domain.
///
/// **v1 stub**: always returns `true`. The user controls CDN acceleration via the
/// enable/disable toggle in settings. A future version will parse the URL hostname
/// and match against the Cloudflare IP ranges to auto-detect eligible domains.
pub(crate) fn is_cloudflare_domain(_url: &str) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_accelerated_client() {
        let settings = AppSettings::default();
        let localhost = Ipv4Addr::new(127, 0, 0, 1);
        let result = build_accelerated_client("example.com", localhost, &settings);
        assert!(result.is_ok(), "should build client without panicking");
    }

    #[test]
    fn test_is_cloudflare_domain() {
        // v1 stub: always returns true for any URL
        assert!(is_cloudflare_domain("https://example.com/file.bin"));
        assert!(is_cloudflare_domain("https://cloudflare.com/"));
        assert!(is_cloudflare_domain("https://not-cloudflare.example/"));
        assert!(is_cloudflare_domain(""));
    }
}
