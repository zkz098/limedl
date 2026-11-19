use super::*;

const SAMPLE_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const SAMPLE_HASH_UPPER: &str = "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855";
const OTHER_HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

#[test]
fn test_is_valid_sha256_hex() {
    assert!(is_valid_sha256_hex(SAMPLE_HASH));
    assert!(is_valid_sha256_hex(SAMPLE_HASH_UPPER));
    assert!(!is_valid_sha256_hex("short"));
    assert!(!is_valid_sha256_hex(&format!("{SAMPLE_HASH}extra")));
    assert!(!is_valid_sha256_hex("g".repeat(64).as_str()));
}

#[test]
fn test_parse_single_raw_hash() {
    assert_eq!(
        parse_sha256_file(SAMPLE_HASH, Some("file.iso")),
        Some(SAMPLE_HASH.to_string())
    );
    assert_eq!(
        parse_sha256_file(&format!("  {SAMPLE_HASH_UPPER}\n"), Some("file.iso")),
        Some(SAMPLE_HASH.to_string())
    );
}

/// Byte 64 can fall inside a multi-byte character. Slicing there used to
/// panic (found by the `sha256_manifest` fuzz target); the line must simply
/// be skipped.
#[test]
fn test_multibyte_character_at_hash_boundary_does_not_panic() {
    let line = format!("{}{}in", "a".repeat(63), 'ѝ');
    assert_eq!(parse_sha256_file(&line, None), None);
    assert_eq!(parse_sha256_file(&line, Some("x")), None);
    assert_eq!(parse_sha256sums(&line, "x"), None);

    // Exact fuzz reproducer: the multi-byte character straddles the
    // 64-byte prefix of the second line.
    let repro = "# geror\n\
                 e3b:c44298fc1c149afbf4c899-fb92427ae41e463856*1 b9957852*test-rѝin\n\
                 t-rѝin\n1e";
    assert_eq!(parse_sha256_file(repro, None), None);
    assert_eq!(parse_sha256_file(repro, Some("test-file.bin")), None);
    assert_eq!(parse_sha256sums(repro, "test-file.bin"), None);
}

#[test]
fn test_parse_gnu_sha256sum() {
    let content = format!("{SAMPLE_HASH}  ubuntu-24.04.iso\n{OTHER_HASH}  debian.iso\n");
    assert_eq!(
        parse_sha256_file(&content, Some("ubuntu-24.04.iso")),
        Some(SAMPLE_HASH.to_string())
    );
    assert_eq!(
        parse_sha256_file(&content, Some("debian.iso")),
        Some(OTHER_HASH.to_string())
    );
    assert_eq!(parse_sha256_file(&content, Some("nonexistent.iso")), None);
}

#[test]
fn test_parse_gnu_binary_mode() {
    let content = format!("{SAMPLE_HASH} *ubuntu-24.04.iso\n");
    assert_eq!(
        parse_sha256_file(&content, Some("ubuntu-24.04.iso")),
        Some(SAMPLE_HASH.to_string())
    );
}

#[test]
fn test_parse_bsd_format() {
    let content = format!("SHA256 (ubuntu-24.04.iso) = {SAMPLE_HASH}\n");
    assert_eq!(
        parse_sha256_file(&content, Some("ubuntu-24.04.iso")),
        Some(SAMPLE_HASH.to_string())
    );

    let content2 = format!("SHA256(./dist/app.zip) = {OTHER_HASH}\n");
    assert_eq!(
        parse_sha256_file(&content2, Some("app.zip")),
        Some(OTHER_HASH.to_string())
    );
}

#[test]
fn test_parse_pgp_signed_sha256sums() {
    let content = format!(
        "-----BEGIN PGP SIGNED MESSAGE-----\n\
         Hash: SHA512\n\n\
         {SAMPLE_HASH}  ubuntu-24.04.iso\n\
         {OTHER_HASH}  ubuntu-24.04-server.iso\n\
         -----BEGIN PGP SIGNATURE-----\n\
         Version: GnuPG v1\n\
         ...\n\
         -----END PGP SIGNATURE-----\n"
    );
    assert_eq!(
        parse_sha256sums(&content, "ubuntu-24.04.iso"),
        Some(SAMPLE_HASH.to_string())
    );
    assert_eq!(
        parse_sha256sums(&content, "ubuntu-24.04-server.iso"),
        Some(OTHER_HASH.to_string())
    );
    assert_eq!(parse_sha256sums(&content, "unknown.iso"), None);
}

