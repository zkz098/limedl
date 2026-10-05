use serde::{Deserialize, Serialize};

use crate::types::ChecksumMode;

/// Parsed Metalink document (supporting Metalink 4.0 RFC 5854 and Metalink 3.0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetalinkDocument {
    pub files: Vec<MetalinkFile>,
    pub origin: Option<String>,
    pub published: Option<String>,
}

/// A target downloadable file inside a Metalink document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetalinkFile {
    pub name: String,
    pub size: Option<u64>,
    pub hashes: Vec<ChecksumEntry>,
    pub pieces: Option<PieceVerification>,
    pub resources: Vec<MirrorResource>,
    pub metaurls: Vec<MetalinkMetaUrl>,
    pub identity: Option<String>,
    pub version: Option<String>,
    pub language: Option<String>,
    pub os: Option<String>,
}

impl MetalinkFile {
    /// Pick the strongest cryptographic hash supported by limedl.
    /// Priority order: SHA-512 > SHA-256 > BLAKE3.
    pub fn best_checksum(&self) -> Option<(ChecksumMode, String)> {
        if let Some(entry) = self.hashes.iter().find(|h| h.algorithm == ChecksumMode::Sha512) {
            return Some((ChecksumMode::Sha512, entry.hash.clone()));
        }
        if let Some(entry) = self.hashes.iter().find(|h| h.algorithm == ChecksumMode::Sha256) {
            return Some((ChecksumMode::Sha256, entry.hash.clone()));
        }
        if let Some(entry) = self.hashes.iter().find(|h| h.algorithm == ChecksumMode::Blake3) {
            return Some((ChecksumMode::Blake3, entry.hash.clone()));
        }
        None
    }

    /// Return all mirror resource URLs sorted by priority (lowest priority number = highest preference).
    pub fn sorted_mirror_urls(&self) -> Vec<String> {
        let mut sorted = self.resources.clone();
        sorted.sort_by_key(|r| r.priority);
        sorted.into_iter().map(|r| r.url).collect()
    }
}

/// A whole-file cryptographic hash digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecksumEntry {
    pub algorithm: ChecksumMode,
    pub raw_type: String,
    pub hash: String,
}

/// Piece/chunk hashes for progressive piece verification (<pieces> element).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PieceVerification {
    pub piece_length: u64,
    pub algorithm: ChecksumMode,
    pub hashes: Vec<String>,
}

/// A mirror URL resource with priority and metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorResource {
    pub url: String,
    /// Normalized priority: 1 is the highest priority (RFC 5854 semantics).
    pub priority: u32,
    /// ISO 3166-1 alpha-2 location code (e.g. "cn", "us", "de").
    pub location: Option<String>,
    /// Optional max concurrent connections for this mirror.
    pub max_connections: Option<usize>,
}

/// External metadata link (<metaurl>), such as a BitTorrent torrent or magnet link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetalinkMetaUrl {
    pub url: String,
    pub media_type: String,
    pub priority: u32,
}
