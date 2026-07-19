// Integration tests for limedl-core
// Note: commands_tests lives in src-tauri (Tauri crate) since it tests Tauri-layer functions

mod bootstrap_tests;
mod http_executor_tests;
mod manager_tests;
mod persistence_e2e_tests;
mod persistence_tests;
mod retry_tests;
mod scheduler_concurrency_tests;
mod scheduler_tests;
mod settings_roundtrip_tests;
mod cdn_e2e_tests;
mod checksum_e2e_tests;
mod mirror_e2e_tests;

#[cfg(feature = "aria2-rpc")]
mod aria2_ws_e2e_tests;
