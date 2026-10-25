use quick_xml::Reader;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};

use super::types::{
    ChecksumEntry, MetalinkDocument, MetalinkFile, MetalinkMetaUrl, MirrorResource,
    PieceVerification,
};
use crate::error::{DownloadError, Result};
use crate::types::ChecksumMode;

/// Parse a Metalink 4.0 (RFC 5854) or Metalink 3.0 XML string.
pub fn parse_metalink_xml(xml: &str) -> Result<MetalinkDocument> {
    if xml.len() > 10 * 1024 * 1024 {
        return Err(DownloadError::InvalidRequest(
            "Metalink XML payload exceeds 10MB limit".into(),
        ));
    }

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut builder = MetalinkBuilder::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => builder.on_start(e),
            Ok(Event::Text(ref e)) => builder.on_text(e),
            Ok(Event::End(ref e)) => builder.on_end(e),
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(DownloadError::InvalidResponse(format!(
                    "Invalid Metalink XML format: {e}"
                )));
            }
            _ => {}
        }
        buf.clear();
    }

    builder.finish()
}

/// Attributes preserved from an `Event::Start` for the following `Event::Text`.
#[derive(Default)]
struct MetalinkAttrs {
    name: Option<String>,
    type_: Option<String>,
    priority: Option<u32>,
    preference: Option<u32>,
    location: Option<String>,
    max_connections: Option<usize>,
    mediatype: Option<String>,
    length: Option<u64>,
}

impl MetalinkAttrs {
    fn from_event(e: &BytesStart) -> Self {
        let mut attrs = Self::default();
        for attr in e.attributes().flatten() {
            let key = attr.key.as_ref().to_ascii_lowercase();
            let val = attr.value.to_string();
            match key.as_str() {
                "name" => attrs.name = Some(val),
                "type" => attrs.type_ = Some(val),
                "priority" => attrs.priority = val.parse::<u32>().ok(),
                "preference" => attrs.preference = val.parse::<u32>().ok(),
                "location" => attrs.location = Some(val.to_ascii_lowercase()),
                "maxconnections" => attrs.max_connections = val.parse::<usize>().ok(),
                "mediatype" => attrs.mediatype = Some(val),
                "length" => attrs.length = val.parse::<u64>().ok(),
                _ => {}
            }
        }
        attrs
    }
}

/// Streaming state machine for one Metalink document.
#[derive(Default)]
struct MetalinkBuilder {
    files: Vec<MetalinkFile>,
    origin: Option<String>,
    published: Option<String>,
    current_file: Option<FileBuilder>,
    current_tag: Vec<String>,
    in_pieces: bool,
    current_pieces: Option<PieceBuilder>,
    attrs: MetalinkAttrs,
}

impl MetalinkBuilder {
    fn on_start(&mut self, e: &BytesStart) {
        let tag_name = e.name().as_ref().to_ascii_lowercase();
        self.current_tag.push(tag_name.clone());
        self.attrs = MetalinkAttrs::from_event(e);

        match tag_name.as_str() {
            "file" => self.start_file(),
            "pieces" => self.start_pieces(),
            _ => {}
        }
    }

    fn start_file(&mut self) {
        let raw_name = self.attrs.name.take().unwrap_or_default();
        let base_name = std::path::Path::new(&raw_name)
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or(&raw_name);
        let safe_name = sanitize_filename::sanitize(base_name);
        self.current_file = Some(FileBuilder {
            name: if safe_name.is_empty() {
                "download".to_string()
            } else {
                safe_name
            },
            ..Default::default()
        });
    }

    fn start_pieces(&mut self) {
        self.in_pieces = true;
        if let Some(len) = self.attrs.length {
            let algo = self
                .attrs
                .type_
                .as_deref()
                .and_then(parse_algo)
                .unwrap_or(ChecksumMode::Sha256);
            self.current_pieces = Some(PieceBuilder {
                piece_length: len,
                algorithm: algo,
                hashes: Vec::new(),
            });
        }
    }

    fn on_text(&mut self, e: &BytesText) {
        let text = quick_xml::escape::unescape(e.as_ref())
            .unwrap_or_default()
            .trim()
            .to_string();
        if text.is_empty() {
            return;
        }

        let parent = self.current_tag.last().cloned().unwrap_or_default();
        match parent.as_str() {
            "origin" => {
                self.origin = Some(text);
                return;
            }
            "published" => {
                self.published = Some(text);
                return;
            }
            _ => {}
        }

        if self.current_file.is_some() {
            self.handle_file_text(&parent, text);
        }
    }

