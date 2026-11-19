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
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use crate::sha256_hex;
use anyhow::{Context, Result, bail};

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
        annotation(
            "MiSans VF failed the pinned size/sha256 check (did Xiaomi change the font build?)",
        );
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

fn fetch_via_zip(url: &str, out: &Path, tmp: &TempDir, spec: &Spec, policy: &Policy) -> Result<()> {
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
        let resolved = fs::canonicalize(source).with_context(|| format!("resolve {source}"))?;
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

mod zip;
#[cfg(test)]
use zip::find_entry;
use zip::{ZipSource, extract_entry};

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
                Err(error) => {
                    failures.push(format!("{} (range probe): {error:#}", transport.label))
                }
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
        bail!(
            "server did not report content length\n  {}",
            failures.join("\n  ")
        )
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
        let file = self
            .file
            .lock()
            .map_err(|_| anyhow::anyhow!("file lock poisoned"))?;
        Ok(file.metadata().context("stat zip file")?.len())
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<()> {
        let mut file = self
            .file
            .lock()
            .map_err(|_| anyhow::anyhow!("file lock poisoned"))?;
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
            path: dir.join(format!(
                "{FONT_NAME}.staging-{}",
                uuid::Uuid::new_v4().simple()
            )),
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
mod tests;
