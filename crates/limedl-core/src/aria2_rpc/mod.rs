
use foldhash::HashMap;



mod bind;
mod context;
mod dispatch;
mod jsonrpc;
mod download;
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