    fn handle_file_text(&mut self, parent: &str, text: String) {
        match parent {
            "size" => {
                if let Ok(size) = text.parse::<u64>()
                    && let Some(file) = &mut self.current_file
                {
                    file.size = Some(size);
                }
            }
            "identity" => {
                if let Some(file) = &mut self.current_file {
                    file.identity = Some(text);
                }
            }
            "version" => {
                if let Some(file) = &mut self.current_file {
                    file.version = Some(text);
                }
            }
            "language" => {
                if let Some(file) = &mut self.current_file {
                    file.language = Some(text);
                }
            }
            "os" => {
                if let Some(file) = &mut self.current_file {
                    file.os = Some(text);
                }
            }
            "hash" => self.handle_hash_text(text),
            "url" => self.handle_url_text(text),
            "metaurl" => self.handle_metaurl_text(text),
            _ => {}
        }
    }

    fn handle_hash_text(&mut self, text: String) {
        if self.in_pieces {
            if let Some(pieces) = &mut self.current_pieces {
                pieces.hashes.push(text.to_ascii_lowercase());
            }
            return;
        }

        let raw_type = self.attrs.type_.clone().unwrap_or_else(|| "sha-256".into());
        let algo = parse_algo(&raw_type).unwrap_or(ChecksumMode::None);
        if let Some(file) = &mut self.current_file {
            file.hashes.push(ChecksumEntry {
                algorithm: algo,
                raw_type,
                hash: text.to_ascii_lowercase(),
            });
        }
    }

    fn handle_url_text(&mut self, text: String) {
        let normalized_priority = if let Some(pri) = self.attrs.priority {
            pri.max(1)
        } else if let Some(pref) = self.attrs.preference {
            (101u32).saturating_sub(pref.min(100)).max(1)
        } else {
            100
        };

        let url_type = self.attrs.type_.as_deref().unwrap_or("http");
        if url_type.eq_ignore_ascii_case("bittorrent") {
            if let Some(file) = &mut self.current_file {
                file.metaurls.push(MetalinkMetaUrl {
                    url: text,
                    media_type: "torrent".into(),
                    priority: normalized_priority,
                });
            }
        } else if (text.starts_with("http://") || text.starts_with("https://"))
            && let Some(file) = &mut self.current_file
        {
            file.resources.push(MirrorResource {
                url: text,
                priority: normalized_priority,
                location: self.attrs.location.clone(),
                max_connections: self.attrs.max_connections,
            });
        }
    }

    fn handle_metaurl_text(&mut self, text: String) {
        let priority = self.attrs.priority.unwrap_or(100).max(1);
        let media_type = self
            .attrs
            .mediatype
            .clone()
            .unwrap_or_else(|| "torrent".into());
        if let Some(file) = &mut self.current_file {
            file.metaurls.push(MetalinkMetaUrl {
                url: text,
                media_type,
                priority,
            });
        }
    }

    fn on_end(&mut self, e: &BytesEnd) {
        let tag_name = e.name().as_ref().to_ascii_lowercase();
        if self.current_tag.last().is_some_and(|t| t == &tag_name) {
            self.current_tag.pop();
        }

        match tag_name.as_str() {
            "file" => {
                if let Some(builder) = self.current_file.take() {
                    self.files.push(builder.build());
                }
            }
            "pieces" => {
                self.in_pieces = false;
                if let Some(pieces) = self.current_pieces.take()
                    && let Some(file) = &mut self.current_file
                {
                    file.pieces = Some(pieces.build());
                }
            }
            _ => {}
        }
    }

    fn finish(self) -> Result<MetalinkDocument> {
        if self.files.is_empty() {
            return Err(DownloadError::InvalidResponse(
                "Metalink XML contains no valid <file> entries".into(),
            ));
        }

        Ok(MetalinkDocument {
            files: self.files,
            origin: self.origin,
            published: self.published,
        })
    }
}

fn parse_algo(raw: &str) -> Option<ChecksumMode> {
    let lower = raw.trim().to_ascii_lowercase();
    match lower.as_str() {
        "sha-256" | "sha256" => Some(ChecksumMode::Sha256),
        "sha-512" | "sha512" => Some(ChecksumMode::Sha512),
        "blake3" => Some(ChecksumMode::Blake3),
        _ => None,
    }
}

#[derive(Default)]
struct FileBuilder {
    name: String,
    size: Option<u64>,
    hashes: Vec<ChecksumEntry>,
    pieces: Option<PieceVerification>,
    resources: Vec<MirrorResource>,
    metaurls: Vec<MetalinkMetaUrl>,
    identity: Option<String>,
    version: Option<String>,
    language: Option<String>,
    os: Option<String>,
}

impl FileBuilder {
    fn build(self) -> MetalinkFile {
        MetalinkFile {
            name: self.name,
            size: self.size,
            hashes: self.hashes,
            pieces: self.pieces,
            resources: self.resources,
            metaurls: self.metaurls,
            identity: self.identity,
            version: self.version,
            language: self.language,
            os: self.os,
        }
    }
}

struct PieceBuilder {
    piece_length: u64,
    algorithm: ChecksumMode,
    hashes: Vec<String>,
}

impl PieceBuilder {
    fn build(self) -> PieceVerification {
        PieceVerification {
            piece_length: self.piece_length,
            algorithm: self.algorithm,
            hashes: self.hashes,
        }
    }
}
