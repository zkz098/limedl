use std::time::Duration;

use reqwest::{Client, ClientBuilder, Proxy, header, redirect::Policy};

use super::{
    error::{DownloadError, Result},
    types::{AppSettings, ProxyMode, default_http_user_agent},
};

/// Normalize and validate a user-agent string.
/// Returns the default Chrome user-agent if the input is empty,
/// or an error if the value is too long or contains invalid characters.
pub(super) fn normalize_user_agent(user_agent: &str) -> Result<String> {
    let normalized = user_agent.trim();
    if normalized.is_empty() {
        return Ok(default_http_user_agent());
    }
    if normalized.len() > 512 || header::HeaderValue::from_str(normalized).is_err() {
        return Err(DownloadError::InvalidResponse(String::from(
            "invalid user-agent value",
        )));
    }

    Ok(normalized.to_string())
}

/// Build a fully-configured `reqwest::Client` from application settings.
/// Used by DownloadManager and BT Backend for standard HTTP(S) downloads.
pub(crate) fn build_http_client(settings: &AppSettings) -> Result<Client> {
    let builder = Client::builder();
    let builder = configure_client_builder(builder, settings)?;
    builder.build().map_err(DownloadError::from)
}

/// Apply shared proxy, user-agent, redirect, and TCP settings to a `ClientBuilder`.
/// Callers can chain additional configuration (e.g., `resolve_to_addrs`) afterward.
pub(crate) fn configure_client_builder(
    mut builder: ClientBuilder,
    settings: &AppSettings,
) -> Result<ClientBuilder> {
    let default_user_agent = normalize_user_agent(&settings.download.default_user_agent)?;
    builder = builder
        .redirect(Policy::limited(10))
        .tcp_nodelay(true)
        .read_timeout(Duration::from_secs(15))
        .user_agent(default_user_agent);

    match settings.proxy.mode {
        ProxyMode::Disabled => {
            builder = builder.no_proxy();
        }
        ProxyMode::System => {}
        ProxyMode::Manual => {
            let proxy = Proxy::all(&settings.proxy.manual_url)
                .map_err(|error| DownloadError::InvalidProxy(error.to_string()))?;
            builder = builder.proxy(proxy);
        }
    }

    Ok(builder)
}
