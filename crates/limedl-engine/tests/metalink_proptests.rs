//! Property-based tests for Metalink XML and HTTP header parsers in `limedl-engine`.

use limedl_engine::metalink::{parse_metalink_headers, parse_metalink_xml};
use proptest::prelude::*;
use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn parse_metalink_xml_never_panics_on_arbitrary_input(xml in "\\PC{0,1024}") {
        if let Ok(doc) = parse_metalink_xml(&xml) {
            for file in &doc.files {
                prop_assert!(!file.name.is_empty());
                prop_assert!(!file.name.contains('/'));
                prop_assert!(!file.name.contains('\\'));
                prop_assert!(!file.name.contains('\0'));
                for mirror in &file.resources {
                    prop_assert!(mirror.priority >= 1);
                }
            }
        }
    }

    #[test]
    fn parse_metalink_xml_sanitizes_adversarial_file_names_and_attributes(
        raw_name in "(\\.\\./|\\.\\.\\\\|/etc/|C:\\\\){0,3}[a-zA-Z0-9._\\u{4e00}-\\u{9fff}-]{0,24}",
        pri in "\\PC{0,16}",
        len in "\\PC{0,16}",
        algo in "\\PC{0,16}",
        url in "\\PC{0,64}",
        hash in "\\PC{0,128}",
    ) {
        let escaped_name = raw_name
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('"', "&quot;");
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<metalink xmlns="urn:ietf:params:xml:ns:metalink">
  <file name="{escaped_name}">
    <size>{len}</size>
    <hash type="{algo}">{hash}</hash>
    <pieces length="{len}" type="{algo}">
      <hash>{hash}</hash>
    </pieces>
    <url priority="{pri}" maxconnections="{pri}">{url}</url>
  </file>
</metalink>"#
        );
        if let Ok(doc) = parse_metalink_xml(&xml) {
            for file in &doc.files {
                prop_assert!(!file.name.is_empty());
                prop_assert!(!file.name.contains('/'));
                prop_assert!(!file.name.contains('\\'));
                prop_assert!(!file.name.contains('\0'));
                prop_assert_ne!(&file.name, "..");
                prop_assert_ne!(&file.name, ".");
                for mirror in &file.resources {
                    prop_assert!(mirror.priority >= 1);
                }
            }
        }
    }

    #[test]
    fn parse_metalink_headers_never_panics_on_arbitrary_link_and_digest(
        links in prop::collection::vec("[\\x20-\\x7e]{0,160}", 0..4),
        digests in prop::collection::vec("[\\x20-\\x7e]{0,160}", 0..4),
    ) {
        let mut headers = HeaderMap::new();
        for link in links {
            if let Ok(val) = HeaderValue::from_str(&link) {
                headers.append(header::LINK, val);
            }
        }
        for digest in digests {
            if let Ok(val) = HeaderValue::from_str(&digest) {
                headers.append(HeaderName::from_static("digest"), val);
            }
        }
        let meta = parse_metalink_headers(&headers);
        for mirror in &meta.mirrors {
            prop_assert!(mirror.priority >= 1);
            prop_assert!(!mirror.url.is_empty());
        }
        for entry in &meta.hashes {
            prop_assert!(entry.hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
        }
    }
}
