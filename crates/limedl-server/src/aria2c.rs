//! aria2c-compatible CLI — Stage 1: single-file HTTP subset.
//!
//! Provides `limedl aria2c <urls...> [options]` with a hand-rolled argument
//! parser that mirrors aria2c's CLI conventions (`--long=value`, `-s value`,
//! `-s4` compact short form, repeated positional URIs) rather than clap's
//! subcommand model. Downloads are dispatched through the shared `limedl-core`
//! engine (`DownloadManager`).
//!
//! Only the high-frequency subset of aria2c options is implemented; obscure
//! options are intentionally not supported. Exit codes follow aria2c (0 = ok,
//! 1 = generic error, 3 = resource not found).

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use tokio::sync::Semaphore;

const SPLIT_DEFAULT: usize = 5;
const MAX_CONCURRENT_DEFAULT: usize = 5;

/// Parsed aria2c-compatible arguments (Stage 1 subset).
#[derive(Debug, Clone, Default)]
pub struct Aria2cArgs {
    pub urls: Vec<String>,
    pub out: Option<String>,
    pub dir: Option<PathBuf>,
    pub max_connection_per_server: Option<usize>,
    pub split: Option<usize>,
    pub min_split_size: Option<u64>,
    pub resume: bool,
    pub max_download_limit: Option<u64>,
    pub max_overall_download_limit: Option<u64>,
    pub user_agent: Option<String>,
    pub headers: Vec<String>,
    pub max_concurrent_downloads: Option<usize>,
    /// (algorithm-name, hex digest) from `--checksum=TYPE=HASH`
    pub checksum: Option<(String, String)>,
    pub dry_run: bool,
    pub quiet: bool,
    /// `-i` batch input file (URLs, one per line, `#` comments)
    pub input_file: Option<PathBuf>,
    /// `-Z` conditional-get: skip when the output file already exists
    pub conditional_get: bool,
    /// `--select-file` (1-based) — BT file selection
    pub select_files: Vec<usize>,
    /// `-S` show torrent file listing (accepted; engine display limited)
    pub show_files: bool,
    /// `--enable-rpc` config (mode: start an aria2 RPC server)
    pub rpc: Option<RpcConfig>,
    /// Warnings for accepted-but-unwired options
    pub unsupported: Vec<String>,
}

/// RPC (`--enable-rpc`) configuration mapping to the aria2 RPC server.
#[derive(Debug, Clone, Default)]
pub struct RpcConfig {
    pub secret: Option<String>,
    pub port: u16,
    pub listen_all: bool,
}

/// Parse `--size`-style values with K/M/G suffixes (bytes for limits, split …).
fn parse_size(value: &str) -> Result<u64, String> {
    let (num, mult) = match value.chars().last() {
        Some('k') | Some('K') => (&value[..value.len() - 1], 1024u64),
        Some('m') | Some('M') => (&value[..value.len() - 1], 1024 * 1024),
        Some('g') | Some('G') => (&value[..value.len() - 1], 1024 * 1024 * 1024),
        _ => (value, 1),
    };
    num.trim()
        .parse::<u64>()
        .map(|n| n * mult)
        .map_err(|_| format!("invalid size: {value}"))
}

fn parse_pos_usize(value: &str, name: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("invalid value for {name}: {value}"))
}

/// Whether a given option consumes a value (the following token, or `=value`).
fn option_takes_value(name: &str) -> bool {
    matches!(
        name,
        "-d" | "--dir"
            | "-o" | "--out"
            | "-j" | "--max-concurrent-downloads"
            | "-x" | "--max-connection-per-server"
            | "-s" | "--split"
            | "-k" | "--min-split-size"
            | "--max-download-limit"
            | "--max-overall-download-limit"
            | "--user-agent" | "-U"
            | "--header" | "-H"
            | "--checksum"
            | "-i" | "--input-file"
            | "--select-file"
            | "--seed-time"
            | "--rpc-secret"
            | "--rpc-listen-port"
    )
}

