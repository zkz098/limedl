use std::path::PathBuf;

use super::error::{DownloadError, Result};
use super::types::ChecksumMode;

pub mod detect;
pub use detect::{detect_sha256, parse_sha256_file, parse_sha256sums};

pub enum ChecksumHasher {
    Blake3(Box<blake3::Hasher>),
    Sha256(sha2::Sha256),
    Sha512(sha2::Sha512),
}

impl ChecksumHasher {
    pub fn new(mode: ChecksumMode) -> Result<Self> {
        match mode {
            ChecksumMode::None => Err(DownloadError::InvalidResponse(
                "checksum mode is None but hashing was reached".into(),
            )),
            ChecksumMode::Blake3 => Ok(Self::Blake3(Box::new(blake3::Hasher::new()))),
            ChecksumMode::Sha256 => {
                use sha2::Digest;
                Ok(Self::Sha256(sha2::Sha256::new()))
            }
            ChecksumMode::Sha512 => {
                use sha2::Digest;
                Ok(Self::Sha512(sha2::Sha512::new()))
            }
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Blake3(hasher) => {
                hasher.update(bytes);
            }
            Self::Sha256(hasher) => {
                use sha2::Digest;
                hasher.update(bytes);
            }
            Self::Sha512(hasher) => {
                use sha2::Digest;
                hasher.update(bytes);
            }
        }
    }

    pub fn finalize(self) -> String {
        match self {
            Self::Blake3(hasher) => hasher.finalize().to_hex().to_string(),
            Self::Sha256(hasher) => {
                use sha2::Digest;
                hex_lower(&hasher.finalize())
            }
            Self::Sha512(hasher) => {
                use sha2::Digest;
                hex_lower(&hasher.finalize())
            }
        }
    }
}

/// Lowercase hex encoding of a fixed-size SHA-2 digest.
fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Compute checksum from ordered byte slices (for in-memory buffer use).
pub fn hash_slices(mode: ChecksumMode, slices: &[&[u8]]) -> String {
    use blake3::Hasher;

    match mode {
        ChecksumMode::None => String::new(),
        ChecksumMode::Blake3 => {
            let mut hasher = Hasher::new();
            for slice in slices {
                hasher.update(slice);
            }
            hasher.finalize().to_hex().to_string()
        }
        ChecksumMode::Sha256 => {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            for slice in slices {
                hasher.update(slice);
            }
            hex_lower(&hasher.finalize())
        }
        ChecksumMode::Sha512 => {
            use sha2::{Digest, Sha512};
            let mut hasher = Sha512::new();
            for slice in slices {
                hasher.update(slice);
            }
            hex_lower(&hasher.finalize())
        }
    }
}

pub async fn calculate_checksum(path: PathBuf, mode: ChecksumMode) -> Result<String> {
    tokio::task::spawn_blocking(move || -> Result<String> {
        use std::io::Read;

        let mut file = std::fs::File::open(path)?;
        let mut hasher = ChecksumHasher::new(mode)?;
        let mut buffer = vec![0_u8; 1024 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(hasher.finalize())
    })
    .await
    .map_err(|error| DownloadError::Internal(format!("checksum computation failed: {error}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonical NIST/BLAKE3 digests of `b"abc"`.
    const ABC_BLAKE3: &str = "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85";
    const ABC_SHA256: &str =
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const ABC_SHA512: &str = "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
                             2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f";

    #[test]
    fn hash_slices_matches_known_vectors() {
        let slices: &[&[u8]] = &[b"a", b"bc"];
        assert_eq!(hash_slices(ChecksumMode::Blake3, slices), ABC_BLAKE3);
        assert_eq!(hash_slices(ChecksumMode::Sha256, slices), ABC_SHA256);
        assert_eq!(hash_slices(ChecksumMode::Sha512, slices), ABC_SHA512);
    }

    #[test]
    fn hasher_matches_hash_slices() {
        for (mode, expected) in [
            (ChecksumMode::Blake3, ABC_BLAKE3),
            (ChecksumMode::Sha256, ABC_SHA256),
            (ChecksumMode::Sha512, ABC_SHA512),
        ] {
            let mut hasher = ChecksumHasher::new(mode).expect("hasher");
            hasher.update(b"a");
            hasher.update(b"bc");
            assert_eq!(hasher.finalize(), expected, "mode {mode:?}");
        }
    }

    #[test]
    fn digest_lengths_are_stable() {
        let slices: &[&[u8]] = &[b"abc"];
        assert_eq!(hash_slices(ChecksumMode::Blake3, slices).len(), 64);
        assert_eq!(hash_slices(ChecksumMode::Sha256, slices).len(), 64);
        assert_eq!(hash_slices(ChecksumMode::Sha512, slices).len(), 128);
    }

    #[test]
    fn none_mode_is_not_a_hasher() {
        assert!(ChecksumHasher::new(ChecksumMode::None).is_err());
        assert!(hash_slices(ChecksumMode::None, &[b"abc"]).is_empty());
    }
}
