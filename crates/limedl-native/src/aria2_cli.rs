//! Parser for a pasted `aria2c` command line.
//!
//! Users frequently copy a whole aria2 command out of a browser extension or a
//! forum post (URL plus `--out` / `--header` / `--checksum`) and paste it into
//! the new-task box. This module turns such a line into the subset of
//! [`limedl_core::types::StartDownloadRequest`] that limedl can honor, and
//! records the options it had to ignore so the dialog can say so instead of
//! silently dropping a cookie jar or an input file.
//!
//! Only a single command is parsed here; batch handling (one command per line)
//! lives in [`crate::url_utils`].

use limedl_core::types::ChecksumMode;

/// Options that materially change what is downloaded but that the engine cannot
/// honor. The mapped value is the label reported to the user, so an alias
/// (`-i`) does not show up as a different option than its long form.
const UNSUPPORTED_VALUE_OPTIONS: &[(&str, &str)] = &[
    ("--input-file", "--input-file"),
    ("-i", "--input-file"),
    ("--load-cookies", "--load-cookies"),
    ("--save-cookies", "--save-cookies"),
    ("--metalink-file", "--metalink-file"),
    ("-M", "--metalink-file"),
    ("--torrent-file", "--torrent-file"),
    ("-T", "--torrent-file"),
    ("--http-user", "--http-user"),
    ("--http-passwd", "--http-passwd"),
    ("--ftp-user", "--ftp-user"),
    ("--ftp-passwd", "--ftp-passwd"),
    ("--no-proxy", "--no-proxy"),
];

/// Flag options (no value) that cannot be honored.
const UNSUPPORTED_FLAG_OPTIONS: &[&str] = &["--no-check-certificate"];

/// Options whose value is a URL. They are ignored, but the value must still be
/// consumed so a proxy URI is never mistaken for a download target.
const PROXY_VALUE_OPTIONS: &[&str] = &[
    "--all-proxy",
    "--http-proxy",
    "--https-proxy",
    "--ftp-proxy",
    "--proxy",
    "--all-proxy-user",
    "--all-proxy-passwd",
    "--http-proxy-user",
    "--http-proxy-passwd",
    "--https-proxy-user",
    "--https-proxy-passwd",
    "--ftp-proxy-user",
    "--ftp-proxy-passwd",
];

/// The subset of an aria2 command limedl understands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Aria2Command {
    /// Positional download targets, in the order they appeared.
    pub urls: Vec<String>,
    /// `--out` / `-o`
    pub out: Option<String>,
    /// `--dir` / `-d`
    pub dir: Option<String>,
    /// `--header` values plus a synthesized `Referer:` for `--referer`.
    pub headers: Vec<String>,
    /// `--user-agent` / `-U`
    pub user_agent: Option<String>,
    /// `--checksum=TYPE=DIGEST`, normalized to a lowercase digest.
    pub checksum: Option<(ChecksumMode, String)>,
    /// `-Z` / `--force-sequential`
    pub force_sequential: bool,
    /// Options that were recognized but cannot be honored, deduplicated.
    pub unsupported: Vec<String>,
}

/// Cheap check used to decide whether an input is worth a full parse.
pub fn looks_like_aria2_command(text: &str) -> bool {
    let joined = join_continuations(text);
    tokenize(&joined)
        .first()
        .is_some_and(|token| is_aria2c_program(token))
}

