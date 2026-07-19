use axum::http::header::{HeaderName, HeaderValue};
use tower_http::set_header::SetResponseHeaderLayer;

/// Returns the Content-Security-Policy header value for the NAS WebUI.
pub fn nas_csp_header() -> HeaderValue {
    HeaderValue::from_static(
         "default-src 'self'; connect-src 'self' ws://localhost:9090; \
         style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; \
         font-src 'self'; script-src 'self'; object-src 'none'; \
         base-uri 'self'; form-action 'self'",
    )
}

/// Convenience: all security header layers as a tuple of layers.
/// Use in router: `.layer(security_headers_layers())`
pub fn security_headers_layers() -> (
    SetResponseHeaderLayer<HeaderValue>,
    SetResponseHeaderLayer<HeaderValue>,
    SetResponseHeaderLayer<HeaderValue>,
    SetResponseHeaderLayer<HeaderValue>,
) {
    (
        SetResponseHeaderLayer::overriding(
            HeaderName::from_static("content-security-policy"),
            nas_csp_header(),
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
