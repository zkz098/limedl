//! Fetches the pinned MiSans VF variable font into
//! `crates/limedl-native/assets/fonts/`, where `build.rs` embeds it at compile
//! time.
//!
//! Migrated from `scripts/fetch-misans.ps1`. The transport hardening from the
//! 2026-10-01 CDN outage is preserved; the difference is that the logic now
//! lives next to the rest of the release tooling and is covered by tests
//! (zip parsing, extraction, verification, and an end-to-end range fetch
//! against a local HTTP server).
//!
//! Why the fetch still drives `curl` instead of an in-process HTTP client: the
//! transport ladder (`--http2` → `--http1.1` → `--http1.1 --ipv4`) is what
//! recovered CI from that outage, and its semantics *are* curl's flags
//! (`--retry-all-errors`, `--speed-limit/--speed-time`, `--continue-at -`). A
//! Rust stack would be a second implementation of the same policy plus a large
//! dependency in a job that only needs ~16 MB of the 217 MB archive. curl ships
//! with Windows 10+, macOS and every Linux CI image; `--from-path` /
//! `LIMEDL_MISANS_TTF` is the escape hatch when it does not.
//!
//! Behavior, unchanged from the script it replaces:
//!   * `--verify` checks the local copy only; never touches the network. The CI
//!     jobs that receive the font as an artifact run this.
//!   * `--force` re-downloads even when the local copy already matches.
//!   * `--from-path` / `LIMEDL_MISANS_TTF` takes an extracted `.ttf` from a
//!     local file or an alternate URL; the pinned size/sha256 is still enforced.
//!   * Otherwise the zip entry is fetched with HTTP range requests (2 MiB
//!     chunks), falling back to the full archive with resume + retries, across
//!     two passes 30 s apart. Nothing lands in the repo until the extracted
//!     font matches the pinned size + sha256; the verified file is moved into
//!     place afterwards, so a failed fetch can never destroy a good local copy.

use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use flate2::read::DeflateDecoder;

use crate::sha256_hex;

// ── Pinned font build (update together when Xiaomi ships a new font) ────────
const ENTRY_SUFFIX: &str = "MiSansVF.ttf"; // unique entry name inside the zip
const EXPECTED_SIZE: u64 = 20_093_424;
const EXPECTED_SHA256: &str = "0ddef90648998900175cfdca9a6f087a2544c182f130b0ad4f7e94a03a115e79";

pub const DEFAULT_ZIP_URL: &str = "https://hyperos.mi.com/font-download/MiSans.zip";

const FONT_DIR: &str = "crates/limedl-native/assets/fonts";
const FONT_NAME: &str = "MiSansVF.ttf";
const ENV_FROM_PATH: &str = "LIMEDL_MISANS_TTF";

// ── Tuning (the constants the PowerShell script used) ───────────────────────
const CHUNK_SIZE: u64 = 2 * 1024 * 1024; // range-fetch granularity for the entry
const TAIL_SIZE: u64 = 1024 * 1024; // end-of-central-directory window
const LOCAL_HEADER_BYTES: usize = 512; // enough for the local header + name/extra
const PROBE_TIMEOUT: &str = "60"; // 1-byte size probe / HEAD
const CHUNK_TIMEOUT: &str = "120"; // one 2 MiB entry chunk
const FULL_TIMEOUT: &str = "240"; // last resort: the whole 217 MB archive
const PLAIN_TIMEOUT: &str = "300"; // --from-path URL

#[cfg(windows)]
const CURL: &str = "curl.exe";
#[cfg(not(windows))]
const CURL: &str = "curl";

/// CLI options for `cargo xtask fetch-font`.
#[derive(Debug, Default)]
pub struct Options {
    pub force: bool,
    pub verify: bool,
    pub from_path: Option<String>,
    pub zip_url: String,
}

/// Pinned expectation for the fetched font. Production always uses
/// [`Spec::pinned`]; the tests build a synthetic one so the whole fetch path can
/// run against a local server with a few kilobytes instead of Xiaomi's 217 MB
/// archive.
#[derive(Clone, Debug)]
pub struct Spec {
    pub entry_suffix: String,
    pub expected_size: u64,
    pub expected_sha256: String,
}

