# Engine development notes

Cross-cutting rules that are easy to miss when editing the engine.

## Adding a checksum algorithm

The supported set is deliberately narrow: Blake3 (default) / SHA-256 / SHA-512 —
the full range of `ChecksumMode` (plus `None` for "no verification"). Weak and
non-cryptographic hashes (SHA-1, XXH3-128) were removed because a checksum
failure is meant to trigger a re-download, so a collision-forgeable digest defeats
the mechanism. Note `xxhash-rust` is still a dependency, but only for internal ID
derivation (aria2 GID, `download_id`), never for checksums.

`ChecksumHasher` wraps the three algorithms; `mode == None` never calls it (it
returns `Err`). Blake3 output uses `to_hex()`; the two SHA-2 variants share the
module-private `hex_lower()` (64/128 lowercase hex chars). `hash_slices()` is the
synchronous fast path for in-memory buffers.

Adding an algorithm means touching **all** of these, or one site is missed and
verification silently does nothing:

- `crates/limedl-core/src/types/common.rs` — the enum
- `crates/limedl-core/src/checksum/mod.rs` — the hasher and `hash_slices`
- `crates/limedl-core/src/database/manifest_repo.rs` — the text mapping
- `crates/limedl-core/src/aria2_rpc/options.rs` — `TYPE=DIGEST` parsing
- desktop `bridge/forms/enums.rs` + `combo::CHECKSUMS` (index-aligned with the
  `@tr` list in `tab_download.slint`) and the three `.po` catalogs

### Backward compatibility (upgrade path — do not delete)

- Historical `sha1` / `xxh3_128` rows in the database are mapped to `None` by
  `text_to_checksum_mode`, with a warning: the stored digest is from a removed
  algorithm, so mapping it to a live algorithm would fail every re-check for the
  life of the row; mapping to `None` lets the task finish. Unknown values still
  error.
- A leftover `defaultChecksum` in `settings.json` is rewritten to `blake3` by the
  in-place rewrite in `load_settings`; otherwise one enum value fails the whole
  config deserialization and falls back to defaults.

## Async file I/O

Async functions and async blocks must not call `std::fs` directly — that is
SonarQube's `rust:S7493` ("blocking file operation"). Fourteen such sites were
once scattered across `bootstrap` / `manager` / `disk_io` / `bt_backend` /
`http_executor::finalize` / `aria2_rpc` and the desktop `update` and settings
dialogs. Use `tokio::fs` (`create_dir_all` / `metadata` / `rename` / `write` /
`read` / `remove_dir_all`), or wrap the genuinely synchronous part in
`spawn_blocking`.

The exception is **synchronous functions**: `update/mod.rs`'s tar extraction and
`clean_update_work_dir` are synchronous flows, and changing them would drag in a
runtime for no benefit.

The self-update executable is written with mode `0o700`, not `0o755`
(SonarQube `rust:S2612`): the file lives in the user's own update work directory
and is executed only by that user, so group/other read/write/execute bits are
unnecessary surface on a file that subsequently replaces the running process.