/// Apply a single parsed option to the config.
fn apply(name: &str, value: Option<String>, cfg: &mut Aria2cArgs) -> Result<(), String> {
    let need = || value.clone().unwrap_or_default();
    match name {
        "-c" | "--continue" => cfg.resume = true,
        "-q" | "--quiet" => cfg.quiet = true,
        "--dry-run" => cfg.dry_run = true,
        "--version" => return Err(VERSION_MARK.to_string()),
        "--help" | "-h" => return Err(HELP_MARK.to_string()),
        "-d" | "--dir" => cfg.dir = Some(PathBuf::from(need())),
        "-o" | "--out" => cfg.out = Some(need()),
        "-j" | "--max-concurrent-downloads" => {
            cfg.max_concurrent_downloads = Some(parse_pos_usize(&need(), name)?);
        }
        "-x" | "--max-connection-per-server" => {
            cfg.max_connection_per_server = Some(parse_pos_usize(&need(), name)?);
        }
        "-s" | "--split" => cfg.split = Some(parse_pos_usize(&need(), name)?),
        "-k" | "--min-split-size" => cfg.min_split_size = Some(parse_size(&need())?),
        "--max-download-limit" => cfg.max_download_limit = Some(parse_size(&need())?),
        "--max-overall-download-limit" => {
            cfg.max_overall_download_limit = Some(parse_size(&need())?);
        }
        "--user-agent" | "-U" => cfg.user_agent = Some(need()),
        "--header" | "-H" => cfg.headers.push(need()),
        "-i" | "--input-file" => cfg.input_file = Some(PathBuf::from(need())),
        "--select-file" => {
            let v = need();
            match v.parse::<usize>() {
                Ok(n) if n >= 1 => cfg.select_files.push(n),
                _ => return Err(format!("invalid --select-file index: {v} (must be >= 1)")),
            }
        }
        "-Z" | "--conditional-get" => cfg.conditional_get = true,
        "-S" | "--show-files" => cfg.show_files = true,
        "--seed-time" => {
            let v = need();
            v.parse::<u64>()
                .map_err(|_| format!("invalid --seed-time: {v}"))?;
            cfg.unsupported.push("--seed-time is not wired in the CLI engine; ignoring".to_string());
        }
        "--check-integrity" => {
            cfg.unsupported
                .push("--check-integrity is not wired in the CLI engine; ignoring".to_string());
        }
        "--enable-rpc" => {
            cfg.rpc = Some(cfg.rpc.take().unwrap_or_default());
        }
        "--rpc-secret" => {
            let rpc = cfg.rpc.get_or_insert_with(RpcConfig::default);
            rpc.secret = Some(need());
        }
        "--rpc-listen-port" => {
            let v = need();
            let rpc = cfg.rpc.get_or_insert_with(RpcConfig::default);
            rpc.port = v
                .parse::<u16>()
                .map_err(|_| format!("invalid --rpc-listen-port: {v}"))?;
        }
        "--rpc-listen-all" | "--enable-rpc-listen-all" => {
            let rpc = cfg.rpc.get_or_insert_with(RpcConfig::default);
            rpc.listen_all = true;
        }
        "--checksum" => {
            let v = need();
            let (alg, hash) = v
                .split_once('=')
                .ok_or_else(|| format!("invalid --checksum (expected TYPE=HASH): {v}"))?;
            cfg.checksum = Some((alg.to_ascii_lowercase(), hash.to_ascii_lowercase()));
        }
        other => return Err(format!("unknown option: {other}")),
    }
    Ok(())
}

/// Sentinel returned by `parse_args` for --version.
pub const VERSION_MARK: &str = "\u{0}version";
/// Sentinel returned by `parse_args` for --help.
pub const HELP_MARK: &str = "\u{0}help";

/// Hand-rolled aria2c argument parser.
///
/// Returns `Err(HELP_MARK)`/`Err(VERSION_MARK)` sentinel strings for
/// `--help`/`--version` so callers can print and exit cleanly.
pub fn parse_args(argv: &[String]) -> Result<Aria2cArgs, String> {
    let mut cfg = Aria2cArgs::default();
    let mut i = 0usize;
    let mut only_positional = false;

    while i < argv.len() {
        let arg = &argv[i];
        i += 1;

        if only_positional || arg == "-" || !arg.starts_with('-') {
            cfg.urls.push(arg.clone());
            continue;
        }
        if arg == "--" {
            only_positional = true;
            continue;
        }

        // Normalize `--name=value` into (name, inline value).
        let (name, inline) = if let Some(rest) = arg.strip_prefix("--") {
            match rest.split_once('=') {
                Some((n, val)) => (format!("--{n}"), Some(val.to_string())),
                None => (arg.clone(), None),
            }
        } else if arg.len() > 2 {
            // Compact short form: `-x4` / `-s8`. Only valid for value-taking opts.
            let (flag, val) = arg.split_at(2);
            if option_takes_value(flag) {
                apply(flag, Some(val.to_string()), &mut cfg)?;
                continue;
            }
            return Err(format!("unknown option: {arg}"));
        } else {
            (arg.clone(), None)
        };

        let value = if let Some(v) = inline {
            Some(v)
        } else if option_takes_value(&name) {
            let val = argv
                .get(i)
                .cloned()
                .ok_or_else(|| format!("option {name} requires a value"))?;
            i += 1;
            Some(val)
        } else {
            None
        };

        apply(&name, value, &mut cfg)?;
    }

    Ok(cfg)
}

