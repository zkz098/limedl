pub mod accelerator;
pub mod ip_ranges;
pub mod resolver;
pub mod speed_test;
#[cfg(test)]
pub mod tests;

pub use accelerator::{AccelState, CdnAccelerator};
pub use ip_ranges::CLOUDFLARE_IPV4_RANGES;
pub use resolver::{build_accelerated_client, is_cloudflare_domain};
pub use speed_test::{CdnTestPhase, CdnTestProgress, DefaultNodeResult, SpeedTestResult};