/// Parse `text` as an aria2 command line.
///
/// Returns `None` when the text is not an `aria2c` invocation or names no
/// download target — a bare `aria2c --help` must not hijack the URL field.
pub fn parse(text: &str) -> Option<Aria2Command> {
    let joined = join_continuations(text);
    let tokens = tokenize(&joined);
    if !tokens.first().is_some_and(|token| is_aria2c_program(token)) {
        return None;
    }

    let mut cmd = Aria2Command::default();
    let mut i = 1;
    while i < tokens.len() {
        let token = tokens[i].clone();
        if token.starts_with('-') && token != "-" {
            let (name, inline) = split_option(&token);

            if let Some(label) = unsupported_value_label(&name) {
                cmd.unsupported.push(label.to_string());
                take_value(&tokens, &mut i, inline.as_deref());
            } else if UNSUPPORTED_FLAG_OPTIONS.contains(&name.as_str()) {
                cmd.unsupported.push(name);
            } else if PROXY_VALUE_OPTIONS.contains(&name.as_str()) {
                take_value(&tokens, &mut i, inline.as_deref());
            } else {
                match name.as_str() {
                    "--out" | "-o" => {
                        if let Some(value) = take_value(&tokens, &mut i, inline.as_deref()) {
                            cmd.out = Some(value);
                        }
                    }
                    "--dir" | "-d" => {
                        if let Some(value) = take_value(&tokens, &mut i, inline.as_deref()) {
                            cmd.dir = Some(value);
                        }
                    }
                    "--header" | "-H" | "--http-header" => {
                        if let Some(value) = take_value(&tokens, &mut i, inline.as_deref())
                            && !value.trim().is_empty()
                        {
                            cmd.headers.push(value);
                        }
                    }
                    "--user-agent" | "-U" => {
                        if let Some(value) = take_value(&tokens, &mut i, inline.as_deref())
                            && !value.trim().is_empty()
                        {
                            cmd.user_agent = Some(value);
                        }
                    }
                    "--referer" | "--referrer" | "--http-referer" => {
                        if let Some(value) = take_value(&tokens, &mut i, inline.as_deref())
                            && !value.trim().is_empty()
                        {
                            cmd.headers.push(format!("Referer: {value}"));
                        }
                    }
                    "--checksum" => {
                        if let Some(value) = take_value(&tokens, &mut i, inline.as_deref()) {
                            match parse_checksum_spec(&value) {
                                Ok(Some(pair)) => cmd.checksum = Some(pair),
                                Ok(None) => {}
                                // Recognized but not a digest limedl verifies.
                                Err(()) => cmd.unsupported.push("--checksum".to_string()),
                            }
                        }
                    }
                    "--force-sequential" | "-Z" => cmd.force_sequential = true,
                    // Any other option: consume the next token as its value
                    // unless it is another option or a URL (the latter covers
                    // boolean flags such as `--continue` written without a
                    // value directly before the target).
                    _ => {
                        if let Some(next) = tokens.get(i + 1)
                            && !next.starts_with('-')
                            && next != "-"
                            && !looks_like_url(next)
                        {
                            i += 1;
                        }
                    }
                }
            }
        } else if looks_like_url(&token) {
            cmd.urls.push(token);
        }
        i += 1;
    }

    cmd.unsupported.dedup();
    if cmd.urls.is_empty() {
        return None;
    }
    Some(cmd)
}

fn unsupported_value_label(name: &str) -> Option<&'static str> {
    UNSUPPORTED_VALUE_OPTIONS
        .iter()
        .find(|(option, _)| *option == name)
        .map(|(_, label)| *label)
}

/// Join `^` / `\` line continuations into a single logical line.
fn join_continuations(text: &str) -> String {
    let mut out = String::new();
    let mut pending = false;
    for raw in text.lines() {
        let line = raw.trim_end();
        let (line, continues) = match line.strip_suffix('^').or_else(|| line.strip_suffix('\\')) {
            Some(stripped) => (stripped, true),
            None => (line, false),
        };
        if pending {
            out.push(' ');
        }
        out.push_str(line);
        pending = continues;
    }
    out
}

