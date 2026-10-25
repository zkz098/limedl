use foldhash::HashMap;

// The engine modules this crate reads, imported at its root so the moved files
// kept their `crate::dispatcher::…` / `crate::types::…` spellings.
//
// Private, deliberately: these are the engine modules this crate *reads*, not
// part of its API. Re-exporting them made `limedl-rpc` a second facade over
// `limedl-core` — `limedl_rpc::types` and `limedl_core::types` both resolved,
// and nothing told you which one a consumer had picked.
use limedl_core::{
    backend_registry, bt_backend, dispatcher, event_bus, http, manager, manifest, metalink, types,
};
// Only the e2e suite drives a real boot; nothing in this crate's product code
// calls it, which is why it is not in the list above (a `pub use` hid that).
#[cfg(test)]
use limedl_core::bootstrap;
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