impl Spec {
    fn pinned() -> Self {
        Self {
            entry_suffix: ENTRY_SUFFIX.to_string(),
            expected_size: EXPECTED_SIZE,
            expected_sha256: EXPECTED_SHA256.to_string(),
        }
    }
}

/// Retry/pass policy. Defaults are the script's constants; tests zero the
/// delays so a failure path does not sleep for 30 s.
#[derive(Clone, Debug)]
pub struct Policy {
    pub passes: u32,
    pub pass_pause: Duration,
    pub retries: u32,
    pub retry_delay: Duration,
    pub full_retries: u32,
    pub full_retry_delay: Duration,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            passes: 2,
            pass_pause: Duration::from_secs(30),
            retries: 2,
            retry_delay: Duration::from_secs(3),
            full_retries: 3,
            full_retry_delay: Duration::from_secs(10),
        }
    }
}

pub fn run(root: &Path, opts: &Options) -> Result<()> {
    let from_path = opts
        .from_path
        .clone()
        .or_else(env_from_path)
        .filter(|value| !value.trim().is_empty());
    run_with(
        root,
        opts,
        from_path.as_deref(),
        &Spec::pinned(),
        &Policy::default(),
    )
}

fn env_from_path() -> Option<String> {
    std::env::var(ENV_FROM_PATH).ok()
}

fn run_with(
    root: &Path,
    opts: &Options,
    from_path: Option<&str>,
    spec: &Spec,
    policy: &Policy,
) -> Result<()> {
    let out_dir = root.join(FONT_DIR);
    let out_file = out_dir.join(FONT_NAME);

    // Early exits first: `--verify` must not even create the font directory.
    if opts.verify {
        if !verify_font(&out_file, spec) {
            annotation("MiSans VF is missing or is not the pinned build (see the log)");
            bail!(
                "MiSans VF missing or not the pinned build at {}\n  \
                 expected: {} bytes, sha256={}\n  \
                 actual:   {}\n\
                 Run: cargo xtask fetch-font",
                out_file.display(),
                spec.expected_size,
                spec.expected_sha256,
                format_state(&out_file)
            );
        }
        println!("MiSans VF present and verified: {}", out_file.display());
        return Ok(());
    }
    if !opts.force && verify_font(&out_file, spec) {
        println!(
            "MiSans VF already present and up to date: {}",
            out_file.display()
        );
        return Ok(());
    }

    fs::create_dir_all(&out_dir).with_context(|| format!("create {}", out_dir.display()))?;
    let tmp = TempDir::create("misans-fetch")?;
    // Staged next to the target so the final move is a same-volume rename: the
    // pinned font either stays exactly as it was, or is replaced by a verified
    // one.
    let staging = Staging::new(&out_dir)?;

    if let Some(source) = from_path {
        import_from_source(source, staging.path(), policy)?;
    } else {
        fetch_via_zip(&opts.zip_url, staging.path(), &tmp, spec, policy)?;
    }

    if !verify_font(staging.path(), spec) {
        annotation("MiSans VF failed the pinned size/sha256 check (did Xiaomi change the font build?)");
        bail!(
            "Fetched font failed verification.\n  \
             expected: {} bytes, sha256={}\n  \
             actual:   {}\n\
             If Xiaomi published a new font build, update ENTRY_SUFFIX / \
             EXPECTED_SIZE / EXPECTED_SHA256 in xtask/src/fetch_font.rs.",
            spec.expected_size,
            spec.expected_sha256,
            format_state(staging.path())
        );
    }

    fs::rename(staging.path(), &out_file)
        .with_context(|| format!("move verified font into {}", out_file.display()))?;
    staging.commit();
    println!("MiSans VF fetched OK: {}", out_file.display());
    Ok(())
}

// ── Fetch paths ─────────────────────────────────────────────────────────────

