//! Per-client Aria2 RPC token rows (`aria2_rpc.clients`).
//!
//! Rows live in the UI model until Save, exactly like the media overrides. A
//! row's `token` holds the plaintext only right after Generate/Regenerate; the
//! authoritative value is `token_hash` — an Argon2 PHC string — which is what
//! gets persisted. Hashing happens in the add/regenerate handler, so Save never
//! runs Argon2.

use std::collections::HashSet;

use limedl_core::types::{AppSettings, Aria2Client};
use slint::SharedString;

use crate::Aria2ClientItem;
use crate::bridge::format_timestamp_ms;
use crate::i18n::{self, Language};

/// Text state of one client row while the settings dialog is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aria2ClientText {
    pub id: String,
    pub name: String,
    /// Plaintext token, shown once. Empty for a client that is already stored.
    pub token: String,
    /// Argon2 PHC string of the current token (authoritative, persisted).
    pub token_hash: String,
    pub created_at_ms: i64,
}

impl Aria2ClientText {
    /// Build a fresh row with a newly generated token and its hash.
    ///
    /// The plaintext is carried in `token` for the one-time reveal; only
    /// `token_hash` ever reaches settings.json.
    pub fn generate(name: String) -> Result<Self, String> {
        let token = limedl_core::aria2_rpc::generate_token();
        let token_hash = limedl_core::aria2_rpc::hash_token(&token)?;
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            token,
            token_hash,
            created_at_ms: now_ms(),
        })
    }

    /// Replace this row's token with a freshly generated one.
    pub fn regenerate(&mut self) -> Result<(), String> {
        let token = limedl_core::aria2_rpc::generate_token();
        self.token_hash = limedl_core::aria2_rpc::hash_token(&token)?;
        self.token = token;
        self.created_at_ms = now_ms();
        Ok(())
    }
}

/// Convert the editor rows into persisted clients.
///
/// Names must be non-empty and unique (case-insensitively): they are how a user
/// tells two clients apart. A row without a hash can never authenticate, so it
/// is rejected instead of being silently persisted as a dead entry.
pub fn parse_aria2_clients(
    rows: &[Aria2ClientText],
    lang: Language,
) -> Result<Vec<Aria2Client>, String> {
    let mut clients = Vec::with_capacity(rows.len());
    let mut seen: HashSet<String> = HashSet::new();
    for row in rows {
        let name = row.name.trim();
        if name.is_empty() {
            return Err(i18n::format_validation_client_name_empty(lang));
        }
        if !seen.insert(name.to_lowercase()) {
            return Err(i18n::format_validation_client_name_duplicate(lang, name));
        }
        let token_hash = row.token_hash.trim();
        if token_hash.is_empty() {
            return Err(i18n::format_validation_client_token_missing(lang, name));
        }
        clients.push(Aria2Client {
            id: if row.id.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                row.id.clone()
            },
            name: name.to_string(),
            token_hash: token_hash.to_string(),
            created_at_ms: row.created_at_ms,
        });
    }
    Ok(clients)
}

/// Build the editor rows from persisted settings (never carries a plaintext).
pub fn aria2_clients_from_settings(settings: &AppSettings) -> Vec<Aria2ClientText> {
    settings
        .aria2_rpc
        .clients
        .iter()
        .map(|client| Aria2ClientText {
            id: client.id.clone(),
            name: client.name.clone(),
            token: String::new(),
            token_hash: client.token_hash.clone(),
            created_at_ms: client.created_at_ms,
        })
        .collect()
}

