//! `DownloadManager::resolve_client` — CDN branch selection and caching.

use std::net::{IpAddr, Ipv4Addr};

use crate::cdn::CdnAccelerator;
use crate::testing::make_manager;
use crate::types::AppSettings;

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

const CF_IP: Ipv4Addr = Ipv4Addr::new(104, 16, 0, 1);
const CF_URL: &str = "https://104.16.0.1/file.bin";

async fn enable_cdn(dm: &crate::manager::DownloadManager, active_ip: Option<&str>) -> TestResult {
    dm.settings_service
        .update_with(|settings| {
            settings.cdn_acceleration.enabled = true;
            settings.cdn_acceleration.active_ip = active_ip.map(str::to_string);
            Ok(())
        })
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn resolve_client_returns_standard_client_when_cdn_disabled() -> TestResult {
    let (_tmp, dm) = make_manager();

    let (_, accelerated, ip) = dm.resolve_client(CF_URL).await;
    assert!(!accelerated);
    assert!(ip.is_none());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn resolve_client_ignores_non_cloudflare_hosts() -> TestResult {
    let (_tmp, dm) = make_manager();
    enable_cdn(&dm, Some("104.16.0.1")).await?;

    // localhost resolves offline to a loopback address, which is not a
    // Cloudflare range, so the standard client must be used.
    let (_, accelerated, ip) = dm.resolve_client("http://localhost:1/file.bin").await;
    assert!(!accelerated);
    assert!(ip.is_none());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn resolve_client_falls_back_to_the_persisted_ip_and_caches() -> TestResult {
    let (_tmp, dm) = make_manager();
    enable_cdn(&dm, Some("104.16.0.1")).await?;

    let (_, accelerated, ip) = dm.resolve_client(CF_URL).await;
    assert!(accelerated);
    assert_eq!(ip.as_deref(), Some("104.16.0.1"));

    // The second lookup must hit the (host, ip) client cache and still be
    // accelerated.
    let (_, cached_accelerated, cached_ip) = dm.resolve_client(CF_URL).await;
    assert!(cached_accelerated);
    assert_eq!(cached_ip.as_deref(), Some("104.16.0.1"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn resolve_client_requires_an_active_ip() -> TestResult {
    let (_tmp, dm) = make_manager();
    enable_cdn(&dm, None).await?;

    let (_, accelerated, ip) = dm.resolve_client(CF_URL).await;
    assert!(!accelerated);
    assert!(ip.is_none());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn resolve_client_prefers_the_in_memory_accelerator() -> TestResult {
    let (_tmp, dm) = make_manager();
    enable_cdn(&dm, Some("104.16.0.1")).await?;

    let accelerator = std::sync::Arc::new(CdnAccelerator::new());
    accelerator
        .apply_ip(IpAddr::V4(CF_IP), 12.5, &AppSettings::default())
        .await?;
    dm.set_cdn_accelerator(accelerator);

    let (_, accelerated, ip) = dm.resolve_client(CF_URL).await;
    assert!(accelerated);
    assert_eq!(ip.as_deref(), Some("104.16.0.1"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn resolve_client_uses_persisted_ip_when_accelerator_is_idle() -> TestResult {
    let (_tmp, dm) = make_manager();
    enable_cdn(&dm, Some("104.16.0.1")).await?;

    // An idle accelerator has no active IP, so the persisted settings value
    // must still enable acceleration.
    dm.set_cdn_accelerator(std::sync::Arc::new(CdnAccelerator::new()));

    let (_, accelerated, ip) = dm.resolve_client(CF_URL).await;
    assert!(accelerated);
    assert_eq!(ip.as_deref(), Some("104.16.0.1"));

    Ok(())
}
