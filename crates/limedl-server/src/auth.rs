use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::Response,
};

use super::config::AuthConfig;

/// axum middleware: check Basic Auth header or ?token= query parameter before
/// allowing WebSocket upgrade. The token parameter is base64(username:password).
/// If AuthConfig is None, all requests pass through.
pub async fn basic_auth_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
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
        .is_some_and(|decoded| decoded == expected);

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
        .is_some_and(|decoded| decoded == expected);

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
    base64::engine::general_purpose::STANDARD
        .decode(input)
        .ok()
}
