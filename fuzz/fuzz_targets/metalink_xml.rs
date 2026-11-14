#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct FuzzMirror {
    priority: String,
    maxconnections: String,
    location: String,
    url: String,
}

#[derive(Arbitrary, Debug)]
struct FuzzFile {
    name: String,
    size: String,
    hash_type: String,
    hash_val: String,
    pieces_len: String,
    pieces_type: String,
    piece_hashes: Vec<String>,
    mirrors: Vec<FuzzMirror>,
}

#[derive(Arbitrary, Debug)]
enum FuzzInput {
    Structured { v4: bool, files: Vec<FuzzFile> },
    Raw(String),
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fuzz_target!(|input: FuzzInput| {
    let xml = match input {
        FuzzInput::Structured { v4, files } => {
            let mut out = String::from(if v4 {
                r#"<?xml version="1.0" encoding="UTF-8"?><metalink xmlns="urn:ietf:params:xml:ns:metalink">"#
            } else {
                r#"<?xml version="1.0" encoding="UTF-8"?><metalink version="3.0" xmlns="http://www.metalinker.org/"><files>"#
            });
            for f in files.into_iter().take(4) {
                out.push_str(&format!(
                    r#"<file name="{}"><size>{}</size><hash type="{}">{}</hash><pieces length="{}" type="{}">"#,
                    escape_xml(&f.name),
                    escape_xml(&f.size),
                    escape_xml(&f.hash_type),
                    escape_xml(&f.hash_val),
                    escape_xml(&f.pieces_len),
                    escape_xml(&f.pieces_type),
                ));
                for ph in f.piece_hashes.into_iter().take(16) {
                    out.push_str(&format!("<hash>{}</hash>", escape_xml(&ph)));
                }
                out.push_str("</pieces>");
                for m in f.mirrors.into_iter().take(8) {
                    out.push_str(&format!(
                        r#"<url priority="{}" maxconnections="{}" location="{}">{}</url>"#,
                        escape_xml(&m.priority),
                        escape_xml(&m.maxconnections),
                        escape_xml(&m.location),
                        escape_xml(&m.url),
                    ));
                }
                out.push_str("</file>");
            }
            out.push_str(if v4 {
                "</metalink>"
            } else {
                "</files></metalink>"
            });
            out
        }
        FuzzInput::Raw(raw) => raw,
    };

    if let Ok(doc) = limedl_engine::metalink::parse_metalink_xml(&xml) {
        for file in &doc.files {
            assert!(!file.name.is_empty());
            assert!(!file.name.contains('/'));
            assert!(!file.name.contains('\\'));
            assert!(!file.name.contains('\0'));
        }
    }
});