fn fetch_via_zip(
    url: &str,
    out: &Path,
    tmp: &TempDir,
    spec: &Spec,
    policy: &Policy,
) -> Result<()> {
    let mut failures = Vec::new();
    for pass in 1..=policy.passes {
        if pass > 1 {
            println!("Pass {pass} of {}", policy.passes);
        }
        println!("Fetching MiSans VF via HTTP range requests (about 16 MB)...");
        match read_zip_entry_via_range(url, out, spec, policy) {
            Ok(()) => return Ok(()),
            Err(error) => {
                eprintln!("warning: range extraction failed ({error:#})");
                failures.push(format!("range: {error:#}"));
            }
        }
        if pass < policy.passes {
            // The 217 MB archive is the last resort and only worth trying on
            // the first pass: a CDN that cannot serve a 2 MiB chunk for a
            // minute is not going to serve 217 MB either.
            match read_zip_entry_via_full_download(url, out, tmp, spec, policy) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    eprintln!("warning: full download failed ({error:#})");
                    failures.push(format!("full: {error:#}"));
                }
            }
            let _ = fs::remove_file(out);
            println!("warning: retrying in {} s...", policy.pass_pause.as_secs());
            std::thread::sleep(policy.pass_pause);
        }
    }

    annotation(&format!(
        "MiSans VF could not be fetched from the CDN ({url}) - rerun this job once it recovers"
    ));
    bail!(
        "MiSans VF could not be fetched: {url} is unreachable or serving broken responses (see the warnings above).\n\
         Rerun the job once the CDN recovers, or set {ENV_FROM_PATH} / --from-path to a copy of {ENTRY_SUFFIX} you already have.\n  {}",
        failures.join("\n  ")
    )
}

fn read_zip_entry_via_range(url: &str, out: &Path, spec: &Spec, policy: &Policy) -> Result<()> {
    let curl = Curl::new(policy)?;
    let source = CurlSource { curl: &curl, url };
    extract_entry(&source, &spec.entry_suffix, out)
}

fn read_zip_entry_via_full_download(
    url: &str,
    out: &Path,
    tmp: &TempDir,
    spec: &Spec,
    policy: &Policy,
) -> Result<()> {
    let curl = Curl::new(policy)?;
    let source = CurlSource { curl: &curl, url };
    let total = source.total_len()?;
    println!("Downloading full archive ({} MB)...", total / (1024 * 1024));
    let zip_path = tmp.path().join("MiSans.zip");
    curl.download_resumable(url, &zip_path)?;
    let local = FileSource::open(&zip_path)?;
    extract_entry(&local, &spec.entry_suffix, out)
}

/// `--from-path` / `LIMEDL_MISANS_TTF`: an already-extracted `.ttf`, local or
/// remote. The pinned hash check still runs afterwards.
fn import_from_source(source: &str, out: &Path, policy: &Policy) -> Result<()> {
    let lower = source.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        println!("Fetching MiSans VF from {source} ...");
        let curl = Curl::new(policy)?;
        curl.plain_download(source, out)
    } else {
        let resolved =
            fs::canonicalize(source).with_context(|| format!("resolve {source}"))?;
        println!("Using MiSans VF from {} ...", resolved.display());
        fs::copy(&resolved, out)
            .with_context(|| format!("copy {} to {}", resolved.display(), out.display()))?;
        Ok(())
    }
}

// ── Verification ────────────────────────────────────────────────────────────

fn verify_font(path: &Path, spec: &Spec) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if metadata.len() != spec.expected_size {
        return false;
    }
    match fs::read(path) {
        Ok(bytes) => sha256_hex(&bytes) == spec.expected_sha256,
        Err(_) => false,
    }
}

fn format_state(path: &Path) -> String {
    let Ok(metadata) = fs::metadata(path) else {
        return "missing".to_string();
    };
    match fs::read(path) {
        Ok(bytes) => format!("{} bytes, sha256={}", metadata.len(), sha256_hex(&bytes)),
        Err(error) => format!("{} bytes, unreadable: {error}", metadata.len()),
    }
}

fn annotation(message: &str) {
    // GitHub only parses the rest of the line as the annotation text.
    if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
        println!("::error::{message}");
    }
}

// ── Zip parsing (shared by the remote and downloaded-file paths) ────────────

/// Random-access byte source: an HTTP endpoint or a local file.
trait ZipSource {
    fn total_len(&self) -> Result<u64>;
    /// Fill `buf` completely starting at `offset`.
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<()>;
}

struct ZipEntry {
    method: u16,
    compressed_size: u64,
    local_header_offset: u64,
}

