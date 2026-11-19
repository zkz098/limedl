use std::path::Path;

use crate::types::DiskType;

// NOTE: This fallback is for platforms without a native disk detection
// backend (not Windows/Linux/macOS). Linux and macOS have their own
// implementions above; this module only covers unknown targets.
pub fn detect_disk_type(_path: &Path) -> DiskType {
    DiskType::Ssd
}

use std::collections::HashMap;

pub fn detect_all_disk_types() -> HashMap<String, DiskType> {
    HashMap::new()
}