#[test]
fn test_generate_candidate_urls() {
    let url = "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso";
    let candidates = generate_candidate_urls(url);
    let urls: Vec<_> = candidates.into_iter().map(|c| c.url).collect();

    assert!(urls.contains(
        &"https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso.sha256".to_string()
    ));
    assert!(urls.contains(
        &"https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso.sha256sum".to_string()
    ));
    assert!(urls.contains(&"https://releases.ubuntu.com/24.04/SHA256SUMS".to_string()));
    assert!(urls.contains(&"https://releases.ubuntu.com/24.04/sha256sums.txt".to_string()));
}

#[test]
fn test_generate_candidate_urls_with_query() {
    let url = "https://example.com/downloads/package.tar.gz?token=secret123";
    let candidates = generate_candidate_urls(url);
    let urls: Vec<_> = candidates.into_iter().map(|c| c.url).collect();

    assert!(urls.contains(
        &"https://example.com/downloads/package.tar.gz.sha256?token=secret123".to_string()
    ));
    assert!(urls.contains(&"https://example.com/downloads/package.tar.gz.sha256".to_string()));
    assert!(urls.contains(&"https://example.com/downloads/SHA256SUMS".to_string()));
}

#[test]
fn test_extract_basename_strips_directory_prefixes() {
    assert_eq!(extract_basename("./dist/app.zip"), "app.zip");
    assert_eq!(extract_basename(".\\dist\\app.zip"), "app.zip");
    assert_eq!(extract_basename("  dir/file.iso  "), "file.iso");
    assert_eq!(extract_basename("bare.bin"), "bare.bin");
}

/// The parser must skip noise lines and keep scanning: a wrong `expected_checksum`
/// is worse than none — it turns a healthy download into a corruption report.
#[test]
fn test_parse_sha256_file_skips_noise_and_uses_later_match() {
    let content = format!(
        "# generated by sha256sum\n\
         -----BEGIN PGP SIGNED MESSAGE-----\n\
         Hash: SHA256\n\
         \n\
         {OTHER_HASH}  other-file.iso\n\
         {SAMPLE_HASH}  ubuntu-24.04.iso\n"
    );
    assert_eq!(
        parse_sha256_file(&content, Some("ubuntu-24.04.iso")),
        Some(SAMPLE_HASH.to_string())
    );
    assert_eq!(
        parse_sha256_file(&content, Some("other-file.iso")),
        Some(OTHER_HASH.to_string())
    );
    assert_eq!(parse_sha256_file(&content, Some("missing.iso")), None);

    // A standalone 64-hex line anywhere after a comment is still a bare hash.
    assert_eq!(
        parse_sha256_file(&format!("# comment\n{SAMPLE_HASH}\n"), Some("anything.iso")),
        Some(SAMPLE_HASH.to_string())
    );
}

/// Some producers glue the name straight to the digest without the GNU
/// separator; the fallback scan exists for exactly that shape.
#[test]
fn test_parse_sha256_file_fallback_accepts_hash_glued_to_name() {
    let content = format!("{OTHER_HASH}other.iso\n{SAMPLE_HASH}target.iso\n");
    assert_eq!(
        parse_sha256_file(&content, Some("target.iso")),
        Some(SAMPLE_HASH.to_string())
    );
    assert_eq!(
        parse_sha256_file(&content, None),
        Some(OTHER_HASH.to_string())
    );
}

