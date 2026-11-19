use std::io::Write;

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
        expected_sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            .to_string(),
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
        parse_content_range_total(
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-0/227880072\r\n"
        ),
        Some(227_880_072)
    );
    assert_eq!(
        parse_content_range_total("Content-Range: bytes 0-0/5"),
        Some(5)
    );
    assert_eq!(parse_content_range_total("Content-Length: 5"), None);
    assert_eq!(
        parse_content_length("HTTP/1.1 200 OK\nContent-Length: 12\n\n"),
        Some(12)
    );
    assert_eq!(
        parse_status("HTTP/1.1 302 Found\r\n\r\nHTTP/2 206\r\n"),
        Some(206)
    );
    assert!(features_include_http2(
        "Features: alt-svc AsynchDNS HSTS HTTP2\n"
    ));
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
    run_with(
        &dir,
        &opts,
        Some("does-not-exist.ttf"),
        &spec,
        &Policy::default(),
    )
    .unwrap();
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
    assert_eq!(
        fs::read(dir.join(FONT_DIR).join(FONT_NAME)).unwrap(),
        payload
    );
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
                let Ok(stream) = stream else { break };
                handle_test_connection(stream, &zip);
            }
        });
        Self {
            url: format!("http://{addr}/MiSans.zip"),
        }
    }
}

fn handle_test_connection(mut stream: std::net::TcpStream, zip: &[u8]) {
    let Ok(clone) = stream.try_clone() else {
        return;
    };
    let mut reader = std::io::BufReader::new(clone);
    let mut request_line = String::new();
    if std::io::BufRead::read_line(&mut reader, &mut request_line).unwrap_or(0) == 0 {
        return;
    }
    let range = read_test_range_header(&mut reader);
    let method = request_line.split_whitespace().next().unwrap_or("");
    let total = zip.len() as u64;
    let (status, start, end) = match (method, range) {
        ("HEAD", _) => ("200 OK", 0, total - 1),
        (_, Some((s, e))) => ("206 Partial Content", s.min(total - 1), e.min(total - 1)),
        _ => ("200 OK", 0, total - 1),
    };
    let body: &[u8] = if method == "HEAD" {
        &[]
    } else {
        &zip[start as usize..=end as usize]
    };
    let mut response = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\n", body.len());
    if status.starts_with("206") {
        response.push_str(&format!("Content-Range: bytes {start}-{end}/{total}\r\n"));
    }
    response.push_str("Accept-Ranges: bytes\r\nConnection: close\r\n\r\n");
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn read_test_range_header(
    reader: &mut std::io::BufReader<std::net::TcpStream>,
) -> Option<(u64, u64)> {
    let mut range = None;
    loop {
        let mut line = String::new();
        if std::io::BufRead::read_line(reader, &mut line).unwrap_or(0) == 0
            || line == "\r\n"
            || line == "\n"
        {
            break;
        }
        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("range:") {
            range = parse_test_range(rest);
        }
    }
    range
}

fn parse_test_range(rest: &str) -> Option<(u64, u64)> {
    let rest = rest.trim();
    let rest = rest.strip_prefix("bytes=")?;
    let (start, end) = rest.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?))
}
