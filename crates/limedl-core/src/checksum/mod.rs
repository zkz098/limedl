use std::path::PathBuf;

use super::error::{DownloadError, Result};
use super::types::ChecksumMode;

pub enum ChecksumHasher {
    Blake3(Box<blake3::Hasher>),
    Sha256(sha2::Sha256),
    Xxh3_128(Box<xxhash_rust::xxh3::Xxh3>),
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
            ChecksumMode::Xxh3128 => Ok(Self::Xxh3_128(Box::new(xxhash_rust::xxh3::Xxh3::new()))),
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
            Self::Xxh3_128(hasher) => {
                hasher.update(bytes);
            }
        }
    }

    pub fn finalize(self) -> String {
        match self {
            Self::Blake3(hasher) => hasher.finalize().to_hex().to_string(),
            Self::Sha256(hasher) => {
                use sha2::Digest;
                let result = hasher.finalize();
                result.iter().map(|b| format!("{:02x}", b)).collect::<String>()
            }
            Self::Xxh3_128(hasher) => format!("{:032x}", hasher.digest128()),
        }
    }
}

/// Compute checksum from ordered byte slices (for in-memory buffer use).
pub fn hash_slices(mode: ChecksumMode, slices: &[&[u8]]) -> String {
    use blake3::Hasher;
    use sha2::{Digest, Sha256};

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
            let mut hasher = Sha256::new();
            for slice in slices {
                hasher.update(slice);
            }
            let result = hasher.finalize();
            result.iter().map(|b| format!("{:02x}", b)).collect::<String>()
        }
        ChecksumMode::Xxh3128 => {
            let mut hasher = xxhash_rust::xxh3::Xxh3::new();
            for slice in slices {
                hasher.update(slice);
            }
            format!("{:032x}", hasher.digest128())
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