#[test]
fn test_parse_bsd_line_rejects_malformed_entries() {
    // No parentheses at all.
    assert_eq!(
        parse_sha256_file("SHA256 ubuntu.iso = abcd", Some("ubuntu.iso")),
        None
    );
    // Opening parenthesis without a closing one.
    assert_eq!(
        parse_sha256_file("SHA256 (ubuntu.iso = abcd", Some("ubuntu.iso")),
        None
    );
    // Closing parenthesis before the opening one.
    assert_eq!(
        parse_sha256_file("SHA256 ) ubuntu.iso ( = abcd", Some("ubuntu.iso")),
        None
    );
    // Well-formed shape, but the digest is not a SHA-256 hex string.
    assert_eq!(
        parse_sha256_file("SHA256 (ubuntu.iso) = not-a-hash", Some("ubuntu.iso")),
        None
    );
    // Different algorithm.
    assert_eq!(
        parse_sha256_file(
            &format!("MD5 (ubuntu.iso) = {SAMPLE_HASH}"),
            Some("ubuntu.iso")
        ),
        None
    );
    // Hyphenated spellings work, and a name mismatch must not match.
    let hyphenated = format!("SHA-256 (./dist/ubuntu.iso) = {SAMPLE_HASH}");
    assert_eq!(
        parse_sha256_file(&hyphenated, Some("ubuntu.iso")),
        Some(SAMPLE_HASH.to_string())
    );
    assert_eq!(parse_sha256_file(&hyphenated, Some("debian.iso")), None);
}

#[test]
fn test_parse_gnu_line_edge_cases() {
    // Shorter than a digest, and a 64-byte non-hex prefix: both rejected.
    assert_eq!(parse_sha256_file("short line", Some("file.iso")), None);
    assert_eq!(
        parse_sha256_file(&format!("{}  file.iso", "g".repeat(64)), Some("file.iso")),
        None
    );
    // Tab separator (GNU allows it) and a mismatching name.
    assert_eq!(
        parse_sha256_file(&format!("{SAMPLE_HASH}\tfile.iso"), Some("file.iso")),
        Some(SAMPLE_HASH.to_string())
    );
    assert_eq!(
        parse_sha256_file(&format!("{SAMPLE_HASH}  file.iso"), Some("other.iso")),
        None
    );
}

#[test]
fn test_parse_sha256sums_bsd_entries() {
    let content = format!("SHA256 (ubuntu.iso) = {SAMPLE_HASH}\n");
    assert_eq!(
        parse_sha256sums(&content, "ubuntu.iso"),
        Some(SAMPLE_HASH.to_string())
    );
    assert_eq!(parse_sha256sums(&content, "debian.iso"), None);
}

#[test]
fn test_generate_candidate_urls_skips_urls_without_a_file_name() {
    // Unparsable URL.
    assert!(generate_candidate_urls("not a url").is_empty());
    // Root path has no file name to derive `<file>.sha256` from.
    assert!(generate_candidate_urls("https://example.com/").is_empty());
    assert!(generate_candidate_urls("https://example.com").is_empty());
}

#[test]
fn test_build_headers_ignores_malformed_extra_headers() {
    let headers = build_headers(
        "limedl-test/1.0",
        &[
            "X-Valid: yes".to_string(),
            "MissingColon".to_string(),
            "Bad Name: value".to_string(),
            "X-Bad-Value: line\nbreak".to_string(),
        ],
    );
    assert_eq!(headers.len(), 2, "only User-Agent and X-Valid may survive");
    assert_eq!(headers.get("x-valid").unwrap(), "yes");
    assert!(headers.get("missingcolon").is_none());
    assert!(headers.get("bad name").is_none());
    assert!(headers.get("x-bad-value").is_none());
}

/// A server answering every checksum candidate with an HTML "not found"
/// page (HTTP 200) must not produce an expected checksum — that would turn
/// every download of this URL into a false corruption failure.
#[tokio::test]
async fn detect_sha256_ignores_html_pages() {
    use axum::http::StatusCode;
    use axum::routing::get;

    let app = axum::Router::new()
        .route(
            "/file.sha256",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    "<!DOCTYPE html><html><body>404 Not Found</body></html>",
                )
            }),
        )
        .fallback(|| async { StatusCode::NOT_FOUND });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve::serve(listener, app).await;
    });

    let client = Client::new();
    let detected = detect_sha256(
        &client,
        &format!("{addr}/file"),
        "file",
        "limedl-test/1.0",
        &[],
    )
    .await;
    assert_eq!(detected, None, "an HTML page is not a checksum");
}

