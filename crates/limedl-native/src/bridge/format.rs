use std::collections::HashMap;

use limedl_core::types::DiskType;

use crate::i18n::{self, Language};

/// Human-readable byte formatting.
pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;

    if bytes >= TIB {
        format!("{:.2} TB", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.2} GB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Format download speed in bytes/second.
pub fn format_speed(speed: Option<f64>) -> String {
    match speed {
        Some(s) if s > 0.0 => format!("{}/s", format_bytes(s as u64)),
        _ => String::new(),
    }
}

/// Format ETA seconds.
pub fn format_eta(eta: Option<u64>, lang: Language) -> String {
    i18n::format_eta(eta, lang)
}

/// Format detected disk types into readable summary text.
pub fn format_disk_types_map(disks: &HashMap<String, DiskType>, lang: Language) -> String {
    if disks.is_empty() {
        return i18n::format_no_disk_detected(lang).to_string();
    }

    let mut parts = Vec::new();
    for (path, disk_type) in disks {
        let type_name = i18n::format_disk_type_name(*disk_type, lang);
        parts.push(format!("{path} ({type_name})"));
    }
    parts.join(" | ")
}

/// Format buffer pool / IO status JSON payload.
pub fn format_io_status_json(val: &serde_json::Value, lang: Language) -> String {
    let allocated = val.get("allocatedBytes").and_then(|v| v.as_u64()).unwrap_or(0);
    let capacity = val.get("capacityBytes").and_then(|v| v.as_u64()).unwrap_or(1024 * 1024 * 1024);
    let active_buffers = val.get("activeBuffers").and_then(|v| v.as_u64()).unwrap_or(0);

    i18n::format_io_status_line(
        &format_bytes(allocated),
        &format_bytes(capacity),
        active_buffers,
        lang,
    )
}


pub fn format_timestamp_ms(ts: u64) -> String {
    let secs = (ts / 1000) as i64;
    if let Ok(dt) = time::OffsetDateTime::from_unix_timestamp(secs) {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            dt.year(),
            dt.month() as u8,
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second()
        )
    } else {
        format!("{secs}")
    }
}

