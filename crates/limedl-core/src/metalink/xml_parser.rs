use quick_xml::Reader;
use quick_xml::events::Event;

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

    let mut files = Vec::new();
    let mut origin = None;
    let mut published = None;

    let mut current_file: Option<FileBuilder> = None;
    let mut current_tag = Vec::new();
    let mut in_pieces = false;
    let mut current_pieces: Option<PieceBuilder> = None;

    // Attributes preserved from Event::Start for Event::Text consumption
    let mut attr_type = None;
    let mut attr_priority = None;
    let mut attr_preference = None;
    let mut attr_location = None;
    let mut attr_max_connections = None;
    let mut attr_mediatype = None;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_ascii_lowercase();
                current_tag.push(tag_name.clone());

                let mut attr_name = None;
                let mut attr_length = None;

                attr_type = None;
                attr_priority = None;
                attr_preference = None;
                attr_location = None;
                attr_max_connections = None;
                attr_mediatype = None;

                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref()).to_ascii_lowercase();
                    let val = String::from_utf8_lossy(&attr.value).to_string();
                    match key.as_str() {
                        "name" => attr_name = Some(val),
                        "type" => attr_type = Some(val),
                        "priority" => attr_priority = val.parse::<u32>().ok(),
                        "preference" => attr_preference = val.parse::<u32>().ok(),
                        "location" => attr_location = Some(val.to_ascii_lowercase()),
                        "maxconnections" => attr_max_connections = val.parse::<usize>().ok(),
                        "mediatype" => attr_mediatype = Some(val),
                        "length" => attr_length = val.parse::<u64>().ok(),
                        _ => {}
                    }
                }

                match tag_name.as_str() {
                    "file" => {
                        let raw_name = attr_name.take().unwrap_or_default();
                        let base_name = std::path::Path::new(&raw_name)
                            .file_name()
                            .and_then(|f| f.to_str())
                            .unwrap_or(&raw_name);
                        let safe_name = sanitize_filename::sanitize(base_name);
                        current_file = Some(FileBuilder {
                            name: if safe_name.is_empty() {
                                "download".to_string()
                            } else {
                                safe_name
                            },
                            ..Default::default()
                        });
                    }
                    "pieces" => {
                        in_pieces = true;
                        if let Some(len) = attr_length {
                            let algo = attr_type
                                .as_deref()
                                .and_then(parse_algo)
                                .unwrap_or(ChecksumMode::Sha256);
                            current_pieces = Some(PieceBuilder {
                                piece_length: len,
                                algorithm: algo,
                                hashes: Vec::new(),
                            });
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                let raw_str = std::str::from_utf8(e.as_ref()).unwrap_or("");
                let text = quick_xml::escape::unescape(raw_str).unwrap_or_default().trim().to_string();
                if text.is_empty() {
                    continue;
                }

                let parent = current_tag.last().map(String::as_str).unwrap_or("");
                match parent {
                    "origin" => {
                        origin = Some(text);
                        continue;
                    }
                    "published" => {
                        published = Some(text);
                        continue;
                    }
                    _ => {}
                }

                if let Some(ref mut file) = current_file {
                    match parent {
                        "size" => {
                            if let Ok(size) = text.parse::<u64>() {
                                file.size = Some(size);
                            }
                        }
                        "identity" => file.identity = Some(text),
                        "version" => file.version = Some(text),
                        "language" => file.language = Some(text),
                        "os" => file.os = Some(text),
                        "hash" => {
                            if in_pieces {
                                if let Some(ref mut pieces) = current_pieces {
                                    pieces.hashes.push(text.to_ascii_lowercase());
                                }
                            } else {
                                let raw_type = attr_type.clone().unwrap_or_else(|| "sha-256".into());
                                let algo = parse_algo(&raw_type).unwrap_or(ChecksumMode::None);
                                file.hashes.push(ChecksumEntry {
                                    algorithm: algo,
                                    raw_type,
                                    hash: text.to_ascii_lowercase(),
                                });
                            }
                        }
                        "url" => {
                            let normalized_priority = if let Some(pri) = attr_priority {
                                pri.max(1)
                            } else if let Some(pref) = attr_preference {
                                (101u32).saturating_sub(pref.min(100)).max(1)
                            } else {
                                100
                            };

                            let url_type = attr_type.as_deref().unwrap_or("http");
                            if url_type.eq_ignore_ascii_case("bittorrent") {
                                file.metaurls.push(MetalinkMetaUrl {
                                    url: text,
                                    media_type: "torrent".into(),
                                    priority: normalized_priority,
                                });
                            } else if text.starts_with("http://") || text.starts_with("https://") {
                                file.resources.push(MirrorResource {
                                    url: text,
                                    priority: normalized_priority,
                                    location: attr_location.clone(),
                                    max_connections: attr_max_connections,
                                });
                            }
                        }
                        "metaurl" => {
                            let priority = attr_priority.unwrap_or(100).max(1);
                            let media_type = attr_mediatype.clone().unwrap_or_else(|| "torrent".into());
                            file.metaurls.push(MetalinkMetaUrl {
                                url: text,
                                media_type,
                                priority,
                            });
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_ascii_lowercase();
                if current_tag.last().is_some_and(|t| t == &tag_name) {
                    current_tag.pop();
                }

                match tag_name.as_str() {
                    "file" => {
                        if let Some(builder) = current_file.take() {
                            files.push(builder.build());
                        }
                    }
                    "pieces" => {
                        in_pieces = false;
                        if let (Some(file), Some(pieces)) =
                            (&mut current_file, current_pieces.take())
                        {
                            file.pieces = Some(pieces.build());
                        }
                    }
                    _ => {}
                }
            }
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

    if files.is_empty() {
        return Err(DownloadError::InvalidResponse(
            "Metalink XML contains no valid <file> entries".into(),
        ));
    }

    Ok(MetalinkDocument {
        files,
        origin,
        published,
    })
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
