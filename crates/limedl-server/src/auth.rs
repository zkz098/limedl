use axum::{
    body::Body,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::Response,
};

use super::config::AuthConfig;

/// axum middleware: check Basic Auth header or ?token= query parameter before
/// allowing WebSocket upgrade. The token parameter is base64(username:password).
/// If AuthConfig is None, all requests pass through.
pub async fn basic_auth_middleware(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    // Extract auth config from request extensions (set in router setup)
    let auth_config = req
        .extensions()
        .get::<Option<AuthConfig>>()
        .cloned()
        .flatten();

    let Some(auth) = auth_config else {
        return Ok(next.run(req).await);
    };

    let expected = format!("{}:{}", auth.username, auth.password);

    // Try Authorization header first
    let valid = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
        .and_then(base64_decode)
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .is_some_and(|decoded| constant_time_eq(decoded.as_bytes(), expected.as_bytes()));

    if valid {
        return Ok(next.run(req).await);
    }

    // Fall back to ?token= query parameter (for WebSocket connections)
    let valid = req
        .uri()
        .query()
        .and_then(|q| {
            // Parse query string manually to avoid pulling in a dependency
            for pair in q.split('&') {
                if let Some(value) = pair.strip_prefix("token=") {
                    return Some(value);
                }
            }
            None
        })
        .and_then(base64_decode)
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .is_some_and(|decoded| constant_time_eq(decoded.as_bytes(), expected.as_bytes()));

    if valid {
        return Ok(next.run(req).await);
    }

    // No valid auth found — challenge
    let mut response = Response::new(axum::body::Body::empty());
    *response.status_mut() = StatusCode::UNAUTHORIZED;
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        header::HeaderValue::from_static(r#"Basic realm="limedl""#),
    );
    Ok(response)
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(input).ok()
}

/// Constant-time comparison of two byte slices. `subtle::ConstantTimeEq::ct_eq`
/// already handles unequal-length slices in constant time (compares the
/// overlap, then folds a length-equality check), so no length pre-check is
/// needed — an early `len() != len()` branch would itself introduce a
/// time-dependent divergence.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    a.ct_eq(b).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{middleware::from_fn, routing::get, Router};
    use base64::Engine;
    use tower::ServiceExt;

    fn auth_cfg() -> AuthConfig {
        AuthConfig { username: "user".into(), password: "pass".into() }
    }

    fn b64(s: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
    }

    /// Mirror the production extension-insertion pattern from `main.rs` lines 177-182:
    /// a wrapping closure inserts the `Option<AuthConfig>` into request extensions
    /// before delegating to `basic_auth_middleware`.
    fn make_router(auth: Option<AuthConfig>) -> Router {
        Router::new()
            .route("/ws", get(|| async { "ok" }))
            .layer(from_fn(move |mut req: Request<Body>, next: Next| {
                req.extensions_mut().insert(auth.clone());
                basic_auth_middleware(req, next)
            }))
    }

    fn request(uri: &str) -> Request<Body> {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn no_auth_config_in_extensions_passes_through() {
        let router = make_router(None);
        let resp = router.oneshot(request("/ws")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn missing_extension_entirely_passes_through() {
        // No wrapping closure to insert the extension — middleware logic falls
        // through because `req.extensions().get::<Option<AuthConfig>>()` is None.
        let router: Router = Router::new()
            .route("/ws", get(|| async { "ok" }))
            .layer(from_fn(basic_auth_middleware));
        let resp = router.oneshot(request("/ws")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn valid_basic_auth_header_passes() {
        let router = make_router(Some(auth_cfg()));
        let mut req = request("/ws");
        req.headers_mut().insert(
            header::AUTHORIZATION,
            axum::http::HeaderValue::from_str(&format!("Basic {}", b64("user:pass"))).unwrap(),
        );
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn wrong_basic_auth_password_returns_unauthorized_with_challenge() {
        let router = make_router(Some(auth_cfg()));
        let mut req = request("/ws");
        req.headers_mut().insert(
            header::AUTHORIZATION,
            axum::http::HeaderValue::from_str(&format!("Basic {}", b64("user:WRONG"))).unwrap(),
        );
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            resp.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            axum::http::HeaderValue::from_static(r#"Basic realm="limedl""#),
        );
    }

    #[tokio::test]
    async fn malformed_basic_auth_header_returns_unauthorized() {
        let router = make_router(Some(auth_cfg()));
        let mut req = request("/ws");
        req.headers_mut().insert(
            header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Basic !!not-base64!!"),
        );
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn valid_token_query_param_passes() {
        let router = make_router(Some(auth_cfg()));
        let req = request(&format!("/ws?token={}", b64("user:pass")));
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn wrong_token_query_param_returns_unauthorized() {
        let router = make_router(Some(auth_cfg()));
        let req = request(&format!("/ws?token={}", b64("user:WRONG")));
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn no_auth_when_configured_returns_unauthorized() {
        let router = make_router(Some(auth_cfg()));
        let resp = router.oneshot(request("/ws")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn constant_time_eq_handles_equal_unequal_and_length_mismatch() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"", b"a"));
        assert!(constant_time_eq(b"", b""));
    }
}