/// Run aria2c-compatible downloads. Returns the process exit code.
pub async fn run(args: &Aria2cArgs) -> anyhow::Result<i32> {
    for w in &args.unsupported {
        eprintln!("aria2c: warning: {w}");
    }

    let mut args = args.clone();
    // `-i/--input-file`: append URIs read from the file.
    if let Some(input) = &args.input_file {
        let content = std::fs::read_to_string(input)
            .map_err(|e| anyhow::anyhow!("cannot read --input-file {}: {e}", input.display()))?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            args.urls.push(line.to_string());
        }
    }

    if args.show_files {
        eprintln!("aria2c: -S/--show-files needs the BT backend file preview; not wired in the CLI engine (Stage 3 subset)");
        return Ok(1);
    }

    // `--enable-rpc`: start the aria2 JSON-RPC server and stay running.
    if let Some(rpc) = &args.rpc {
        return run_rpc(rpc).await;
    }

    if args.urls.is_empty() {
        eprintln!("aria2c: no URIs provided");
        return Ok(3);
    }
    if args.dry_run {
        for url in &args.urls {
            println!("{url}");
        }
        return Ok(0);
    }

    // Options accepted by the parser but not yet wired into the engine: warn
    // rather than silently ignoring them.
    if args.resume {
        eprintln!("aria2c: warning: --continue is not yet wired in the CLI engine; ignoring");
    }
    if args.min_split_size.is_some() {
        eprintln!("aria2c: warning: --min-split-size is not yet wired in the CLI engine; ignoring");
    }

    let temp_dir = std::env::temp_dir().join("limedl-aria2c");
    let state_dir = temp_dir.join("downloads");
    std::fs::create_dir_all(&state_dir).context("create aria2c state dir")?;

    let rate_limiter = Arc::new(limedl_core::RateLimiter::default());
    if let Some(limit) = args.max_overall_download_limit.or(args.max_download_limit) {
        rate_limiter.set_rate(limit);
    }
    let event_bus = Arc::new(limedl_core::EventBus::new(8192));
    let context = Arc::new(
        limedl_core::SystemContext::with_components(
            state_dir.clone(),
            rate_limiter.clone(),
            event_bus.clone(),
        )
        .context("create SystemContext")?,
    );

    let download_manager = Arc::new(
        limedl_core::DownloadManager::new(&context).context("create DownloadManager")?,
    );

    let max_concurrent = args.max_concurrent_downloads.unwrap_or(MAX_CONCURRENT_DEFAULT).max(1);
    let sem = Arc::new(Semaphore::new(max_concurrent));

    let mut handles = Vec::new();
    for (idx, url) in args.urls.iter().enumerate() {
        let manager = download_manager.clone();
        let event_bus = event_bus.clone();
        let sem = sem.clone();
        let args = args.clone();
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|_| anyhow::anyhow!("semaphore closed"))?;
            run_one(&manager, &event_bus, &args, &url, idx).await
        }));
    }

    let mut first_err: Option<i32> = None;
    let mut ok = 0usize;
    for h in handles {
        match h.await.context("download task panicked")?? {
            Some(code) => {
                if first_err.is_none() {
                    first_err = Some(code);
                }
            }
            None => ok += 1,
        }
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
    if ok == args.urls.len() {
        Ok(0)
    } else {
        Ok(first_err.unwrap_or(1))
    }
}