/// Split a command line into tokens, honoring single and double quotes.
///
/// Backslashes stay literal outside quotes (Windows paths) but escape `"` and
/// `\` inside double quotes.
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut started = false;

    while let Some(c) = chars.next() {
        if in_single {
            started = true;
            if c == '\'' {
                in_single = false;
            } else {
                current.push(c);
            }
        } else if in_double {
            started = true;
            match c {
                '"' => in_double = false,
                '\\' => match chars.peek() {
                    Some('"') | Some('\\') => current.push(chars.next().unwrap_or('\\')),
                    _ => current.push('\\'),
                },
                _ => current.push(c),
            }
        } else {
            match c {
                '"' => {
                    in_double = true;
                    started = true;
                }
                '\'' => {
                    in_single = true;
                    started = true;
                }
                c if c.is_whitespace() => {
                    if started {
                        tokens.push(std::mem::take(&mut current));
                        started = false;
                    }
                }
                _ => {
                    current.push(c);
                    started = true;
                }
            }
        }
    }
    if started {
        tokens.push(current);
    }
    tokens
}

fn is_aria2c_program(token: &str) -> bool {
    let base = token.rsplit(['/', '\\']).next().unwrap_or(token);
    let base = base.trim_matches('"').to_ascii_lowercase();
    base == "aria2c" || base == "aria2c.exe"
}

/// Split `--name=value` / `-oVALUE` into a name and its attached value.
fn split_option(token: &str) -> (String, Option<String>) {
    if let Some(rest) = token.strip_prefix("--") {
        if let Some((name, value)) = rest.split_once('=') {
            return (format!("--{name}"), Some(value.to_string()));
        }
        return (token.to_string(), None);
    }
    if token.starts_with('-') && token.len() > 2 {
        let (flag, rest) = token.split_at(2);
        if matches!(flag, "-o" | "-d" | "-U" | "-H") {
            return (flag.to_string(), Some(rest.to_string()));
        }
    }
    (token.to_string(), None)
}

/// Read an option value from the inline `=VALUE` form or the next token.
fn take_value(tokens: &[String], i: &mut usize, inline: Option<&str>) -> Option<String> {
    if let Some(value) = inline {
        return Some(value.to_string());
    }
    let next = tokens.get(*i + 1)?;
    *i += 1;
    Some(next.clone())
}

fn looks_like_url(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("ftp://")
        || lower.starts_with("sftp://")
        || lower.starts_with("magnet:?")
}

