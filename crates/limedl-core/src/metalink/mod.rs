//! Metalink 4.0 (RFC 5854), Metalink 3.0, and Metalink/HTTP (RFC 6249) support.

pub mod http_link;
pub mod pool;
pub mod prober;
pub mod scorer;
pub mod types;
pub mod xml_parser;

#[cfg(test)]
mod tests;

pub use http_link::{MetalinkHttpMetadata, parse_metalink_headers};
pub use pool::{MirrorCandidate, MirrorLease, MirrorPool, MirrorStatusSnapshot};
pub use prober::{ProbeResult, probe_mirrors};
pub use scorer::{MirrorScoringConfig, calculate_mirror_score};
pub use types::{
    ChecksumEntry, MetalinkDocument, MetalinkFile, MetalinkMetaUrl, MirrorResource,
    PieceVerification,
};
pub use xml_parser::parse_metalink_xml;