/// `--enable-rpc`: start the aria2 JSON-RPC server (via the shared core engine)
/// and stay running until Ctrl-C. Returns the process exit code.
async fn run_rpc(rpc: &RpcConfig) -> anyhow::Result<i32> {
    let temp_dir = std::env::temp_dir().join("limedl-aria2c-rpc");
    let state_dir = temp_dir.join("state");
    std::fs::create_dir_all(&state_dir)?;

    let systems = limedl_core::bootstrap::bootstrap(state_dir.clone()).await?;
    let mut settings = systems.settings.clone();
    settings.aria2_rpc.enabled = true;
    settings.aria2_rpc.secret = rpc.secret.clone();
    settings.aria2_rpc.port = rpc.port;

    let (tx, rx) = tokio::sync::watch::channel(false);
    let server = limedl_core::Aria2RpcServer::new(
        systems.registry.clone(),
        &settings.aria2_rpc,
        systems.event_bus.clone(),
    );
    let addr = format!("127.0.0.1:{}", rpc.port);
    println!("aria2c: RPC server listening on {addr} (Ctrl-C to stop)");

    tokio::select! {
        res = server.serve(rx, vec![]) => { res?; }
        _ = tokio::signal::ctrl_c() => { println!("aria2c: shutting down"); }
    }
    let _ = tx.send(true);
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(0)
}

