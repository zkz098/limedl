//! Write-coalescing primitives shared by the buffer pool and the device queues.
//!
//! These live in `file_ops` — the lowest layer that touches the file — because
//! both `buffer_pool::worker` (the runtime path) and `io_scheduler::queue` (the
//! per-device queue threads) perform the same coalesced write.  Keeping them in
//! `buffer_pool` made `io_scheduler` depend on `buffer_pool` while
//! `buffer_pool` depended on `io_scheduler` for device metrics — a cycle the
//! module-boundary guard rejects.

use std::fs::File;

use bytes::Bytes;

use super::{write_all_at, write_all_vectored_at};
use crate::error::DownloadError;

/// Write sync policy for IoWorker batch processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// No sync (SSD mode).
    None,
    /// Adaptive sync (HDD periodic/volume threshold: >=16MB or >=3s).
    Adaptive,
    /// Force immediate sync (on pause, cancel, finish, or flush_all).
    Force,
}

/// Write a list of offset-sorted byte chunks to disk, coalescing adjacent entries
/// into a single system call / vectored I/O (pwritev on Unix) to drastically reduce
/// context switches and eliminate memory allocations.
pub fn write_coalesced_entries(file: &File, entries: &[(u64, Bytes)]) -> Result<(), DownloadError> {
    if entries.is_empty() {
        return Ok(());
    }
    if entries.len() == 1 {
        return write_all_at(file, &entries[0].1, entries[0].0);
    }

    let mut idx = 0;
    while idx < entries.len() {
        let start_offset = entries[idx].0;
        let mut end_offset = start_offset + entries[idx].1.len() as u64;
        let mut next_idx = idx + 1;

        // Group all contiguous slices
        while next_idx < entries.len() && entries[next_idx].0 == end_offset {
            end_offset += entries[next_idx].1.len() as u64;
            next_idx += 1;
        }

        if next_idx == idx + 1 {
            // Single chunk, direct zero-copy write
            write_all_at(file, &entries[idx].1, start_offset)?;
        } else {
            // Multiple adjacent chunks: zero-copy vectored write (Unix) or coalesced single syscall (Windows)
            let slices: Vec<&[u8]> = entries[idx..next_idx]
                .iter()
                .map(|(_, d)| d.as_ref())
                .collect();
            write_all_vectored_at(file, &slices, start_offset)?;
        }

        idx = next_idx;
    }
    Ok(())
}