fn extract_entry<S: ZipSource + ?Sized>(source: &S, suffix: &str, out: &Path) -> Result<()> {
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
fn find_entry(tail: &[u8], total: u64, suffix: &str) -> Result<ZipEntry> {
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

// ── curl transport ladder ───────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct CurlTransport {
    label: &'static str,
    args: &'static [&'static str],
}

const HTTP2: CurlTransport = CurlTransport {
    label: "http2",
    args: &["--http2"],
};
const HTTP1: CurlTransport = CurlTransport {
    label: "http1.1",
    args: &["--http1.1"],
};
const IPV4: CurlTransport = CurlTransport {
    label: "ipv4",
    args: &["--http1.1", "--ipv4"],
};

struct Curl {
    ladder: Vec<CurlTransport>,
    retries: u32,
    retry_delay: Duration,
    full_retries: u32,
    full_retry_delay: Duration,
    tmp: TempDir,
}

impl Curl {
    fn new(policy: &Policy) -> Result<Self> {
        let output = Command::new(CURL)
            .arg("--version")
            .output()
            .with_context(|| {
                format!(
                    "run {CURL} --version (curl is required to fetch the font; install it or \
                     use --from-path / {ENV_FROM_PATH} with an existing copy)"
                )
            })?;
        if !output.status.success() {
            bail!("{CURL} --version failed ({})", output.status);
        }
        let features = String::from_utf8_lossy(&output.stdout);
        let mut ladder = Vec::new();
        if features_include_http2(&features) {
            ladder.push(HTTP2);
        }
        ladder.push(HTTP1);
        ladder.push(IPV4);
        Ok(Self {
            ladder,
            retries: policy.retries,
            retry_delay: policy.retry_delay,
            full_retries: policy.full_retries,
            full_retry_delay: policy.full_retry_delay,
            tmp: TempDir::create("misans-curl")?,
        })
    }

    /// The flags every request carries. The retry/speed flags are deliberate:
    /// curl's own retry cannot recover from a stream killed mid-body, but it
    /// does ride out transient failures, and the ladder below handles what it
    /// cannot.
    fn common(
        &self,
        transport: CurlTransport,
        timeout: &str,
        retries: u32,
        retry_delay: Duration,
    ) -> Vec<String> {
        let mut args: Vec<String> = ["--silent", "--show-error", "--fail", "--location"]
            .iter()
            .map(|arg| (*arg).to_string())
            .collect();
        args.extend(transport.args.iter().map(|arg| (*arg).to_string()));
        args.extend(
            [
                "--retry",
                &retries.to_string(),
                "--retry-delay",
                &retry_delay.as_secs().to_string(),
                "--retry-all-errors",
                "--speed-limit",
                "1024",
                "--speed-time",
                "30",
                "--connect-timeout",
                "20",
                "--max-time",
                timeout,
            ]
            .iter()
            .map(|arg| (*arg).to_string()),
        );
        args
    }

    /// 1-byte ranged GET (`Content-Range`) first, HEAD `Content-Length` second.
    /// The range probe doubles as a check that the endpoint serves ranges at
    /// all; HEAD alone silently downgraded the fast path during the outage.
    fn probe_total(&self, url: &str) -> Result<u64> {
        let mut failures = Vec::new();
        for transport in &self.ladder {
            let header_path = self.tmp.path().join("probe-headers.txt");
            let body_path = self.tmp.path().join("size-probe.bin");
            let mut args = self.common(*transport, PROBE_TIMEOUT, 0, Duration::ZERO);
            args.extend([
                "-r".to_string(),
                "0-0".to_string(),
                "--dump-header".to_string(),
                header_path.display().to_string(),
                "--output".to_string(),
                body_path.display().to_string(),
                url.to_string(),
            ]);
            match run_curl(&args) {
                Ok(_) => {
                    let headers = fs::read_to_string(&header_path)
                        .with_context(|| format!("read {}", header_path.display()))?;
                    if let Some(total) = parse_content_range_total(&headers)
                        && total > 0
                    {
                        return Ok(total);
                    }
                }
                Err(error) => failures.push(format!("{} (range probe): {error:#}", transport.label)),
            }

            let mut args = self.common(*transport, PROBE_TIMEOUT, 0, Duration::ZERO);
            args.push("--head".to_string());
            args.push(url.to_string());
            match run_curl(&args) {
                Ok(headers) => {
                    let headers = String::from_utf8_lossy(&headers);
                    if let Some(total) = parse_content_length(&headers)
                        && total > 0
                    {
                        return Ok(total);
                    }
                }
                Err(error) => failures.push(format!("{} (HEAD): {error:#}", transport.label)),
            }
        }
        bail!("server did not report content length\n  {}", failures.join("\n  "))
    }