/// Build the Slint model items for the editor.
pub fn aria2_clients_to_slint(rows: &[Aria2ClientText]) -> Vec<Aria2ClientItem> {
    rows.iter()
        .map(|row| Aria2ClientItem {
            id: SharedString::from(row.id.as_str()),
            name: SharedString::from(row.name.as_str()),
            token: SharedString::from(row.token.as_str()),
            token_hash: SharedString::from(row.token_hash.as_str()),
            created_at_ms: SharedString::from(row.created_at_ms.to_string()),
            created_text: SharedString::from(if row.created_at_ms > 0 {
                format_timestamp_ms(row.created_at_ms as u64)
            } else {
                String::new()
            }),
        })
        .collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use limedl_core::types::{Aria2AuthMode, Aria2RpcSettings};

    fn settings_with(clients: Vec<Aria2Client>) -> AppSettings {
        AppSettings {
            aria2_rpc: Aria2RpcSettings {
                enabled: true,
                port: 6800,
                secret: None,
                auth_mode: Aria2AuthMode::PerClient,
                clients,
                cors_allowed_origins: vec![],
                listen_address: "127.0.0.1".into(),
                allow_any_origin: false,
                exit_on_shutdown: false,
            },
            ..AppSettings::default()
        }
    }

    fn row(id: &str, name: &str, hash: &str) -> Aria2ClientText {
        Aria2ClientText {
            id: id.to_string(),
            name: name.to_string(),
            token: String::new(),
            token_hash: hash.to_string(),
            created_at_ms: 1,
        }
    }

    /// Generated rows carry a plaintext token and a distinct Argon2 hash; the
    /// plaintext must never appear inside the hash.
    #[test]
    fn generate_produces_a_token_and_hash() {
        let a = Aria2ClientText::generate("A".into()).expect("generate a");
        let b = Aria2ClientText::generate("B".into()).expect("generate b");
        assert_eq!(a.token.len(), 43);
        assert!(a.token_hash.starts_with("$argon2id$"));
        assert_ne!(a.token, b.token);
        assert_ne!(a.token_hash, b.token_hash);
        assert!(!a.token_hash.contains(&a.token));
    }

    #[test]
    fn regenerate_replaces_token_and_hash() {
        let mut row = Aria2ClientText::generate("A".into()).expect("generate");
        let old_token = row.token.clone();
        let old_hash = row.token_hash.clone();
        row.regenerate().expect("regenerate");
        assert_ne!(row.token, old_token);
        assert_ne!(row.token_hash, old_hash);
        assert_eq!(row.name, "A");
    }

    #[test]
    fn parse_rejects_empty_and_duplicate_names() {
        let empty = vec![row("a", "   ", "$argon2id$h")];
        assert!(parse_aria2_clients(&empty, Language::EnUs).is_err());

        let dupes = vec![row("a", "Phone", "$argon2id$h"), row("b", "phone", "$argon2id$h")];
        assert!(parse_aria2_clients(&dupes, Language::EnUs).is_err());

        let missing_hash = vec![row("a", "Phone", "  ")];
        assert!(parse_aria2_clients(&missing_hash, Language::EnUs).is_err());
    }

    /// Save keeps the row's hash (never re-hashes), trims the name, and mints an
    /// id for a row that arrived without one.
    #[test]
    fn parse_trims_names_and_assigns_missing_ids() {
        let rows = vec![row("", "  Phone  ", "  $argon2id$hash  ")];
        let clients = parse_aria2_clients(&rows, Language::EnUs).expect("parse");
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].name, "Phone");
        assert_eq!(clients[0].token_hash, "$argon2id$hash");
        assert!(!clients[0].id.is_empty());
    }

    #[test]
    fn settings_round_trip_carries_hash_but_never_a_plaintext() {
        let settings = settings_with(vec![Aria2Client {
            id: "a".into(),
            name: "Phone".into(),
            token_hash: "$argon2id$hash".into(),
            created_at_ms: 5,
        }]);
        let rows = aria2_clients_from_settings(&settings);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].token.is_empty());
        assert_eq!(rows[0].token_hash, "$argon2id$hash");

        let back = parse_aria2_clients(&rows, Language::EnUs).expect("parse");
        assert_eq!(back, settings.aria2_rpc.clients);
    }
}
