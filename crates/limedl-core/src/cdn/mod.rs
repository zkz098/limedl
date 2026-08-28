pub mod accelerator;
pub mod ip_ranges;
pub mod provider;
pub mod resolver;
pub mod service;
pub mod speed_test;
#[cfg(test)]
pub mod tests;

pub use accelerator::{AccelState, CdnAccelerator};
pub use ip_ranges::{CdnIpCache, CLOUDFLARE_IPV4_RANGES, CLOUDFLARE_IPV6_RANGES, FALLBACK_CACHE};
pub use provider::{
    CdnProvider, CdnProviderKind, CloudflareProvider, CustomCdnProvider, FastlyProvider,
    FASTLY_IPV4_RANGES, FASTLY_IPV6_RANGES,
};
pub use resolver::{build_accelerated_client, is_cloudflare_domain};
pub use service::{CdnService, CdnTestOutcome};
pub use speed_test::{CdnTestPhase, CdnTestProgress, DefaultNodeResult, SpeedTestResult};