    /// One ranged read, walked across the ladder. The response must be 206 with
    /// exactly the requested number of bytes; a 200 means the server ignored
    /// the range and would hand back the whole archive.
    fn range(&self, url: &str, start: u64, end: u64, buf: &mut [u8]) -> Result<()> {
        let mut failures = Vec::new();
        for transport in &self.ladder {
            match self.range_once(*transport, url, start, end, buf) {
                Ok(()) => return Ok(()),
                Err(error) => failures.push(format!("{}: {error:#}", transport.label)),
            }
        }
        bail!("all transports failed -- {}", failures.join(" | "))
    }

    fn range_once(
        &self,
        transport: CurlTransport,
        url: &str,
        start: u64,
        end: u64,
        buf: &mut [u8],
    ) -> Result<()> {
        let header_path = self.tmp.path().join("range-headers.txt");
        let mut args = self.common(transport, CHUNK_TIMEOUT, self.retries, self.retry_delay);
        args.extend([
            "-r".to_string(),
            format!("{start}-{end}"),
            "--dump-header".to_string(),
            header_path.display().to_string(),
            url.to_string(),
        ]);
        let body = run_curl(&args)?;
        let headers = fs::read_to_string(&header_path)
            .with_context(|| format!("read {}", header_path.display()))?;
        let status = parse_status(&headers).context("curl produced no HTTP status line")?;
        if status != 206 {
            bail!("range request returned HTTP {status} (the server ignored the range)");
        }
        if body.len() != buf.len() {
            bail!(
                "short range read {start}-{end}: got {} of {} bytes",
                body.len(),
                buf.len()
            );
        }
        buf.copy_from_slice(&body);
        Ok(())
    }

    /// Plain `--output` download with the ladder. Used for whole files (the
    /// archive fallback and `--from-path` URLs).
    fn to_file(
        &self,
        url: &str,
        extra: &[&str],
        out: &Path,
        timeout: &str,
        retries: u32,
        retry_delay: Duration,
    ) -> Result<()> {
        let mut failures = Vec::new();
        for transport in &self.ladder {
            let mut args = self.common(*transport, timeout, retries, retry_delay);
            args.extend(extra.iter().map(|arg| (*arg).to_string()));
            args.push(url.to_string());
            args.extend(["--output".to_string(), out.display().to_string()]);
            match run_curl(&args) {
                Ok(_) => return Ok(()),
                Err(error) => failures.push(format!("{}: {error:#}", transport.label)),
            }
        }
        bail!("all transports failed -- {}", failures.join(" | "))
    }

    fn download_resumable(&self, url: &str, out: &Path) -> Result<()> {
        // `--continue-at -` resumes a partial file, so the retry flags only
        // ever re-fetch the bytes that are still missing.
        self.to_file(
            url,
            &["--continue-at", "-"],
            out,
            FULL_TIMEOUT,
            self.full_retries,
            self.full_retry_delay,
        )
    }

    fn plain_download(&self, url: &str, out: &Path) -> Result<()> {
        self.to_file(url, &[], out, PLAIN_TIMEOUT, self.retries, self.retry_delay)
    }
}

struct CurlSource<'a> {
    curl: &'a Curl,
    url: &'a str,
}

impl ZipSource for CurlSource<'_> {
    fn total_len(&self) -> Result<u64> {
        self.curl.probe_total(self.url)
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<()> {
        if buf.is_empty() {
            return Ok(());
        }
        self.curl
            .range(self.url, offset, offset + buf.len() as u64 - 1, buf)
    }
}

struct FileSource {
    file: Mutex<fs::File>,
}

impl FileSource {
    fn open(path: &Path) -> Result<Self> {
        let file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }
}

