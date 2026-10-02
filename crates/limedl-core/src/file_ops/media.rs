//! Directory → media policy shared by every platform detector.
//!
//! Two platform-independent pieces live here so the Windows, Linux and macOS
//! backends cannot disagree about them, and so both are unit-testable on any
//! host:
//!
//! * [`MediaOverrides`] — the `io_baseline.disk_type_overrides` lookup. Keys are
//!   user-typed *directories* while the query is a download destination, usually
//!   a descendant of one of them, so lookup is a component-boundary prefix match
//!   over normalized paths rather than the exact-string `HashMap::get` it used
//!   to be (`D:\downloads` never matched `D:\downloads\`, let alone a file
//!   inside it).
//! * [`is_network_filesystem`] — the fstype classifier behind
//!   [`DiskType::Network`] on Linux/macOS, where the local block-device
//!   heuristics say nothing about a network mount.

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::path::Path;

use crate::types::DiskType;

/// Normalize a directory path for override comparison.
///
/// The comparison must be insensitive to spelling differences that do not change
/// which directory a path refers to:
///
/// * a trailing separator (`D:\dl\` is `D:\dl`) — the old exact-string match made
///   these two different keys, so a correctly configured override silently never
///   applied;
/// * separator style on Windows (`D:/dl` is `D:\dl`);
/// * case on Windows (`Z:` is `z:`, and so are UNC server and share names);
/// * the verbatim spelling Rust produces for long and UNC paths (`\\?\C:\dl`,
///   `\\?\UNC\srv\share` — a canonicalized path arrives in that shape).
///
/// A root keeps at least one character (`/`, `C:`, `\\server\share`) so that
/// stripping trailing separators can never produce the empty string, which would
/// otherwise match every directory in [`MediaOverrides::lookup`].
pub fn normalize_media_path(raw: &str) -> String {
    let mut normalized = raw.trim().to_string();

    #[cfg(windows)]
    {
        if let Some(rest) = normalized.strip_prefix(r"\\?\UNC\") {
            normalized = format!(r"\\{rest}");
        } else if let Some(rest) = normalized.strip_prefix(r"\\?\") {
            normalized = rest.to_string();
        }
        normalized = normalized.replace('/', "\\").to_lowercase();
    }

    while normalized.len() > 1 && (normalized.ends_with('\\') || normalized.ends_with('/')) {
        normalized.pop();
    }
    normalized
}

/// Is `raw` a directory an override can be keyed on?
///
/// Detection on a relative path would have to guess — the Windows detector
/// answers "SSD" for one — so the settings editor rejects them up front instead
/// of persisting a key that can never match a download destination.
pub fn is_usable_override_key(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return false;
    }
    if Path::new(trimmed).is_absolute() {
        return true;
    }
    #[cfg(windows)]
    {
        // `\\server\share` is a rooted location, but `Path::is_absolute()` is
        // false for the spelling without a trailing separator — which is exactly
        // how a user types a share.
        if is_unc_root(trimmed) {
            return true;
        }
    }
    false
}

/// `\\server\share` (with or without a trailing separator or a nested path).
#[cfg(windows)]
fn is_unc_root(raw: &str) -> bool {
    let Some(rest) = raw.strip_prefix(r"\\") else {
        return false;
    };
    let mut parts = rest
        .split(['\\', '/'])
        .filter(|part| !part.is_empty());
    matches!((parts.next(), parts.next()), (Some(_), Some(_)))
}

/// Does the override key `key` cover the (normalized) directory `dir`?
///
/// The separator check is what keeps `D:\dl` from covering `D:\dl2`: an override
/// must match on a component boundary.
fn key_covers(dir: &str, key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    match dir.strip_prefix(key) {
        Some(rest) => rest.is_empty() || rest.starts_with('\\') || rest.starts_with('/'),
        None => false,
    }
}

/// Snapshot of `disk_type_overrides` with pre-normalized, longest-key-first
/// entries.
///
/// Built once per settings change rather than per lookup: the device topology
/// resolves every download destination and cannot afford to normalize the whole
/// map each time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaOverrides {
    /// `(normalized key, media)`, sorted by descending key length so the first
    /// match is the most specific override — a nested rule must win over its
    /// parent — and so two snapshots of equal maps compare equal regardless of
    /// `HashMap` iteration order.
    entries: Vec<(String, DiskType)>,
}

