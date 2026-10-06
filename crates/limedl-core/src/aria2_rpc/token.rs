//! Per-client token generation, Argon2id hashing and constant-time checks.
//!
//! The Aria2 RPC endpoint historically accepted one shared `secret`. In
//! [`Aria2AuthMode::PerClient`] every client gets its own 256-bit token and only
//! an Argon2id hash of it is persisted (settings.json) — the plaintext is shown
//! once in the UI and never stored.
//!
//! Verification is constant-time: the shared-secret path uses
//! [`subtle::ConstantTimeEq`], and Argon2's `verify_password` is itself written
//! not to short-circuit on the first differing byte.
//!
//! Argon2 is deliberately expensive (~20-40 ms), so a token that verified once
//! is remembered for the lifetime of the server by a fast BLAKE3 digest. The
//! digest is never persisted and is never accepted on its own — an entry is
//! only inserted after a full Argon2 verification succeeded, so a database or
//! settings-file leak cannot be replayed against the running server.

use std::sync::Mutex;

use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use subtle::ConstantTimeEq;

use super::HashMap;

/// Token entropy before base64 encoding.
const TOKEN_BYTES: usize = 32;
/// Argon2 salt length (RFC 9106 recommends 16 bytes).
const SALT_BYTES: usize = 16;
/// Upper bound on remembered verifications. At most one entry per configured
/// client can ever be inserted, so this only guards against a future bug
/// leaking entries.
const CACHE_CAPACITY: usize = 256;

/// One configured client, as seen by the verifier.
#[derive(Debug, Clone)]
pub(crate) struct ClientToken {
    pub(crate) id: String,
    pub(crate) name: String,
    /// Argon2id PHC string.
    pub(crate) token_hash: String,
}

/// Per-client authentication state.
pub(crate) struct PerClientAuth {
    clients: Vec<ClientToken>,
    /// BLAKE3 digest of a plaintext token -> index into `clients`. Filled only
    /// after a successful Argon2 verification.
    verified: Mutex<HashMap<[u8; 32], usize>>,
}

impl PerClientAuth {
    pub(crate) fn new(clients: Vec<ClientToken>) -> Self {
        Self {
            clients,
            verified: Mutex::new(HashMap::default()),
        }
    }

    /// Verify `token` and return the matching client.
    ///
    /// Argon2 runs only on the first request bearing a given token; later
    /// requests hit the digest cache. An empty client list rejects everything
    /// (fail closed) rather than degrading into "no authentication".
    pub(crate) fn verify(&self, token: &str) -> Option<&ClientToken> {
        if self.clients.is_empty() {
            return None;
        }

        let digest: [u8; 32] = blake3::hash(token.as_bytes()).into();
        // A poisoned lock only means another thread panicked while inserting;
        // falling through to a fresh Argon2 verification is always safe.
        if let Ok(cache) = self.verified.lock()
            && let Some(&index) = cache.get(&digest)
        {
            return self.clients.get(index);
        }

        let index = self
            .clients
            .iter()
            .position(|client| verify_token(token, &client.token_hash))?;

        if let Ok(mut cache) = self.verified.lock() {
            if cache.len() >= CACHE_CAPACITY {
                cache.clear();
            }
            cache.insert(digest, index);
        }
        self.clients.get(index)
    }
}

/// Generate a fresh 256-bit token, URL-safe base64 without padding.
pub fn generate_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Hash a plaintext token with Argon2id (default parameters: 19 MiB, t=2, p=1).
///
/// Only the returned PHC string may be persisted.
pub fn hash_token(token: &str) -> Result<String, String> {
    let mut salt_bytes = [0u8; SALT_BYTES];
    rand::fill(&mut salt_bytes);
    Argon2::default()
        .hash_password_with_salt(token.as_bytes(), &salt_bytes)
        .map(|hash| hash.to_string())
        .map_err(|err| err.to_string())
}

/// Verify `token` against a stored Argon2 PHC string, in constant time.
///
/// A malformed hash is a non-match rather than an error so a hand-edited
/// settings file cannot take the RPC server down.
pub(crate) fn verify_token(token: &str, stored_hash: &str) -> bool {
    Argon2::default()
        .verify_password(token.as_bytes(), stored_hash)
        .is_ok()
}

/// Constant-time string equality, for the shared-secret path.
pub(crate) fn constant_time_eq(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_are_unique_and_url_safe() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
        // 32 bytes -> 43 base64 chars, no padding.
        assert_eq!(a.len(), 43);
        assert!(
            a.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }

    #[test]
    fn hash_and_verify_round_trip() {
        let token = generate_token();
        let hash = hash_token(&token).expect("hash");
        assert!(hash.starts_with("$argon2id$"), "PHC string: {hash}");
        assert!(
            !hash.contains(&token),
            "the plaintext must not appear in the hash"
        );
        assert!(verify_token(&token, &hash));
        assert!(!verify_token("wrong", &hash));
        assert!(!verify_token(&token, "not-a-phc-string"));
    }

    #[test]
    fn constant_time_eq_matches_equality() {
        assert!(constant_time_eq("secret", "secret"));
        assert!(!constant_time_eq("secret", "secrez"));
        assert!(!constant_time_eq("secret", "secret-longer"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn per_client_auth_matches_only_the_right_token() {
        let auth = PerClientAuth::new(vec![
            ClientToken {
                id: "a".into(),
                name: "A".into(),
                token_hash: hash_token("alpha").expect("hash alpha"),
            },
            ClientToken {
                id: "b".into(),
                name: "B".into(),
                token_hash: hash_token("beta").expect("hash beta"),
            },
        ]);
        assert_eq!(auth.verify("alpha").map(|c| c.id.as_str()), Some("a"));
        assert_eq!(auth.verify("beta").map(|c| c.id.as_str()), Some("b"));
        assert!(auth.verify("gamma").is_none());
    }

    #[test]
    fn per_client_auth_rejects_when_empty() {
        let auth = PerClientAuth::new(Vec::new());
        assert!(auth.verify("anything").is_none());
    }

    #[test]
    fn verified_token_is_cached_by_digest() {
        let auth = PerClientAuth::new(vec![ClientToken {
            id: "a".into(),
            name: "A".into(),
            token_hash: hash_token("alpha").expect("hash alpha"),
        }]);
        assert!(auth.verify("alpha").is_some());
        assert_eq!(auth.verified.lock().expect("cache").len(), 1);
        // A repeat verification stays cached instead of running Argon2 again.
        assert!(auth.verify("alpha").is_some());
        assert_eq!(auth.verified.lock().expect("cache").len(), 1);
        // A wrong token must not be cached.
        assert!(auth.verify("wrong").is_none());
        assert_eq!(auth.verified.lock().expect("cache").len(), 1);
    }
}
