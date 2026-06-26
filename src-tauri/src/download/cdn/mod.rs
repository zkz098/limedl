mod accelerator;
pub mod commands;
mod ip_ranges;
mod resolver;
mod speed_test;
#[cfg(test)]
mod tests;

pub(crate) use accelerator::CdnAccelerator;
pub(crate) use resolver::{build_accelerated_client, is_cloudflare_domain};