impl MediaOverrides {
    /// Normalize and sort an override map.
    pub fn new<S: BuildHasher>(overrides: &HashMap<String, DiskType, S>) -> Self {
        let mut entries: Vec<(String, DiskType)> = overrides
            .iter()
            .map(|(key, media)| (normalize_media_path(key), *media))
            .collect();
        entries.sort_by(|(left, _), (right, _)| {
            right.len().cmp(&left.len()).then_with(|| left.cmp(right))
        });
        Self { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Media forced for `dir`, if any override covers it.
    pub fn lookup(&self, dir: &Path) -> Option<DiskType> {
        if self.entries.is_empty() {
            return None;
        }
        let dir = normalize_media_path(&dir.to_string_lossy());
        self.entries
            .iter()
            .find(|(key, _)| key_covers(&dir, key))
            .map(|(_, media)| *media)
    }
}

/// [`MediaOverrides::lookup`] for callers that hold the settings map.
///
/// Rebuilding the snapshot per call is deliberate: it keeps the normalization
/// rules in exactly one place, and the async callers that need it (the per-start
/// buffer-mode decision) read a map with a handful of entries at most.
pub fn lookup_media_override<S: BuildHasher>(
    overrides: &HashMap<String, DiskType, S>,
    dir: &Path,
) -> Option<DiskType> {
    MediaOverrides::new(overrides).lookup(dir)
}

/// Does this filesystem type name a location whose media the local block-device
/// heuristics cannot see?
///
/// The list is deliberately a *closed* set of remote and host-brokered
/// transports rather than "anything that is not ext4/xfs/apfs": an unknown
/// fstype — a brand-new local filesystem, or a plain `fuse` mount such as
/// ntfs-3g — must keep the previous behaviour of being reported as SSD instead
/// of being called remote.
///
/// Host-brokered entries (`9p`, `virtiofs`, `drvfs`, VM guest additions) are
/// included because they forward the *host's* storage into a VM or container: a
/// WSL guest cannot tell whether the host volume behind `\\wsl$` / `/mnt/c` is a
/// platter or flash, so "unknown media" is the honest answer there too. Inside
/// WSL that is still an improvement over the silent SSD claim, because the
/// per-directory override can now reach the device queue.
pub fn is_network_filesystem(fstype: &str) -> bool {
    let fstype = fstype.trim().to_ascii_lowercase();
    matches!(
        fstype.as_str(),
        // Network protocols.
        "nfs"
            | "nfs4"
            | "cifs"
            | "smb"
            | "smb2"
            | "smb3"
            | "smbfs"
            | "afpfs"
            | "webdav"
            | "davfs"
            | "davfs2"
            | "gfs"
            | "gfs2"
            | "ocfs2"
            | "glusterfs"
            | "ceph"
            | "beegfs"
            | "lustre"
            | "afs"
            // Host-brokered / forwarded filesystems.
            | "9p"
            | "virtiofs"
            | "drvfs"
            | "prl_fs"
            | "vboxsf"
            | "vmhgfs"
            | "osxfuse"
            | "macfuse"
            // Remote FUSE backends. Bare `fuse` is intentionally absent: it also
            // covers local filesystems like ntfs-3g.
            | "fuse.sshfs"
            | "fuse.smbnetfs"
            | "fuse.rclone"
            | "fuse.gvfsd-fuse"
            | "fuse.davfs2"
            | "fuse.s3fs"
            | "fuse.gcsfuse"
            | "fuse.blobfuse"
            | "fuse.curlftpfs"
    )
}

/// Is `path` inside the directory `ancestor` (or equal to it)?
///
/// `strip_prefix` compares whole components, so `/mnt/nas2` is not inside
/// `/mnt/nas`. Windows path semantics apply when compiled for Windows, which is
/// only relevant to the callers that pass Windows paths.
pub fn path_is_within(path: &Path, ancestor: &Path) -> bool {
    path.strip_prefix(ancestor).is_ok()
}

/// `(mount point, fstype)` for every entry in a `/proc/self/mountinfo` dump.
///
/// Format: `id parent major:minor root mount-point options [optional…] - fstype source super-options`.
/// The optional fields make the position of `fstype` variable, so the parser
/// keys off the `-` separator rather than counting from either end. Mount points
/// are unescaped; a malformed line is skipped rather than failing the scan.
///
/// Lives here rather than in the Linux backend so it can be unit-tested on every
/// host: the gate's Windows job never compiles `cfg(target_os = "linux")` code,
/// which is exactly how a broken parser would reach a Linux CI run unnoticed.
pub fn parse_mountinfo(content: &str) -> Vec<(String, String)> {
    content
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(' ').collect();
            let separator = fields.iter().position(|field| *field == "-")?;
            let mount_point = unescape_mountinfo(fields.get(4)?);
            let fstype = fields.get(separator + 1)?;
            // A truncated or trailing-separator line leaves one of them empty,
            // and an empty mount point or fstype would only pollute the scan.
            if mount_point.is_empty() || fstype.is_empty() {
                return None;
            }
            Some((mount_point, fstype.to_string()))
        })
        .collect()
}

