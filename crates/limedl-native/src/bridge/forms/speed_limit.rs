use limedl_core::types::AppSettings;
use slint::SharedString;

use crate::SpeedLimitSlotItem;
use crate::i18n::{self, Language};

/// Text state of one schedule row as typed by the user (authoritative until
/// save, so half-typed values survive model rebuilds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedLimitSlotText {
    pub start_hour: String,
    pub end_hour: String,
    pub limit_kb: String,
}

impl Default for SpeedLimitSlotText {
    fn default() -> Self {
        Self {
            start_hour: "0".to_string(),
            end_hour: "6".to_string(),
            limit_kb: "0".to_string(),
        }
    }
}

/// Parse a schedule text field into u32, rejecting empty/garbage input.
fn parse_schedule_u32(
    raw: &str,
    field: i18n::SettingsField,
    lang: Language,
) -> Result<u32, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(i18n::format_validation_integer(lang, field, raw));
    }
    trimmed
        .parse::<u32>()
        .map_err(|_| i18n::format_validation_integer(lang, field, trimmed))
}

/// Validate + convert the schedule rows into the core type.
///
/// Hours must be 0-23 (24-hour clock); the limit may be any u64 KB/s value
/// (0 = unlimited). Returns a localized message on the first invalid row.
pub fn parse_speed_limit_slots(
    rows: &[SpeedLimitSlotText],
    lang: Language,
) -> Result<Vec<limedl_core::types::SpeedLimitSlot>, String> {
    let mut slots = Vec::with_capacity(rows.len());
    for row in rows {
        let start = parse_schedule_u32(
            &row.start_hour,
            i18n::SettingsField::SpeedLimitStartHour,
            lang,
        )?;
        let end = parse_schedule_u32(&row.end_hour, i18n::SettingsField::SpeedLimitEndHour, lang)?;
        let limit_kb =
            parse_schedule_u32(&row.limit_kb, i18n::SettingsField::SpeedLimitLimit, lang)?;
        if start > 23 {
            return Err(i18n::format_validation_range(
                lang,
                i18n::SettingsField::SpeedLimitStartHour,
                0,
                23,
            ));
        }
        if end > 23 {
            return Err(i18n::format_validation_range(
                lang,
                i18n::SettingsField::SpeedLimitEndHour,
                0,
                23,
            ));
        }
        slots.push(limedl_core::types::SpeedLimitSlot {
            start_hour: start as u8,
            end_hour: end as u8,
            limit_bps: limit_kb as u64 * 1024,
        });
    }
    Ok(slots)
}

/// Build the Slint model items for the schedule editor.
pub fn speed_limit_slots_to_slint(
    rows: &[SpeedLimitSlotText],
    lang: Language,
) -> Vec<SpeedLimitSlotItem> {
    rows.iter()
        .map(|row| {
            let start = row.start_hour.trim().parse::<u32>().unwrap_or(0).min(23);
            let end = row.end_hour.trim().parse::<u32>().unwrap_or(0).min(23);
            let limit = row.limit_kb.trim().parse::<u64>().unwrap_or(0);
            SpeedLimitSlotItem {
                start_hour: SharedString::from(row.start_hour.as_str()),
                end_hour: SharedString::from(row.end_hour.as_str()),
                limit_kb: SharedString::from(row.limit_kb.as_str()),
                wraps: start >= end,
                summary: SharedString::from(i18n::format_schedule_summary(start, end, limit, lang)),
            }
        })
        .collect()
}

/// Convert persisted settings into the editor's per-row text state.
pub fn speed_limit_slots_from_settings(settings: &AppSettings) -> Vec<SpeedLimitSlotText> {
    settings
        .speed_limit_schedule
        .iter()
        .map(|slot| SpeedLimitSlotText {
            start_hour: slot.start_hour.to_string(),
            end_hour: slot.end_hour.to_string(),
            limit_kb: (slot.limit_bps / 1024).to_string(),
        })
        .collect()
}
