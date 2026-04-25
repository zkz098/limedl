use super::{
    error::{DownloadError, Result},
    types::ChecksumMode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MetalinkEntry {
    pub(super) file_name: Option<String>,
    pub(super) url: String,
    pub(super) checksum_mode: Option<ChecksumMode>,
}

#[derive(Default)]
struct ParsedFile {
    file_name: Option<String>,
    urls: Vec<String>,
    checksum_mode: Option<ChecksumMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextTarget {
    Url,
    Hash,
}

pub(super) fn parse_metalink(content: &str) -> Result<Vec<MetalinkEntry>> {
    let mut entries = Vec::new();
    let mut current_file: Option<ParsedFile> = None;
    let mut text_target: Option<TextTarget> = None;
    let mut text_buffer = String::new();
    let mut pending_hash_mode: Option<ChecksumMode> = None;
    let mut cursor = 0;

    while let Some(start) = content[cursor..].find('<') {
        let tag_start = cursor + start;
        collect_text(&content[cursor..tag_start], text_target, &mut text_buffer);

        let Some(end) = content[tag_start..].find('>') else {
            return Err(DownloadError::InvalidResponse(String::from(
                "metalink xml contains an unterminated tag",
            )));
        };
        let tag_end = tag_start + end;
        let tag = content[tag_start + 1..tag_end].trim();
        cursor = tag_end + 1;

        if tag.is_empty() || tag.starts_with('?') || tag.starts_with('!') || tag.starts_with("--") {
            continue;
        }

        let is_close = tag.starts_with('/');
        let is_self_closing = tag.ends_with('/');
        let raw = tag.trim_start_matches('/').trim_end_matches('/').trim();
        let (name, attrs) = split_tag(raw);
        let local = local_name(name);

        if is_close {
            match local {
                "file" => {
                    if let Some(file) = current_file.take() {
                        push_file_entry(&mut entries, file);
                    }
                }
                "url" if text_target == Some(TextTarget::Url) => {
                    if let Some(file) = current_file.as_mut() {
                        let url = decode_xml_entities(text_buffer.trim());
                        if is_http_url(&url) {
                            file.urls.push(url);
                        }
                    }
                    text_buffer.clear();
                    text_target = None;
                }
                "hash" if text_target == Some(TextTarget::Hash) => {
                    if let Some(file) = current_file.as_mut() {
                        file.checksum_mode = file.checksum_mode.or(pending_hash_mode);
                    }
                    text_buffer.clear();
                    pending_hash_mode = None;
                    text_target = None;
                }
                _ => {}
            }
            continue;
        }

        match local {
            "file" => {
                current_file = Some(ParsedFile {
                    file_name: attribute(attrs, "name").filter(|value| !value.is_empty()),
                    ..ParsedFile::default()
                });
            }
            "url" if current_file.is_some() => {
                text_buffer.clear();
                text_target = Some(TextTarget::Url);
            }
            "hash" if current_file.is_some() => {
                text_buffer.clear();
                pending_hash_mode =
                    attribute(attrs, "type").and_then(|value| parse_checksum_mode(&value));
                text_target = Some(TextTarget::Hash);
            }
            _ => {}
        }

        if is_self_closing && local == "file" {
            current_file = None;
        }
    }

    collect_text(&content[cursor..], text_target, &mut text_buffer);

    if let Some(file) = current_file.take() {
        push_file_entry(&mut entries, file);
    }

    let entries = entries
        .into_iter()
        .filter(|entry| is_http_url(&entry.url))
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return Err(DownloadError::InvalidResponse(String::from(
            "metalink did not contain any http or https file urls",
        )));
    }

    Ok(entries)
}

fn push_file_entry(entries: &mut Vec<MetalinkEntry>, file: ParsedFile) {
    if let Some(url) = file.urls.into_iter().next() {
        entries.push(MetalinkEntry {
            file_name: file.file_name,
            url,
            checksum_mode: file.checksum_mode,
        });
    }
}

fn collect_text(text: &str, target: Option<TextTarget>, buffer: &mut String) {
    if target.is_some() {
        buffer.push_str(text);
    }
}

fn split_tag(raw: &str) -> (&str, &str) {
    if let Some(index) = raw.find(char::is_whitespace) {
        (&raw[..index], raw[index..].trim())
    } else {
        (raw, "")
    }
}

fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn attribute(attrs: &str, target: &str) -> Option<String> {
    let bytes = attrs.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }

        let key_start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && bytes[index] != b'='
            && bytes[index] != b'/'
        {
            index += 1;
        }
        let key = &attrs[key_start..index];

        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }

        if index >= bytes.len() || bytes[index] != b'=' {
            continue;
        }
        index += 1;

        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }

        if index >= bytes.len() || (bytes[index] != b'"' && bytes[index] != b'\'') {
            continue;
        }

        let quote = bytes[index];
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        let value = &attrs[value_start..index];
        index += usize::from(index < bytes.len());

        if local_name(key).eq_ignore_ascii_case(target) {
            return Some(decode_xml_entities(value));
        }
    }

    None
}

fn parse_checksum_mode(value: &str) -> Option<ChecksumMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "blake3" => Some(ChecksumMode::Blake3),
        "sha-256" | "sha256" => Some(ChecksumMode::Sha256),
        "xxh3_128" | "xxh3-128" => Some(ChecksumMode::Xxh3128),
        _ => None,
    }
}

fn is_http_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn decode_xml_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::parse_metalink;
    use crate::download::types::ChecksumMode;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

    #[test]
    fn parses_meta4_file_urls() -> TestResult {
        let entries = parse_metalink(
            r#"
            <metalink xmlns="urn:ietf:params:xml:ns:metalink">
              <file name="image.iso">
                <size>1024</size>
                <hash type="sha-256">abc</hash>
                <url priority="1">https://mirror.example.com/image.iso</url>
                <url>ftp://mirror.example.com/image.iso</url>
              </file>
            </metalink>
            "#,
        )?;

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file_name.as_deref(), Some("image.iso"));
        assert_eq!(entries[0].url, "https://mirror.example.com/image.iso");
        assert_eq!(entries[0].checksum_mode, Some(ChecksumMode::Sha256));
        Ok(())
    }

    #[test]
    fn parses_namespaced_tags_and_entities() -> TestResult {
        let entries = parse_metalink(
            r#"
            <m:metalink>
              <m:file name="a&amp;b.bin">
                <m:url>https://example.com/a&amp;b.bin</m:url>
              </m:file>
            </m:metalink>
            "#,
        )?;

        assert_eq!(entries[0].file_name.as_deref(), Some("a&b.bin"));
        assert_eq!(entries[0].url, "https://example.com/a&b.bin");
        Ok(())
    }
}
