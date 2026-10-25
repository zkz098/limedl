// Integration tests for limedl-core

mod adversarial_interception_tests;
mod bootstrap_tests;
mod bt_backend_tests;
mod buffer_integrity_tests;
mod cdn_e2e_tests;
mod checksum_e2e_tests;
mod corruption_oracle_tests;
mod resume_corruption_tests;
// `pub(crate)`: the aria2 RPC tests reuse `make_manager`/`inject_download` from
// here to cover `resolve_gid` and the GID cache against a real DownloadManager.
mod disk_detect_test;
pub(crate) mod dispatcher_tests;
mod http_executor_tests;
mod manager_tests;
mod mirror_e2e_tests;
mod module_boundaries_tests;
mod persistence_e2e_tests;
mod persistence_tests;
mod protocol_tests;
mod retry_tests;
mod scheduler_concurrency_tests;
mod scheduler_tests;
mod settings_roundtrip_tests;
