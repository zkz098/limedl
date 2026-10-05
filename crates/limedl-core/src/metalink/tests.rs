use reqwest::header::{HeaderMap, HeaderValue};

use super::*;
use crate::types::ChecksumMode;

#[test]
fn test_parse_metalink4_rfc5854() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<metalink xmlns="urn:ietf:params:xml:ns:metalink">
  <published>2026-10-05T12:00:00Z</published>
  <origin>https://example.org/download.meta4</origin>
  <file name="example-linux.iso">
    <size>104857600</size>
    <identity>Example OS Linux</identity>
    <version>26.04</version>
    <language>en</language>
    <os>Linux</os>
    <hash type="sha-256">2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae</hash>
    <hash type="sha-512">cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e</hash>
    <pieces length="1048576" type="sha-256">
      <hash>e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855</hash>
      <hash>a591a6d40bf420404a011733cfb7b190d62c65bf0bcda32b57b277d9ad9f146e</hash>
    </pieces>
    <url priority="1" location="de" maxconnections="4">https://mirror1.example.org/example.iso</url>
    <url priority="2" location="us" maxconnections="2">https://mirror2.example.org/example.iso</url>
    <url priority="5" location="cn">http://mirror3.example.org/example.iso</url>
    <metaurl mediatype="torrent" priority="3">https://torrent.example.org/example.torrent</metaurl>
  </file>
</metalink>"#;

    let doc = parse_metalink_xml(xml).expect("parse metalink4");
    assert_eq!(doc.files.len(), 1);
    let file = &doc.files[0];
    assert_eq!(file.name, "example-linux.iso");
    assert_eq!(file.size, Some(104857600));
    assert_eq!(file.identity.as_deref(), Some("Example OS Linux"));
    assert_eq!(file.version.as_deref(), Some("26.04"));
    assert_eq!(file.os.as_deref(), Some("Linux"));

    // Check best checksum chooses sha-512 over sha-256
    let (algo, hash) = file.best_checksum().expect("best checksum");
    assert_eq!(algo, ChecksumMode::Sha512);
    assert!(hash.starts_with("cf83e1"));

    // Pieces check
    let pieces = file.pieces.as_ref().expect("pieces");
    assert_eq!(pieces.piece_length, 1048576);
    assert_eq!(pieces.algorithm, ChecksumMode::Sha256);
    assert_eq!(pieces.hashes.len(), 2);

    // Mirrors check
    assert_eq!(file.resources.len(), 3);
    assert_eq!(file.resources[0].priority, 1);
    assert_eq!(file.resources[0].location.as_deref(), Some("de"));
    assert_eq!(file.resources[0].max_connections, Some(4));

    // Metaurl check
    assert_eq!(file.metaurls.len(), 1);
    assert_eq!(file.metaurls[0].media_type, "torrent");

    let sorted_urls = file.sorted_mirror_urls();
    assert_eq!(sorted_urls[0], "https://mirror1.example.org/example.iso");
    assert_eq!(sorted_urls[1], "https://mirror2.example.org/example.iso");
    assert_eq!(sorted_urls[2], "http://mirror3.example.org/example.iso");
}

#[test]
fn test_parse_metalink3_legacy() {
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<metalink version="3.0" xmlns="http://www.metalinker.org/">
  <files>
    <file name="../../../malicious_name.tar.gz">
      <size>5242880</size>
      <verification>
        <hash type="sha256">e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855</hash>
      </verification>
      <resources>
        <url type="http" preference="100" location="cn">https://cn.mirror.org/file.tar.gz</url>
        <url type="http" preference="80" location="jp">https://jp.mirror.org/file.tar.gz</url>
        <url type="bittorrent" preference="50">http://tracker.org/file.torrent</url>
      </resources>
    </file>
  </files>
</metalink>"#;

    let doc = parse_metalink_xml(xml).expect("parse metalink3");
    assert_eq!(doc.files.len(), 1);
    let file = &doc.files[0];

    // Verify filename was sanitized
    assert_eq!(file.name, "malicious_name.tar.gz");
    assert_eq!(file.size, Some(5242880));

    // Preference 100 -> Priority 1, Preference 80 -> Priority 21
    assert_eq!(file.resources.len(), 2);
    assert_eq!(file.resources[0].priority, 1);
    assert_eq!(file.resources[1].priority, 21);

    // Bittorrent URL mapped to metaurls
    assert_eq!(file.metaurls.len(), 1);
    assert_eq!(file.metaurls[0].url, "http://tracker.org/file.torrent");
}

