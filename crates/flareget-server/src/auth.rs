use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::Response,
};

use super::config::AuthConfig;

/// axum middleware: check Basic Auth header before allowing WebSocket upgrade.
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

    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "));

    let Some(encoded) = auth_header else {
        let mut response = Response::new(axum::body::Body::empty());
        *response.status_mut() = StatusCode::UNAUTHORIZED;
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static(r#"Basic realm="flareget""#),
        );
        return Ok(response);
    };

    // Decode base64
    let decoded =
        String::from_utf8(base64_decode(encoded)?).map_err(|_| StatusCode::UNAUTHORIZED)?;
    let expected = format!("{}:{}", auth.username, auth.password);
    let valid = decoded == expected;

    if valid {
        Ok(next.run(req).await)
    } else {
        let mut response = Response::new(axum::body::Body::empty());
        *response.status_mut() = StatusCode::UNAUTHORIZED;
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static(r#"Basic realm="flareget""#),
        );
        Ok(response)
    }
}

fn base64_decode(input: &str) -> Result<Vec<u8>, StatusCode> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|_| StatusCode::UNAUTHORIZED)
}