/// Download one URL to completion; returns `Some(exit_code)` on failure.
async fn run_one(
    manager: &Arc<limedl_core::DownloadManager>,
    event_bus: &Arc<limedl_core::EventBus>,
    args: &Aria2cArgs,
    url: &str,
    idx: usize,
) -> anyhow::Result<Option<i32>> {
    let mut rx = event_bus.subscribe();

    let (dest_dir, file_name) = resolve_output(args, url, idx);

    // `-Z/--conditional-get`: skip if the output file already exists.
    if args.conditional_get
        && let Some(t) = file_name.as_ref().map(|n| std::path::Path::new(&dest_dir).join(n))
        && t.exists()
    {
        println!("[#{} {url}] skipped ({}) — already exists", idx + 1, t.display());
        return Ok(None);
    }

    let (checksum_mode, expected_checksum) = map_checksum(args.checksum.as_ref())?;

    let thread_count = args
        .split
        .or(args.max_connection_per_server)
        .unwrap_or(SPLIT_DEFAULT);

    let request = limedl_core::types::StartDownloadRequest {
        url: url.to_string(),
        destination_dir: dest_dir,
        file_name,
        kind: None,
        thread_mode: Some(limedl_core::types::ThreadMode::Fixed),
        thread_count: Some(thread_count.max(1)),
        max_retries: Some(5),
        checksum: checksum_mode,
        expected_checksum,
        headers: if args.headers.is_empty() {
            None
        } else {
            Some(args.headers.clone())
        },
        start_paused: false,
        mirror_urls: None,
        user_agent: args.user_agent.clone(),
        selected_file_indices: if args.select_files.is_empty() {
            None
        } else {
            Some(args.select_files.clone())
        },
        priority: None,
    };

    let task_id = manager.start(request).await?;
    let task_id_str = task_id.to_string();

    // Wait for a terminal state for this task.
    loop {
        match rx.recv().await {
            Ok(limedl_core::DownloadEvent::Updated { id, summary_json })
                if id == task_id_str => {
                let state = summary_json.get("state").and_then(|s| s.as_str()).unwrap_or("");
                match state {
                    "completed" => {
                        if !args.quiet {
                            println!("[#{} {url}] completed", idx + 1);
                        }
                        return Ok(None);
                    }
                    "error" => {
                        eprintln!("aria2c: download failed: {url}");
                        return Ok(Some(1));
                    }
                    _ => {}
                }
            }
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
    Ok(Some(1))
}

fn resolve_output(args: &Aria2cArgs, url: &str, idx: usize) -> (String, Option<String>) {
    let file_name = match &args.out {
        Some(t) if t.contains("{}") => t.replace("{}", &default_name(url)),
        Some(t) => t.clone(),
        None => format!("{}_{}", idx + 1, default_name(url)),
    };
    let dest_dir = args
        .dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .to_string();
    (dest_dir, Some(file_name))
}

fn default_name(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or("download")
        .split('?')
        .next()
        .unwrap_or("download")
        .to_string()
}

fn map_checksum(
    checksum: Option<&(String, String)>,
) -> anyhow::Result<(Option<limedl_core::types::ChecksumMode>, Option<String>)> {
    let Some((alg, hash)) = checksum else {
        return Ok((None, None));
    };
    let mode = match alg.as_str() {
        "sha256" | "sha-256" => limedl_core::types::ChecksumMode::Sha256,
        "blake3" => limedl_core::types::ChecksumMode::Blake3,
        "sha1" | "sha-1" => limedl_core::types::ChecksumMode::Sha1,
        "xxh3" | "xxh3_128" => limedl_core::types::ChecksumMode::Xxh3128,
        other => anyhow::bail!(
            "unsupported checksum algorithm: {other} (supported: sha256, blake3, sha1, xxh3)"
        ),
    };
    Ok((Some(mode), Some(hash.clone())))
}

/// Print aria2c-style usage.
pub fn print_usage() {
    println!("Usage: limedl aria2c [options] <URL>...");
    println!("aria2c-compatible single-file HTTP download (Stage 1 subset).");
    println!("  -c, --continue                      resume download");
    println!("  -d, --dir <DIR>                     destination directory");
    println!("  -o, --out <FILE>                    output file (supports {{}} for batch)");
    println!("  -j, --max-concurrent-downloads <N>  max concurrent downloads [5]");
    println!("  -x, --max-connection-per-server <N> max connections per server [1]");
    println!("  -s, --split <N>                     connections per file [5]");
    println!("  -k, --min-split-size <SIZE>         minimum split size");
    println!("      --max-download-limit <SIZE>     per-download rate limit");
    println!("      --max-overall-download-limit <SIZE> overall rate limit");
    println!("      --user-agent <UA>               user agent");
    println!("  -H, --header <HEADER>               append HTTP header");
    println!("      --checksum <TYPE=HASH>          verify checksum (sha256/blake3/sha1/xxh3)");
    println!("      --dry-run                       print URIs only");
    println!("  -q, --quiet                         suppress progress");
    println!("  -i, --input-file <FILE>             read URIs from a file");
    println!("  -Z, --conditional-get               skip if output file exists");
    println!("      --select-file <N>               BT: select 1-based file index (repeatable)");
    println!("  -S, --show-files                    list torrent files (not wired)");
    println!("      --enable-rpc                    start aria2 JSON-RPC server");
    println!("      --rpc-secret <SECRET>           RPC auth secret");
    println!("      --rpc-listen-port <PORT>        RPC listen port (default 6800)");
    println!("      --version                       print version");
    println!("  -h, --help                          show this help");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_urls_and_basic_flags() {
        let a = parse_args(&v(&["-c", "-o", "x.bin", "https://a/b"])).unwrap();
        assert!(a.resume);
        assert_eq!(a.out.as_deref(), Some("x.bin"));
        assert_eq!(a.urls, vec!["https://a/b".to_string()]);
    }

    #[test]
    fn parses_equals_and_size_suffixes() {
        let a = parse_args(&v(&[
            "--split=8",
            "--max-download-limit=1M",
            "--checksum=sha-256=abc",
            "u",
        ]))
        .unwrap();
        assert_eq!(a.split, Some(8));
        assert_eq!(a.max_download_limit, Some(1024 * 1024));
        assert_eq!(a.checksum, Some(("sha-256".to_string(), "abc".to_string())));
    }

    #[test]
    fn accepts_multiple_urls_and_dir() {
        let a = parse_args(&v(&["-d", "/tmp", "u1", "u2"])).unwrap();
        assert_eq!(a.dir, Some(PathBuf::from("/tmp")));
        assert_eq!(a.urls, vec!["u1".to_string(), "u2".to_string()]);
    }

    #[test]
    fn parses_compact_short() {
        let a = parse_args(&v(&["-s4", "u"])).unwrap();
        assert_eq!(a.split, Some(4));
    }

    #[test]
    fn rejects_unknown_and_missing_value() {
        assert!(parse_args(&v(&["--nope", "u"])).is_err());
        assert!(parse_args(&v(&["-o"])).is_err());
    }

    #[test]
    fn help_and_version_markers() {
        assert!(parse_args(&v(&["--help"])).is_err());
        assert!(parse_args(&v(&["--version"])).is_err());
    }

    #[test]
    fn parses_input_file_and_conditional_get() {
        let a = parse_args(&v(&["-i", "list.txt", "-Z", "u"])).unwrap();
        assert_eq!(a.input_file, Some(PathBuf::from("list.txt")));
        assert!(a.conditional_get);
    }

    #[test]
    fn parses_select_file_and_rpc() {
        let a = parse_args(&v(&[
            "--select-file=2",
            "--select-file", "5",
            "--enable-rpc",
            "--rpc-secret=tok",
            "--rpc-listen-port=6900",
            "u",
        ]))
        .unwrap();
        assert_eq!(a.select_files, vec![2, 5]);
        let rpc = a.rpc.expect("rpc config");
        assert_eq!(rpc.secret.as_deref(), Some("tok"));
        assert_eq!(rpc.port, 6900);
    }
}