#[test]
fn test_parse_metalink_http_headers() {
    let mut headers = HeaderMap::new();
    headers.append(
        reqwest::header::LINK,
        HeaderValue::from_static(
            r#"<https://de.mirror.com/file.iso>; rel=duplicate; pri=1; geo=de, <https://us.mirror.com/file.iso>; rel=duplicate; pri=2; pref; geo=us"#,
        ),
    );
    headers.append(
        reqwest::header::LINK,
        HeaderValue::from_static(r#"<https://example.com/file.meta4>; rel=describedby; type="application/metalink4+xml""#),
    );
    headers.append(
        "digest",
        HeaderValue::from_static(
            "SHA-256=47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=",
        ),
    );

    let meta = parse_metalink_headers(&headers);
    assert_eq!(meta.mirrors.len(), 2);
    assert_eq!(meta.mirrors[0].url, "https://de.mirror.com/file.iso");
    assert_eq!(meta.mirrors[0].location.as_deref(), Some("de"));

    // pref flag pulls priority down to min 10
    assert_eq!(meta.mirrors[1].url, "https://us.mirror.com/file.iso");
    assert_eq!(meta.mirrors[1].location.as_deref(), Some("us"));

    assert_eq!(
        meta.metalink_document_url.as_deref(),
        Some("https://example.com/file.meta4")
    );

    assert_eq!(meta.hashes.len(), 1);
    assert_eq!(meta.hashes[0].algorithm, ChecksumMode::Sha256);
    // Base64 decoded to hex: 47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU= is empty string sha256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    assert_eq!(
        meta.hashes[0].hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_mirror_scoring() {
    let config = MirrorScoringConfig {
        preferred_location: Some("cn".to_string()),
        preferred_protocol: "https".to_string(),
    };

    let fast_cn_mirror = MirrorResource {
        url: "https://mirror.sjtu.edu.cn/ubuntu.iso".to_string(),
        priority: 1,
        location: Some("cn".to_string()),
        max_connections: Some(4),
    };

    let slow_us_mirror = MirrorResource {
        url: "http://mirror.mit.edu/ubuntu.iso".to_string(),
        priority: 2,
        location: Some("us".to_string()),
        max_connections: Some(2),
    };

    let score_cn = calculate_mirror_score(&fast_cn_mirror, Some(25), 0, &config);
    let score_us = calculate_mirror_score(&slow_us_mirror, Some(250), 0, &config);

    // Fast CN mirror with priority 1, HTTPS, geo-match, and low RTT must score much higher than slow US HTTP mirror
    assert!(score_cn > score_us);
    assert!(score_cn > 200.0);
}

#[test]
fn test_mirror_pool_leasing_and_concurrency_limit() {
    let resources = vec![
        MirrorResource {
            url: "https://mirror1.com".to_string(),
            priority: 1,
            location: None,
            max_connections: Some(2),
        },
        MirrorResource {
            url: "https://mirror2.com".to_string(),
            priority: 2,
            location: None,
            max_connections: Some(1),
        },
    ];

    let pool = MirrorPool::new(resources, MirrorScoringConfig::default(), 2);

    // Lease 1: should get mirror 1 (priority 1)
    let lease1 = pool.lease_best_mirror().expect("lease 1");
    assert_eq!(lease1.url, "https://mirror1.com");

    // Lease 2: should get mirror 1 again (max_connections is 2)
    let lease2 = pool.lease_best_mirror().expect("lease 2");
    assert_eq!(lease2.url, "https://mirror1.com");

    // Lease 3: mirror 1 is full, should get mirror 2
    let lease3 = pool.lease_best_mirror().expect("lease 3");
    assert_eq!(lease3.url, "https://mirror2.com");

    // Release lease1
    drop(lease1);

    // Lease 4: mirror 1 has capacity again!
    let lease4 = pool.lease_best_mirror().expect("lease 4");
    assert_eq!(lease4.url, "https://mirror1.com");

    // Drop all leases
    drop(lease2);
    drop(lease3);
    drop(lease4);

    let snapshots = pool.snapshots();
    assert_eq!(snapshots[0].active_connections, 0);
    assert_eq!(snapshots[1].active_connections, 0);
}