/// `/proc/self/mountinfo` escapes space, tab, newline and backslash as octal
/// three-digit sequences (`\040` for a space).
fn unescape_mountinfo(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = bytes[index] == b'\\'
            && index + 4 <= bytes.len()
            && bytes[index + 1..index + 4]
                .iter()
                .all(|digit| (b'0'..=b'7').contains(digit));
        if escaped {
            // The three digits are ASCII, so the slice is on char boundaries.
            let code =
                u32::from_str_radix(&field[index + 1..index + 4], 8).unwrap_or(u32::from(b'?'));
            decoded.push(u8::try_from(code).unwrap_or(b'?'));
            index += 4;
            continue;
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overrides(entries: &[(&str, DiskType)]) -> HashMap<String, DiskType> {
        entries
            .iter()
            .map(|(key, media)| (key.to_string(), *media))
            .collect()
    }

    #[test]
    fn normalize_keeps_roots_and_drops_trailing_separators() {
        assert_eq!(
            normalize_media_path("D:\\downloads\\"),
            normalize_media_path("D:\\downloads")
        );
        assert_eq!(
            normalize_media_path("  D:\\downloads  "),
            normalize_media_path("D:\\downloads")
        );
        // Windows folds case, so `d:\dl` and `D:\dl` are the same key.
        #[cfg(windows)]
        assert_eq!(normalize_media_path("D:\\downloads\\"), "d:\\downloads");
        // Unix keeps case: two directories differing only in case are two
        // directories.
        #[cfg(unix)]
        assert_eq!(normalize_media_path("/mnt/Downloads/"), "/mnt/Downloads");

        // A root must not collapse to nothing, or it would match everything.
        #[cfg(windows)]
        assert_eq!(normalize_media_path("/"), "\\");
        #[cfg(unix)]
        assert_eq!(normalize_media_path("/"), "/");
        assert!(!normalize_media_path("/").is_empty());
        assert!(!normalize_media_path("\\\\server\\share\\").is_empty());
        assert!(!normalize_media_path("C:\\").is_empty());
    }

    #[test]
    fn override_matches_descendants_on_component_boundaries() {
        let map = overrides(&[("D:\\downloads", DiskType::Hdd)]);

        assert_eq!(
            lookup_media_override(&map, Path::new("D:\\downloads\\movie.mkv")),
            Some(DiskType::Hdd)
        );
        // The directory itself, in every spelling.
        assert_eq!(
            lookup_media_override(&map, Path::new("D:\\downloads")),
            Some(DiskType::Hdd)
        );
        assert_eq!(
            lookup_media_override(&map, Path::new("D:\\downloads\\")),
            Some(DiskType::Hdd)
        );
        // Sibling with a shared prefix is NOT covered.
        assert_eq!(
            lookup_media_override(&map, Path::new("D:\\downloads-old\\movie.mkv")),
            None
        );
        assert_eq!(lookup_media_override(&map, Path::new("D:\\other")), None);
    }

    #[test]
    fn nested_override_beats_its_parent() {
        let map = overrides(&[
            ("D:\\downloads", DiskType::Hdd),
            ("D:\\downloads\\nvme-cache", DiskType::Ssd),
        ]);

        assert_eq!(
            lookup_media_override(&map, Path::new("D:\\downloads\\nvme-cache\\chunk.bin")),
            Some(DiskType::Ssd)
        );
        assert_eq!(
            lookup_media_override(&map, Path::new("D:\\downloads\\other\\chunk.bin")),
            Some(DiskType::Hdd)
        );
    }

    #[test]
    fn empty_and_unusable_keys_never_match() {
        let map = overrides(&[("", DiskType::Hdd), ("   ", DiskType::Hdd)]);
        assert_eq!(lookup_media_override(&map, Path::new("/tmp/downloads")), None);
        assert_eq!(
            lookup_media_override(&HashMap::<String, DiskType>::default(), Path::new("C:\\a")),
            None
        );
    }

    #[test]
    fn snapshot_is_order_independent_and_reports_its_size() {
        let left = MediaOverrides::new(&overrides(&[
            ("D:\\a", DiskType::Hdd),
            ("D:\\b", DiskType::Ssd),
        ]));
        let right = MediaOverrides::new(&overrides(&[
            ("D:\\b", DiskType::Ssd),
            ("D:\\a", DiskType::Hdd),
        ]));
        assert_eq!(left, right);
        assert_eq!(left.len(), 2);
        assert!(!left.is_empty());
        assert!(MediaOverrides::default().is_empty());
    }

    #[test]
    fn usable_keys_require_an_absolute_location() {
        assert!(!is_usable_override_key(""));
        assert!(!is_usable_override_key("   "));
        assert!(!is_usable_override_key("relative/downloads"));
        #[cfg(windows)]
        {
            assert!(is_usable_override_key("D:\\downloads"));
            assert!(is_usable_override_key("D:\\"));
            // The spelling a user types for a share, without a trailing separator.
            assert!(is_usable_override_key(r"\\nas\downloads"));
            assert!(is_usable_override_key(r"\\wsl$\Ubuntu\home\me"));
            assert!(!is_usable_override_key("D:"));
        }
        #[cfg(unix)]
        assert!(is_usable_override_key("/mnt/downloads"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_spellings_of_one_directory_share_a_key() {
        let map = overrides(&[("D:/Downloads/", DiskType::Hdd)]);
        assert_eq!(
            lookup_media_override(&map, Path::new("d:\\downloads\\sub\\file.bin")),
            Some(DiskType::Hdd)
        );

        // A canonicalized path arrives with a verbatim prefix; it is the same
        // directory as the plain spelling.
        let map = overrides(&[(r"\\?\D:\downloads", DiskType::Hdd)]);
        assert_eq!(
            lookup_media_override(&map, Path::new("D:\\downloads\\file.bin")),
            Some(DiskType::Hdd)
        );

        let map = overrides(&[(r"\\ns\share", DiskType::Hdd)]);
        assert_eq!(
            lookup_media_override(&map, Path::new(r"\\?\UNC\ns\share\file.bin")),
            Some(DiskType::Hdd)
        );

        // UNC server/share names are case-insensitive.
        let map = overrides(&[(r"\\NAS\Downloads", DiskType::Hdd)]);
        assert_eq!(
            lookup_media_override(&map, Path::new(r"\\nas\downloads\file.bin")),
            Some(DiskType::Hdd)
        );
    }

    /// The mountinfo parser: optional fields shift the position of `fstype`, and
    /// mount points escape spaces as `\040`.
    #[test]
    fn mountinfo_parsing_handles_optional_fields_and_escapes() {
        // A root mount, an NFS mount with optional fields, a CIFS mount whose
        // mount point contains an escaped space, and one malformed line.
        let content = concat!(
            "30 25 0:26 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw\n",
            "36 30 0:31 / /mnt/nas rw,relatime shared:5 - nfs4 nas.local:/export rw,addr=10.0.0.5\n",
            "42 30 0:32 / /mnt/space\\040name rw,relatime shared:6 - cifs //nas/share rw,vers=3.1.1\n",
            "57 30 0:40 / /home rw,relatime shared:7 - ext4 /dev/nvme0n1p3 rw\n",
            "99 30 0:41 / /broken rw - \n",
            "not a mountinfo line\n",
        );

        assert_eq!(
            parse_mountinfo(content),
            vec![
                ("/".to_string(), "ext4".to_string()),
                ("/mnt/nas".to_string(), "nfs4".to_string()),
                ("/mnt/space name".to_string(), "cifs".to_string()),
                ("/home".to_string(), "ext4".to_string()),
            ],
            "the malformed lines are skipped, not mis-parsed"
        );

        // And the classifier agrees about which of them are remote.
        let network: Vec<String> = parse_mountinfo(content)
            .into_iter()
            .filter(|(_, fstype)| is_network_filesystem(fstype))
            .map(|(mount_point, _)| mount_point)
            .collect();
        assert_eq!(network, vec!["/mnt/nas".to_string(), "/mnt/space name".to_string()]);
    }

    #[test]
    fn mount_points_are_matched_on_component_boundaries() {
        assert!(path_is_within(Path::new("/mnt/nas/sub/f.bin"), Path::new("/mnt/nas")));
        assert!(path_is_within(Path::new("/mnt/nas"), Path::new("/mnt/nas")));
        // A sibling with a shared name prefix belongs to another mount.
        assert!(!path_is_within(Path::new("/mnt/nas2/f.bin"), Path::new("/mnt/nas")));
        // Every path is inside the root mount.
        assert!(path_is_within(Path::new("/anything"), Path::new("/")));
    }

    #[test]
    fn network_filesystems_are_classified_without_catching_local_ones() {
        for fstype in [
            "nfs", "nfs4", "cifs", "smb3", "9p", "virtiofs", "drvfs", "fuse.sshfs",
        ] {
            assert!(is_network_filesystem(fstype), "{fstype} should be remote");
        }
        // Case and padding must not change the answer.
        assert!(is_network_filesystem(" CIFS "));

        // Local filesystems — and a bare `fuse`, which covers ntfs-3g — stay
        // local so they keep being reported as SSD.
        for fstype in ["ext4", "xfs", "btrfs", "zfs", "apfs", "hfs", "ntfs", "exfat", "fuse", "tmpfs", "overlay"] {
            assert!(!is_network_filesystem(fstype), "{fstype} should not be remote");
        }
    }
}