/// The directory manifest is the last candidate the probe tries, and its
/// GNU-format entry has to be matched against the downloaded file name.
#[tokio::test]
async fn detect_sha256_reads_a_directory_manifest() {
    use axum::http::StatusCode;
    use axum::routing::get;

    let body = format!("{SAMPLE_HASH}  file\n{OTHER_HASH}  other-file\n");
    let app = axum::Router::new()
        .route(
            "/SHA256SUMS",
            get(move || {
                let body = body.clone();
                async move { body }
            }),
        )
        .fallback(|| async { StatusCode::NOT_FOUND });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let _ = axum::serve::serve(listener, app).await;
    });

    let client = Client::new();
    let detected = detect_sha256(
        &client,
        &format!("{addr}/file"),
        "file",
        "limedl-test/1.0",
        &[],
    )
    .await;
    assert_eq!(detected.as_deref(), Some(SAMPLE_HASH));
}

#[test]
fn test_detect_checksum_from_headers_rfc3230_digest() {
    use http::header::HeaderValue;

    // Base64 of ABC_SHA256: ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
    // echo -n "abc" | sha256sum | xxd -r -p | base64 => unhWv48Bz+pBQUDeXa4iI7ADY6OWF3qctBD/YfIAFa0=
    let b64 = "unhWv48Bz+pBQUDeXa4iI7ADY6OWF3qctBD/YfIAFa0=";
    let mut headers = HeaderMap::new();
    headers.insert(
        "Digest",
        HeaderValue::from_str(&format!("SHA-256={b64}")).unwrap(),
    );

    let res = detect_checksum_from_headers(&headers);
    assert_eq!(
        res,
        Some((
            ChecksumMode::Sha256,
            "ba7856bf8f01cfea414140de5dae2223b00363a396177a9cb410ff61f20015ad".to_string()
        ))
    );
}

#[test]
fn test_detect_checksum_from_headers_repr_digest_rfc9530() {
    use http::header::HeaderValue;

    let b64 = "unhWv48Bz+pBQUDeXa4iI7ADY6OWF3qctBD/YfIAFa0=";
    let mut headers = HeaderMap::new();
    headers.insert(
        "Repr-Digest",
        HeaderValue::from_str(&format!("sha-256=:{b64}:")).unwrap(),
    );

    let res = detect_checksum_from_headers(&headers);
    assert_eq!(
        res,
        Some((
            ChecksumMode::Sha256,
            "ba7856bf8f01cfea414140de5dae2223b00363a396177a9cb410ff61f20015ad".to_string()
        ))
    );
}

#[test]
fn test_detect_checksum_from_headers_s3_and_gcs() {
    use http::header::HeaderValue;

    let b64 = "unhWv48Bz+pBQUDeXa4iI7ADY6OWF3qctBD/YfIAFa0=";
    let mut headers = HeaderMap::new();
    headers.insert("x-amz-checksum-sha256", HeaderValue::from_str(b64).unwrap());

    let res = detect_checksum_from_headers(&headers);
    assert_eq!(
        res,
        Some((
            ChecksumMode::Sha256,
            "ba7856bf8f01cfea414140de5dae2223b00363a396177a9cb410ff61f20015ad".to_string()
        ))
    );

    let mut gcs_headers = HeaderMap::new();
    gcs_headers.insert(
        "x-goog-hash",
        HeaderValue::from_str(&format!("crc32c=xyz, sha256={b64}")).unwrap(),
    );
    let gcs_res = detect_checksum_from_headers(&gcs_headers);
    assert_eq!(
        gcs_res,
        Some((
            ChecksumMode::Sha256,
            "ba7856bf8f01cfea414140de5dae2223b00363a396177a9cb410ff61f20015ad".to_string()
        ))
    );
}

#[test]
fn test_detect_checksum_from_headers_hex_headers() {
    use http::header::HeaderValue;

    let mut headers = HeaderMap::new();
    headers.insert(
        "Digest",
        HeaderValue::from_str(
            "BLAKE3=\"6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85\"",
        )
        .unwrap(),
    );

    let res = detect_checksum_from_headers(&headers);
    assert_eq!(
        res,
        Some((
            ChecksumMode::Blake3,
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85".to_string()
        ))
    );
}
