use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use flate2::read::DeflateDecoder;

use super::{CHUNK_SIZE, LOCAL_HEADER_BYTES, TAIL_SIZE};

// ── Zip parsing (shared by the remote and downloaded-file paths) ────────────

/// Random-access byte source: an HTTP endpoint or a local file.
pub(super) trait ZipSource {
    fn total_len(&self) -> Result<u64>;
    /// Fill `buf` completely starting at `offset`.
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<()>;
}

pub(super) struct ZipEntry {
    pub(super) method: u16,
    pub(super) compressed_size: u64,
    local_header_offset: u64,
}

pub(super) fn extract_entry<S: ZipSource + ?Sized>(
    source: &S,
    suffix: &str,
    out: &Path,
) -> Result<()> {
    let total = source.total_len()?;
    if total == 0 {
        bail!("server did not report content length");
    }

    // End-of-central-directory record, from the last 1 MiB of the archive.
    let tail_len = total.min(TAIL_SIZE) as usize;
    let mut tail = vec![0u8; tail_len];
    source.read_at(total - tail_len as u64, &mut tail)?;
    let entry = find_entry(&tail, total, suffix)?;

    // Local file header: 30 fixed bytes, then the (variable) name and extra
    // fields that precede the entry data.
    let head_len = total
        .checked_sub(entry.local_header_offset)
        .context("local file header offset past end of archive")?
        .min(LOCAL_HEADER_BYTES as u64) as usize;
    let mut head = vec![0u8; head_len];
    source.read_at(entry.local_header_offset, &mut head)?;
    if u32_at(&head, 0)? != 0x0403_4b50 {
        bail!("bad local file header");
    }
    let name_len = u16_at(&head, 26)? as u64;
    let extra_len = u16_at(&head, 28)? as u64;
    let data_start = entry
        .local_header_offset
        .checked_add(30 + name_len + extra_len)
        .context("local file header size overflow")?;
    let data_end = data_start
        .checked_add(entry.compressed_size)
        .context("zip entry size overflow")?;
    if data_end > total {
        bail!("entry {suffix} extends past the end of the archive");
    }

    let reader = SourceSlice::new(source, data_start, entry.compressed_size);
    match entry.method {
        // 0 = stored, 8 = deflate; anything else is valid zip but not what
        // Xiaomi publishes.
        0 => copy_to_file(reader, out),
        8 => copy_to_file(DeflateDecoder::new(reader), out),
        method => bail!("unsupported zip compression method {method}"),
    }
}

fn copy_to_file<R: Read>(mut reader: R, out: &Path) -> Result<()> {
    let mut file = fs::File::create(out).with_context(|| format!("create {}", out.display()))?;
    io::copy(&mut reader, &mut file).context("write extracted font")?;
    file.flush().context("flush extracted font")?;
    Ok(())
}

/// Walk the central directory in the fetched tail. The archive size is known
/// and the tail starts at `total - tail.len()`, so the central directory's
/// busiest part is guaranteed to be in the buffer for this archive.
pub(super) fn find_entry(tail: &[u8], total: u64, suffix: &str) -> Result<ZipEntry> {
    let eocd = find_eocd(tail)?;
    let count = u16_at(tail, eocd + 10)? as usize;
    let cd_size = u32_at(tail, eocd + 12)? as u64;
    let cd_offset = u32_at(tail, eocd + 16)? as u64;
    if cd_size > tail.len() as u64 {
        bail!("central directory larger than fetched tail");
    }
    let mut pos = u64::try_from(tail.len())
        .expect("tail length fits in u64")
        .checked_sub(
            total
                .checked_sub(cd_offset)
                .context("central directory offset past end of archive")?,
        )
        .context("central directory larger than fetched tail")? as usize;

    for _ in 0..count {
        if pos + 46 > tail.len() {
            break;
        }
        let name_len = u16_at(tail, pos + 28)? as usize;
        let extra_len = u16_at(tail, pos + 30)? as usize;
        let comment_len = u16_at(tail, pos + 32)? as usize;
        let name = tail.get(pos + 46..pos + 46 + name_len);
        if let Some(name) = name
            && String::from_utf8_lossy(name).ends_with(suffix)
        {
            return Ok(ZipEntry {
                method: u16_at(tail, pos + 10)?,
                compressed_size: u32_at(tail, pos + 20)? as u64,
                local_header_offset: u32_at(tail, pos + 42)? as u64,
            });
        }
        pos += 46 + name_len + extra_len + comment_len;
    }
    bail!("entry {suffix} not found in zip")
}

fn find_eocd(tail: &[u8]) -> Result<usize> {
    if tail.len() < 22 {
        bail!("zip end-of-central-directory not found");
    }
    for i in (0..=tail.len() - 22).rev() {
        if tail[i..i + 4] == [0x50, 0x4b, 0x05, 0x06] {
            return Ok(i);
        }
    }
    bail!("zip end-of-central-directory not found")
}

fn u16_at(buf: &[u8], pos: usize) -> Result<u16> {
    let bytes: [u8; 2] = buf
        .get(pos..pos + 2)
        .context("truncated zip record")?
        .try_into()
        .expect("slice is two bytes");
    Ok(u16::from_le_bytes(bytes))
}

fn u32_at(buf: &[u8], pos: usize) -> Result<u32> {
    let bytes: [u8; 4] = buf
        .get(pos..pos + 4)
        .context("truncated zip record")?
        .try_into()
        .expect("slice is four bytes");
    Ok(u32::from_le_bytes(bytes))
}

/// Sequential `Read` over a byte range of any [`ZipSource`], refilling in
/// 2 MiB chunks so a dropped connection costs one chunk instead of the entry.
struct SourceSlice<'a, S: ZipSource + ?Sized> {
    source: &'a S,
    offset: u64,
    remaining: u64,
    buffer: Vec<u8>,
    position: usize,
}

impl<'a, S: ZipSource + ?Sized> SourceSlice<'a, S> {
    fn new(source: &'a S, offset: u64, remaining: u64) -> Self {
        Self {
            source,
            offset,
            remaining,
            buffer: Vec::new(),
            position: 0,
        }
    }
}

impl<S: ZipSource + ?Sized> Read for SourceSlice<'_, S> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Ok(0);
        }
        if self.position == self.buffer.len() {
            let want = CHUNK_SIZE.min(self.remaining) as usize;
            self.buffer.resize(want, 0);
            self.source
                .read_at(self.offset, &mut self.buffer)
                .map_err(io::Error::other)?;
            self.position = 0;
        }
        let available = &self.buffer[self.position..];
        let count = available.len().min(out.len());
        out[..count].copy_from_slice(&available[..count]);
        self.position += count;
        self.offset += count as u64;
        self.remaining -= count as u64;
        Ok(count)
    }
}
