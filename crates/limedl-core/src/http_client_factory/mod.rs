use std::time::Duration;

use reqwest::{Client, ClientBuilder, Proxy, header, redirect::Policy};

use super::{
    error::{DownloadError, Result},
    types::{AppSettings, ProxyMode, default_http_user_agent},
};

/// Normalize and validate a user-agent string.
/// Returns the default Chrome user-agent if the input is empty,
/// or an error if the value is too long or contains invalid characters.
pub fn normalize_user_agent(user_agent: &str) -> Result<String> {
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
pub fn build_http_client(settings: &AppSettings) -> Result<Client> {
    let builder = Client::builder();
    let builder = configure_client_builder(builder, settings)?;
    builder.build().map_err(DownloadError::from)
}

/// Apply shared proxy, user-agent, redirect, and TCP settings to a `ClientBuilder`.
/// Callers can chain additional configuration (e.g., `resolve_to_addrs`) afterward.
pub fn configure_client_builder(
    mut builder: ClientBuilder,
    settings: &AppSettings,
) -> Result<ClientBuilder> {
    let (connect_timeout, read_timeout, pool_max_idle, keepalive) = if settings.scheduler.expert_mode {
        let exp = &settings.scheduler.expert;
        (
            Duration::from_secs(exp.connect_timeout_secs),
            Duration::from_secs(exp.read_timeout_secs),
            exp.pool_max_idle_per_host,
            Some(Duration::from_secs(exp.tcp_keepalive_secs)),
        )
    } else {
        (
            Duration::from_secs(30),
            Duration::from_secs(30),
            20,
            Some(Duration::from_secs(60)),
        )
    };

    let default_user_agent = normalize_user_agent(&settings.download.default_user_agent)?;
    builder = builder
        .redirect(Policy::limited(10))
        .tcp_nodelay(true)
        .read_timeout(read_timeout)
        .user_agent(default_user_agent)
        .connect_timeout(connect_timeout)
        .pool_max_idle_per_host(pool_max_idle)
        .pool_idle_timeout(Duration::from_secs(120))
        .tcp_keepalive(keepalive);

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

#[cfg(test)]
mod tests;
