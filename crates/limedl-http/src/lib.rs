//! HTTP transport policy for the limedl engine.
//!
//! Three modules that only make sense together:
//!
//! * `http_client_factory` builds the one `reqwest::Client` policy set the
//!   engine has (redirect limit, timeouts, pool sizes, proxy mode, the
//!   `identity` encoding that keeps `Range` offsets intact);
//! * `http` holds the header/url helpers that policy and the executor share;
//! * `rate_limiter` is the token bucket every download and the global cap draw
//!   from.
//!
//! `limedl-types` is re-exported at this crate root so the moved modules kept
//! their `crate::types::…` spellings.

pub use limedl_types::{error, manifest, settings, types};

pub mod http;
pub mod http_client_factory;
pub mod rate_limiter;
