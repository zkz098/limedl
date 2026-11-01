---
title: Data Backup, Migration & Aria2 Import
description: limedl data directory layout, backup and restore, cross-machine migration, portable mode, and migrating from Aria2/Motrix with option mappings and limits
---

# Data Backup, Migration & Aria2 Import

Download tasks, chunk progress, torrent state, and settings live in the local data directory. Before upgrading, rolling back, or moving machines, know which files must be backed up together.

---

## Data directory layout

Default root:

| Platform | Path |
| :--- | :--- |
| Windows | `%LOCALAPPDATA%\limedl\` |
| macOS | `~/Library/Application Support/limedl/` |
| Linux | `$XDG_DATA_HOME/limedl/` or `~/.local/share/limedl/` |
| Override | `LIMEDL_DATA_DIR` has the highest priority |

Typical layout:

```text
<data root>/
├── settings.json              # Application settings
├── settings.json.bak          # Last successful save (best effort)
├── settings.json.corrupt      # Moved here when unparseable
├── downloads/
│   ├── downloads.db           # Tasks, chunk progress, BT metadata (SQLite)
│   ├── logs/
│   │   ├── limedl.log
│   │   └── crash.log
│   └── torrents/              # BT session/resume state
└── update/                    # Update staging, cleaned at startup
```

:::caution[Log location]
Logs live in `<data root>/downloads/logs/`, not in a top-level `logs/`. Older docs or forum posts may show a stale path.
:::

### How settings are written

- Saves write `settings.json.tmp` and atomically rename it to `settings.json`.
- Each successful save tries to write `settings.json.bak`.
- If `settings.json` cannot be parsed at startup, it is moved to `settings.json.corrupt` and `.bak` is tried.

---

## Backup and restore

### Backup

1. **Fully quit limedl** (including the tray), not just the main window;
2. Copy the entire data root, not only `settings.json`;
3. Do not run downloads during the backup, so SQLite is not mid-write.

Backing up only `settings.json` loses the task database, chunk progress, BT session, and logs.

### Restore

1. Quit the app;
2. Put the whole backup directory back in place;
3. If the machine or user name changed, update absolute paths in `settings.json` (`defaultDownloadDir`, directory media overrides, etc.);
4. Start the app and check the task list and download directory.

### Reset

Do not delete the old directory outright. Rename it to `limedl-old`, start fresh, and only delete it after confirming the new data works.

---

## Moving to another machine

- Most paths in the data directory are **absolute**; update them for the new machine.
- Completed files are not packaged automatically; copy the download directory separately.
- The SQLite database may record the old absolute paths. Cross-platform moves (for example Windows → Linux) are not guaranteed to resume tasks; re-add them if necessary.
- `LIMEDL_DATA_DIR` can point at an external drive for portable use, but prefer a stable absolute path: a shortcut that changes the working directory can make the data appear to vanish.

---

## Portable mode

- Point `LIMEDL_DATA_DIR` at a USB/external directory to carry settings, database, and BT state together.
- The download directory is still `defaultDownloadDir`; portable mode does not automatically put downloaded files inside `LIMEDL_DATA_DIR`.
- Quit the app before unplugging the drive, so SQLite is not interrupted.

---

## Migrating from Aria2 / Motrix

limedl **does not parse** `aria2.conf` or `aria2.session`, and has no one-click import. It is compatible with the Aria2 JSON-RPC 2.0 API, so migration has two steps:

1. Enable **Aria2 RPC** in limedl (off by default) and set a token and port;
2. Point AriaNg, Aria2 Explorer, Motrix, or another frontend at `http://127.0.0.1:6800/jsonrpc` and resubmit tasks.

### Common option mapping

| Aria2 option | limedl behavior |
| :--- | :--- |
| `dir` | Task download directory; must be absolute |
| `out` | Rename the output file |
| `pause` | Create the task paused |
| `select-file` | BT/Metalink file index (1-based) |
| `split` | Task thread count (clamped by limedl's limits) |
| `header` / `referer` / `user-agent` | Download request headers and UA |
| `checksum` | Maps to the checksum algorithm and expected value |
| `max-concurrent-downloads` | Traditional mode: parallel tasks; automatic mode: thread budget |
| `max-overall-download-limit` / `max-overall-upload-limit` | Global download/upload limits |
| `max-tries` | Maximum retries (capped) |
| `enable-dht` / `bt-max-peers` / `seed-ratio` / `seed-time` | Mapped to BT settings |

`max-concurrent-downloads`, `max-overall-*`, `max-tries`, and the BT keys are **global options** applied through `aria2.changeGlobalOption`; `dir`, `out`, `pause`, `select-file`, `split`, and `header` are task-level options. **Options that are not implemented one-to-one are ignored**, including some Aria2-specific scheduler/session options. Read the compatibility scope in [Aria2 RPC](/en/ecosystem/aria2-rpc/) before migrating.

### What cannot be imported directly

- Active tasks from `aria2.session` are not imported; re-add them through a frontend, or let the old Aria2 finish them.
- Aria2's DHT/torrent cache and `--save-session` history are not reused.
- Aria2's RPC secret and limedl's token model are similar but not identical; reconfigure the frontend.

---

## Related documentation

- [Aria2 RPC](/en/ecosystem/aria2-rpc/): methods, authentication, and client setup.
- [Self-Update Channels & Rollback](/en/advanced/self-update-channels/): backup considerations before updating.
- [Disk Media & Buffer Tuning](/en/advanced/disk-media-and-buffer-tuning/): overrides and path normalization.
- [FAQ](/en/faq): data directory, upgrades, and platform limits.
