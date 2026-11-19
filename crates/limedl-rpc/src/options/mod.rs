//! Per-task and global option methods plus aria2 option parsing helpers.

mod global;
mod parse;
mod task;

#[cfg(test)]
mod tests;

pub use global::{handle_change_global_option, handle_get_global_option};
pub(crate) use parse::option_is_true;
pub use parse::{
    collect_request_headers, extract_option_headers, extract_option_str, extract_option_u32,
    extract_option_usize, find_header_value, parse_aria2_limit, parse_checksum_option,
    parse_select_file, split_aria2_tail,
};
pub use task::{handle_change_option, handle_get_option};
#[cfg(test)]
pub use tests::{ARIA2NG_GLOBAL_OPTION_KEYS, ARIA2NG_TASK_OPTION_KEYS};
