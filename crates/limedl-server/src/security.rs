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
