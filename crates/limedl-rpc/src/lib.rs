use foldhash::HashMap;

// The engine facade, re-exported as modules so the moved files kept their
// `crate::dispatcher::…` / `crate::types::…` spellings.
pub use limedl_core::{
    backend_registry, bootstrap, bt_backend, dispatcher, event_bus, http, manager, manifest,
    metalink, types,
};
// Test-only, like the modules that use it.
#[cfg(test)]
pub use limedl_core::test_harness;

mod bind;
mod context;
mod dispatch;
mod download;
mod jsonrpc;
mod options;
mod protocol;
mod query;
mod server;
mod system;
mod token;
mod transport;

pub use bind::{format_bind_addr, is_loopback_bind_address};

pub use protocol::internal_id_to_gid;
pub use server::Aria2RpcServer;
pub use system::cleanup_old_aria2_temp_files;
// Token generation/hashing is public for the desktop settings UI, which mints a
// plaintext token once, persists only `token::hash_token`, and shows the
// plaintext to the user exactly once.
pub use token::{generate_token, hash_token};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod e2e_tests;

#[cfg(test)]
mod interop_tests;

#[cfg(test)]
mod oracle_tests;
