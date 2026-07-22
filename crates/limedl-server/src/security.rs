use axum::http::header::{HeaderName, HeaderValue};
use tower_http::set_header::SetResponseHeaderLayer;

/// Returns the Content-Security-Policy header value for the NAS WebUI.
///
/// `connect-src` is generated from the configured `host`/`port`/`tls_enabled`
/// so that the frontend can reach the WebSocket endpoint regardless of the
/// bind address. When the bind host is a wildcard (`0.0.0.0` / `::` / empty),
/// any origin on the configured port is allowed; otherwise the specific bind
/// host is allowed plus a localhost fallback for local development.
pub fn nas_csp_header(host: &str, port: u16, tls_enabled: bool) -> HeaderValue {
    let ws_scheme = if tls_enabled { "wss" } else { "ws" };
    let ws_origin = if host.is_empty() || host == "0.0.0.0" || host == "[::]" {
        format!("{ws_scheme}://*:{port}")
    } else {
        format!("{ws_scheme}://localhost:{port} {ws_scheme}://{host}:{port}")
    };
    let csp = format!(
        "default-src 'self'; connect-src 'self' {ws_origin}; \
         style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; \
         font-src 'self'; script-src 'self'; object-src 'none'; \
         base-uri 'self'; form-action 'self'"
    );
    HeaderValue::from_str(&csp).expect("CSP header is built from valid ASCII tokens")
}

/// Convenience: all security header layers as a tuple of layers.
/// Use in router: `.layer(security_headers_layers(...).3)` etc.
pub fn security_headers_layers(
    host: &str,
    port: u16,
    tls_enabled: bool,
) -> (
    SetResponseHeaderLayer<HeaderValue>,
    SetResponseHeaderLayer<HeaderValue>,
    SetResponseHeaderLayer<HeaderValue>,
    SetResponseHeaderLayer<HeaderValue>,
) {
    (
        SetResponseHeaderLayer::overriding(
            HeaderName::from_static("content-security-policy"),
            nas_csp_header(host, port, tls_enabled),
        ),
        SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ),
        SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ),
        SetResponseHeaderLayer::overriding(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_ipv4_host_emits_wildcard_ws_origin_without_tls() {
        let h = nas_csp_header("0.0.0.0", 9090, false).to_str().unwrap().to_string();
        assert!(h.contains("ws://*:9090"));
        assert!(!h.contains("wss://"));
        assert!(h.contains("default-src 'self'"));
        assert!(h.contains("object-src 'none'"));
    }

    #[test]
    fn wildcard_ipv6_host_emits_wildcard_ws_origin_without_tls() {
        let hv = nas_csp_header("[::]", 9090, false);
        let h = hv.to_str().unwrap();
        assert!(h.contains("ws://*:9090"));
    }

    #[test]
    fn empty_host_with_tls_emits_wss_scheme_only() {
        let h = nas_csp_header("", 9090, true).to_str().unwrap().to_string();
        assert!(h.contains("wss://*:9090"));
        assert!(!h.contains("ws://"));
    }

    #[test]
    fn specific_host_without_tls_emits_both_localhost_and_host_origins() {
        let h = nas_csp_header("example.com", 9090, false).to_str().unwrap().to_string();
        assert!(h.contains("ws://localhost:9090"));
        assert!(h.contains("ws://example.com:9090"));
        assert!(!h.contains("wss://"));
    }

    #[test]
    fn specific_host_with_tls_uses_wss_scheme_and_both_origins() {
        let h = nas_csp_header("example.com", 8443, true).to_str().unwrap().to_string();
        assert!(h.contains("wss://localhost:8443"));
        assert!(h.contains("wss://example.com:8443"));
        assert!(!h.contains("ws://"));
    }

    #[test]
    fn csp_header_value_is_valid_ascii() {
        // The internal `.expect()` already verifies the invariant; this smoke
        // test documents it publicly.
        let h = nas_csp_header("0.0.0.0", 9090, false);
        assert!(h.to_str().is_ok());
    }

    #[test]
    fn security_headers_layers_returns_a_constructable_tuple() {
        let layers = security_headers_layers("example.com", 9090, false);
        // Tuple has 4 elements; smoke-test it constructs without panic.
        let _ = (layers.0, layers.1, layers.2, layers.3);
    }

    /// Golden-string test: lock the entire CSP for one canonical input so
    /// removal/reordering of any directive is caught immediately.
    #[test]
    fn golden_string_specific_host_tls_wss_port_8443() {
        let h = nas_csp_header("example.com", 8443, true)
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(
            h,
            "default-src 'self'; connect-src 'self' wss://localhost:8443 wss://example.com:8443; \
             style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; \
             font-src 'self'; script-src 'self'; object-src 'none'; \
             base-uri 'self'; form-action 'self'"
        );
    }

    /// Golden-string wildcard form: confirm `ws`/`wss` scheme selection and
    /// the `*:port` placeholder together cover all directive keys end-to-end.
    #[test]
    fn golden_string_wildcard_host_no_tls_port_9090() {
        let h = nas_csp_header("0.0.0.0", 9090, false)
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(
            h,
            "default-src 'self'; connect-src 'self' ws://*:9090; \
             style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; \
             font-src 'self'; script-src 'self'; object-src 'none'; \
             base-uri 'self'; form-action 'self'"
        );
    }
}