/// Parse `TYPE=DIGEST`. `Err(())` marks a recognized algorithm limedl cannot
/// verify; `Ok(None)` marks a malformed value that is dropped silently.
fn parse_checksum_spec(spec: &str) -> Result<Option<(ChecksumMode, String)>, ()> {
    let Some((algo, digest)) = spec.split_once('=') else {
        return Ok(None);
    };
    let algo = algo.trim().to_ascii_lowercase();
    let digest = digest.trim().to_ascii_lowercase();
    if digest.is_empty() || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(None);
    }
    let mode = match algo.as_str() {
        "sha-256" | "sha256" => ChecksumMode::Sha256,
        "sha-512" | "sha512" => ChecksumMode::Sha512,
        "blake3" | "b3" => ChecksumMode::Blake3,
        _ => return Err(()),
    };
    Ok(Some((mode, digest)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_only_aria2c_invocations() {
        assert!(looks_like_aria2_command("aria2c https://example.com/a.zip"));
        assert!(looks_like_aria2_command(
            "\"C:\\Program Files\\aria2\\aria2c.exe\" https://example.com/a.zip"
        ));
        assert!(looks_like_aria2_command(
            "^C:\\tools\\aria2c.exe https://example.com/a.zip"
        ));
        assert!(!looks_like_aria2_command("https://example.com/a.zip"));
        assert!(!looks_like_aria2_command(""));
        // A bare invocation names no target, so it must not be applied even
        // though the program token matches.
        assert!(parse("aria2c --help").is_none());
        assert!(parse("not a command").is_none());
    }

    #[test]
    fn parses_the_reported_quark_example() {
        let command = "aria2c \"https://dl-pc-zb.drive.quark.cn/550g/file.ass?token=abc\" \
                       --out \"[YGM] Tensei Shitara Slime Datta Ken [01][Ma10p_2160p][x265_flac].ass\" \
                       --header \"Cookie: b-user-id=abc; ctoken=xyz\"";
        let parsed = parse(command).expect("aria2c command");

        assert_eq!(
            parsed.urls,
            vec!["https://dl-pc-zb.drive.quark.cn/550g/file.ass?token=abc"]
        );
        assert_eq!(
            parsed.out.as_deref(),
            Some("[YGM] Tensei Shitara Slime Datta Ken [01][Ma10p_2160p][x265_flac].ass")
        );
        assert_eq!(
            parsed.headers,
            vec!["Cookie: b-user-id=abc; ctoken=xyz".to_string()]
        );
        assert!(parsed.unsupported.is_empty());
    }

    #[test]
    fn collects_every_option_limedl_can_honor() {
        let parsed = parse(
            "aria2c -d /downloads -o file.bin --header=\"Accept: */*\" -U limedl/1.0 \
             --referer https://pan.example/page --checksum=sha-256=ABCDEF https://a.example/f.bin",
        )
        .expect("aria2c command");

        assert_eq!(parsed.dir.as_deref(), Some("/downloads"));
        assert_eq!(parsed.out.as_deref(), Some("file.bin"));
        assert_eq!(
            parsed.headers,
            vec![
                "Accept: */*".to_string(),
                "Referer: https://pan.example/page".to_string(),
            ]
        );
        assert_eq!(parsed.user_agent.as_deref(), Some("limedl/1.0"));
        assert_eq!(
            parsed.checksum,
            Some((ChecksumMode::Sha256, "abcdef".to_string()))
        );
    }

    #[test]
    fn multiple_urls_and_sequential_are_reported() {
        let parsed = parse("aria2c -Z https://a.example/1.bin https://a.example/2.bin")
            .expect("aria2c command");
        assert_eq!(parsed.urls.len(), 2);
        assert!(parsed.force_sequential);
    }

    #[test]
    fn proxy_values_are_never_mistaken_for_download_urls() {
        let parsed = parse(
            "aria2c --all-proxy http://proxy.local:8080 https://a.example/f.bin",
        )
        .expect("aria2c command");
        assert_eq!(parsed.urls, vec!["https://a.example/f.bin"]);
    }

    #[test]
    fn unknown_boolean_flags_do_not_eat_the_url() {
        let parsed = parse("aria2c --continue https://a.example/f.bin")
            .expect("aria2c command");
        assert_eq!(parsed.urls, vec!["https://a.example/f.bin"]);
        assert!(parsed.unsupported.is_empty());
    }

    #[test]
    fn ignores_performance_values_silently() {
        let parsed = parse("aria2c -x 16 -s 16 -k 1M https://a.example/f.bin")
            .expect("aria2c command");
        assert_eq!(parsed.urls, vec!["https://a.example/f.bin"]);
        assert!(parsed.unsupported.is_empty());
    }

    #[test]
    fn reports_options_it_cannot_honor() {
        let parsed = parse(
            "aria2c --load-cookies cookies.txt --no-check-certificate --checksum=sha-1=abcdef \
             https://a.example/f.bin",
        )
        .expect("aria2c command");

        assert_eq!(
            parsed.unsupported,
            vec![
                "--load-cookies".to_string(),
                "--no-check-certificate".to_string(),
                "--checksum".to_string(),
            ],
            "an alias is reported once and unsupported digests are surfaced"
        );
    }

    #[test]
    fn quoted_whitespace_inside_headers_survives() {
        let parsed = parse("aria2c --header 'Cookie: a=1; b=2' https://a.example/f.bin")
            .expect("aria2c command");
        assert_eq!(parsed.headers, vec!["Cookie: a=1; b=2".to_string()]);
    }

    #[test]
    fn line_continuations_are_joined() {
        let parsed = parse("aria2c --out file.bin ^\n  https://a.example/f.bin").expect("command");
        assert_eq!(parsed.out.as_deref(), Some("file.bin"));
        assert_eq!(parsed.urls, vec!["https://a.example/f.bin"]);
    }
}