impl ZipSource for FileSource {
    fn total_len(&self) -> Result<u64> {
        let file = self.file.lock().map_err(|_| anyhow::anyhow!("file lock poisoned"))?;
        Ok(file.metadata().context("stat zip file")?.len())
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<()> {
        let mut file = self.file.lock().map_err(|_| anyhow::anyhow!("file lock poisoned"))?;
        file.seek(SeekFrom::Start(offset))
            .context("seek in zip file")?;
        file.read_exact(buf).context("read from zip file")?;
        Ok(())
    }
}

fn run_curl(args: &[String]) -> Result<Vec<u8>> {
    let output = Command::new(CURL)
        .args(args)
        .output()
        .with_context(|| format!("run {CURL}"))?;
    if !output.status.success() {
        let code = output
            .status
            .code()
            .map_or_else(|| "signal".to_string(), |code| code.to_string());
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("curl exited with {code}: {}", stderr.trim());
    }
    Ok(output.stdout)
}

fn features_include_http2(version: &str) -> bool {
    version.lines().any(|line| {
        line.trim_start().starts_with("Features:")
            && line.split_whitespace().any(|feature| feature == "HTTP2")
    })
}

fn parse_content_range_total(headers: &str) -> Option<u64> {
    headers.lines().rev().find_map(|line| {
        let rest = line.to_ascii_lowercase();
        let rest = rest.trim_start().strip_prefix("content-range:")?;
        let (_, total) = rest.trim().rsplit_once('/')?;
        total.trim().parse().ok()
    })
}

fn parse_content_length(headers: &str) -> Option<u64> {
    headers
        .lines()
        .filter_map(|line| {
            let lower = line.to_ascii_lowercase();
            lower
                .strip_prefix("content-length:")
                .map(str::trim)
                .map(str::to_string)
        })
        .filter_map(|value| value.parse().ok())
        .next_back()
}

fn parse_status(headers: &str) -> Option<u16> {
    headers.lines().rev().find_map(|line| {
        let mut parts = line.split_whitespace();
        let protocol = parts.next()?;
        if !protocol.starts_with("HTTP/") {
            return None;
        }
        parts.next()?.parse().ok()
    })
}

// ── Filesystem guards ───────────────────────────────────────────────────────

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn create(tag: &str) -> Result<Self> {
        let path = std::env::temp_dir().join(format!("{tag}-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&path).with_context(|| format!("create {}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// A staging file next to the target, removed unless [`Staging::commit`] runs.
struct Staging {
    path: PathBuf,
    active: bool,
}

impl Staging {
    fn new(dir: &Path) -> Result<Self> {
        Ok(Self {
            path: dir.join(format!("{FONT_NAME}.staging-{}", uuid::Uuid::new_v4().simple())),
            active: true,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn commit(mut self) {
        self.active = false;
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "limedl-xtask-font-{tag}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Minimal zip writer: local headers + central directory + EOCD, with
    /// stored or raw-deflate entries.
    fn build_zip(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data, compress) in entries {
            let offset = out.len() as u32;
            let payload = if *compress {
                deflate_bytes(data)
            } else {
                data.to_vec()
            };
            let method: u16 = if *compress { 8 } else { 0 };

            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes()); // version needed
            out.extend_from_slice(&0u16.to_le_bytes()); // flags
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // time
            out.extend_from_slice(&0u16.to_le_bytes()); // date
            out.extend_from_slice(&0u32.to_le_bytes()); // crc32 (not read back)
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&payload);

            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes()); // version made
            central.extend_from_slice(&20u16.to_le_bytes()); // version needed
            central.extend_from_slice(&0u16.to_le_bytes()); // flags
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes()); // time
            central.extend_from_slice(&0u16.to_le_bytes()); // date
            central.extend_from_slice(&0u32.to_le_bytes()); // crc32
            central.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes()); // extra
            central.extend_from_slice(&0u16.to_le_bytes()); // comment
            central.extend_from_slice(&0u16.to_le_bytes()); // disk
            central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
            central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_offset = out.len() as u32;
        let cd_size = central.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    fn deflate_bytes(data: &[u8]) -> Vec<u8> {
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    struct MemorySource(Vec<u8>);

    impl ZipSource for MemorySource {
        fn total_len(&self) -> Result<u64> {
            Ok(self.0.len() as u64)
        }

        fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<()> {
            let start = offset as usize;
            let slice = self
                .0
                .get(start..start + buf.len())
                .context("read past end of memory source")?;
            buf.copy_from_slice(slice);
            Ok(())
        }
    }

    #[test]
    fn finds_and_extracts_stored_and_deflated_entries() {
        let dir = temp_dir("zip");
        let zip = build_zip(&[
            ("docs/readme.txt", b"ignore me", false),
            ("MiSans/MiSansVF.ttf", b"the font bytes", true),
        ]);
        let source = MemorySource(zip.clone());

        let entry = find_entry(&zip, zip.len() as u64, "MiSansVF.ttf").unwrap();
        assert_eq!(entry.method, 8);
        assert_eq!(
            entry.compressed_size,
            deflate_bytes(b"the font bytes").len() as u64
        );

        let out = dir.join("font.ttf");
        extract_entry(&source, "MiSansVF.ttf", &out).unwrap();
        assert_eq!(fs::read(&out).unwrap(), b"the font bytes");

        let zip = build_zip(&[("MiSansVF.ttf", b"stored bytes", false)]);
        let source = MemorySource(zip.clone());
        let out = dir.join("stored.ttf");
        extract_entry(&source, "MiSansVF.ttf", &out).unwrap();
        assert_eq!(fs::read(&out).unwrap(), b"stored bytes");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_archive_without_the_entry() {
        let zip = build_zip(&[("other.txt", b"x", false)]);
        let source = MemorySource(zip.clone());
        let err = extract_entry(&source, "MiSansVF.ttf", Path::new("/nonexistent")).unwrap_err();
        assert!(format!("{err:#}").contains("not found"), "{err:#}");
    }

    #[test]
    fn verifies_size_and_hash() {
        let dir = temp_dir("verify");
        let path = dir.join("font.ttf");
        fs::write(&path, b"abc").unwrap();
        let spec = Spec {
            entry_suffix: "MiSansVF.ttf".to_string(),
            expected_size: 3,
            expected_sha256:
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string(),
        };
        assert!(verify_font(&path, &spec));
        assert!(!verify_font(&dir.join("missing.ttf"), &spec));
        assert!(!verify_font(
            &path,
            &Spec {
                expected_size: 4,
                ..spec.clone()
            }
        ));
        assert!(!verify_font(
            &path,
            &Spec {
                expected_sha256: "00".repeat(32),
                ..spec
            }
        ));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_probe_headers_and_features() {
        assert_eq!(
            parse_content_range_total("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-0/227880072\r\n"),
            Some(227_880_072)
        );
        assert_eq!(parse_content_range_total("Content-Range: bytes 0-0/5"), Some(5));
        assert_eq!(parse_content_range_total("Content-Length: 5"), None);
        assert_eq!(
            parse_content_length("HTTP/1.1 200 OK\nContent-Length: 12\n\n"),
            Some(12)
        );
        assert_eq!(
            parse_status("HTTP/1.1 302 Found\r\n\r\nHTTP/2 206\r\n"),
            Some(206)
        );
        assert!(features_include_http2("Features: alt-svc AsynchDNS HSTS HTTP2\n"));
        assert!(!features_include_http2("Features: alt-svc AsynchDNS\n"));
    }

    #[test]
    fn skips_work_when_the_font_is_already_pinned() {
        let dir = temp_dir("present");
        let payload = b"a pinned font".to_vec();
        let spec = Spec {
            entry_suffix: "MiSansVF.ttf".to_string(),
            expected_size: payload.len() as u64,
            expected_sha256: sha256_hex(&payload),
        };
        let font_dir = dir.join(FONT_DIR);
        fs::create_dir_all(&font_dir).unwrap();
        fs::write(font_dir.join(FONT_NAME), &payload).unwrap();

        let opts = Options {
            from_path: Some(dir.join("does-not-exist.ttf").display().to_string()),
            ..Options::default()
        };
        // No `--force`: the existing copy wins and the bogus --from-path is
        // never consulted.
        run_with(&dir, &opts, Some("does-not-exist.ttf"), &spec, &Policy::default()).unwrap();
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn copies_from_a_local_path() {
        let dir = temp_dir("from-path");
        let payload = b"a pinned font".to_vec();
        let source = dir.join("MiSansVF.ttf");
        fs::write(&source, &payload).unwrap();
        let spec = Spec {
            entry_suffix: "MiSansVF.ttf".to_string(),
            expected_size: payload.len() as u64,
            expected_sha256: sha256_hex(&payload),
        };
        let opts = Options::default();
        run_with(
            &dir,
            &opts,
            Some(source.to_str().unwrap()),
            &spec,
            &Policy::default(),
        )
        .unwrap();
        assert_eq!(fs::read(dir.join(FONT_DIR).join(FONT_NAME)).unwrap(), payload);
        fs::remove_dir_all(&dir).ok();
    }

    /// End-to-end fast path against a local server: probe, tail, entry chunks,
    /// deflate, pin check, atomic move.
    #[test]
    fn fetches_through_http_range_requests() {
        if Command::new(CURL).arg("--version").output().is_err() {
            eprintln!("skipping: curl is not installed");
            return;
        }

        let payload = b"the embedded font payload".repeat(4096);
        let zip = build_zip(&[("MiSans/MiSansVF.ttf", &payload, true)]);
        let server = TestServer::serve(zip);
        let dir = temp_dir("e2e");
        let spec = Spec {
            entry_suffix: "MiSansVF.ttf".to_string(),
            expected_size: payload.len() as u64,
            expected_sha256: sha256_hex(&payload),
        };
        let opts = Options {
            zip_url: server.url.clone(),
            ..Options::default()
        };
        let policy = Policy {
            passes: 1,
            pass_pause: Duration::ZERO,
            ..Policy::default()
        };

        run_with(&dir, &opts, None, &spec, &policy).unwrap();
        let written = fs::read(dir.join(FONT_DIR).join(FONT_NAME)).unwrap();
        assert_eq!(written, payload);

        // A second run is a no-op and does not need the server to be alive.
        run_with(&dir, &opts, None, &spec, &policy).unwrap();
        fs::remove_dir_all(&dir).ok();
    }

    /// A tiny HTTP/1.1 server with byte-range support, just enough for curl.
    struct TestServer {
        url: String,
    }

    impl TestServer {
        fn serve(zip: Vec<u8>) -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { break };
                    let Ok(clone) = stream.try_clone() else { continue };
                    let mut reader = std::io::BufReader::new(clone);
                    let mut request_line = String::new();
                    if std::io::BufRead::read_line(&mut reader, &mut request_line).unwrap_or(0) == 0
                    {
                        continue;
                    }
                    let mut range = None;
                    loop {
                        let mut line = String::new();
                        if std::io::BufRead::read_line(&mut reader, &mut line).unwrap_or(0) == 0
                            || line == "\r\n"
                            || line == "\n"
                        {
                            break;
                        }
                        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("range:") {
                            range = parse_test_range(rest);
                        }
                    }
                    let method = request_line.split_whitespace().next().unwrap_or("");
                    let total = zip.len() as u64;
                    let (status, start, end) = if method == "HEAD" {
                        ("200 OK", 0, total - 1)
                    } else if let Some((start, end)) = range {
                        ("206 Partial Content", start.min(total - 1), end.min(total - 1))
                    } else {
                        ("200 OK", 0, total - 1)
                    };
                    let body: &[u8] = if method == "HEAD" {
                        &[]
                    } else {
                        &zip[start as usize..=end as usize]
                    };
                    let mut response =
                        format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\n", body.len());
                    if status.starts_with("206") {
                        response.push_str(&format!(
                            "Content-Range: bytes {start}-{end}/{total}\r\n"
                        ));
                    }
                    response.push_str("Accept-Ranges: bytes\r\nConnection: close\r\n\r\n");
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.write_all(body);
                    let _ = stream.flush();
                }
            });
            Self {
                url: format!("http://{addr}/MiSans.zip"),
            }
        }
    }

    fn parse_test_range(rest: &str) -> Option<(u64, u64)> {
        let rest = rest.trim();
        let rest = rest.strip_prefix("bytes=")?;
        let (start, end) = rest.split_once('-')?;
        Some((start.parse().ok()?, end.parse().ok()?))
    }
}
