---
type: desktop
title: Task Input Surfaces and aria2c Command Parsing
description: How a user-supplied payload becomes downloads in the limedl desktop client — CLI arguments, limedl:// deep links and second-instance forwarding, clipboard sniffing, dropped files, .txt batch lists and pasted aria2c command lines; the quote-aware tokenizer, the honored and reported-unsupported options, the batch and range expansion rules, and the long-input layout contract.
tags: [desktop, input, parsing, aria2c, clipboard, batch, dialog]
verified:
  - by: openwiki/0.7.2
    at: 2026-10-14T09:19:57.024Z
sources:
  - id: openwiki-source-24d873252a3093670382cb6f
    resource: repo://crates/limedl-native/src/aria2_cli.rs
  - id: openwiki-source-e665feba36c1b1a17291fe32
    resource: repo://crates/limedl-native/src/bridge/clipboard.rs
  - id: openwiki-source-2d2319d07fd88af3e53e3559
    resource: repo://crates/limedl-native/src/event_stream/pollers.rs
  - id: openwiki-source-94c8e8580703ae88e2e93136
    resource: repo://crates/limedl-native/src/handlers/new_task/mod.rs
  - id: openwiki-source-e1dd5dbffb6fd66d41330268
    resource: repo://crates/limedl-native/src/handlers/new_task/payload.rs
  - id: openwiki-source-ba0e80cda1d9e9fb8f06213c
    resource: repo://crates/limedl-native/src/handlers/new_task/submit.rs
  - id: openwiki-source-032e0308bed81475b875de22
    resource: repo://crates/limedl-native/src/i18n/dialogs.rs
  - id: openwiki-source-fb048d5fb7a13a8f9b8fad76
    resource: repo://crates/limedl-native/src/main.rs
  - id: openwiki-source-c57d01ba5535c30c9c372218
    resource: repo://crates/limedl-native/src/platform_adapter.rs
  - id: openwiki-source-bd701500362548a90e23770f
    resource: repo://crates/limedl-native/src/ui_tests/new_task.rs
  - id: openwiki-source-8797d3bd1c23c4a2149dd142
    resource: repo://crates/limedl-native/src/url_utils.rs
  - id: openwiki-source-724a8afc93ff75d26b0451ef
    resource: repo://crates/limedl-native/ui/components/common.slint
  - id: openwiki-source-cac343095874dbb881b02533
    resource: repo://crates/limedl-native/ui/components/new_task_dialog.slint
generated: { by: "pi", at: "2026-10-14T09:19:57.024Z" }
---

# Task Input Surfaces and aria2c Command Parsing

The desktop client accepts a download from far more places than the New Task
dialog's URL box, and users copy whole `aria2c` command lines out of browser
extensions and forum posts rather than bare links. One entry point absorbs all of
that — `handlers::new_task::open_new_task_with_payload` — and classifies the
payload by its *content* rather than by the surface it arrived on.

## One entry point, several producers

| Producer | Path into the dialog |
| --- | --- |
| Cold start CLI argument | `run()` passes the positional argument after logging and bootstrap |
| Second instance | the primary's activation listener receives the payload the secondary sent over the instance socket (`WM_COPYDATA` on Windows when it can find the HWND), shows the window and opens the dialog |
| macOS `open-file` / `open-url`, Windows `WM_DROPFILES` | `platform_adapter` forwards dropped files; `.torrent`/`.metalink`/`.meta4` among several files wins, otherwise the list becomes a batch |
| Clipboard monitor | a 1500 ms poll toasts the detected link; it never starts a download by itself |
| The dialog itself | `url_changed` on the URL field, so typing/pasting into the box goes through the same parser as an external payload |

`open_new_task_with_payload(raw_payload)` trims the payload (including stray
surrounding quotes) and then decides, in order:

1. `limedl://download?url=…` is percent-decoded; `limedl://magnet:…` has its
   prefix stripped back to `magnet:…`; any other `limedl://` path is used as-is.
2. A local path (or `file://` URL) whose extension is `.torrent` opens the torrent
   preview; `.metalink`/`.meta4` is parsed and its first file's best mirror is
   pre-filled with the file name.
3. `magnet:?` fills the URL field and uses the `dn=` parameter (percent-decoded)
   as the file name when present.
4. Everything else goes through the clipboard classifier
   (`bridge::parse_clipboard_download_text`), which answers `SingleUrl`,
   `BatchUrls` or `Empty`.
5. A `SingleUrl` is offered to the aria2c parser **first**: if it is a command
   line, the visible fields are filled from it instead of leaving unparseable
   text in the URL box. HTTP(S) links additionally get an automatic checksum
   probe.

Evidence: `repo://crates/limedl-native/src/handlers/new_task/payload.rs#L1-L2`,
`repo://crates/limedl-native/src/handlers/new_task/payload.rs#L21-L45`,
`repo://crates/limedl-native/src/handlers/new_task/payload.rs#L52-L140`,
`repo://crates/limedl-native/src/handlers/new_task/payload.rs#L145-L200`,
`repo://crates/limedl-native/src/main.rs#L252-L262`,
`repo://crates/limedl-native/src/platform_adapter.rs#L1-L40`,
`repo://crates/limedl-native/src/platform_adapter.rs#L155-L200`.

## The aria2c tokenizer

`aria2_cli` parses exactly one command; batch handling (one command per line) is
`url_utils`'s job. Two cheap gates keep the parser from hijacking the URL field:

- `looks_like_aria2_command` normalizes the first token's basename
  (`aria2c`/`aria2c.exe`, path and quotes stripped) — that is what the clipboard
  classifier and the batch parser test before spending a full parse.
- `parse` returns `None` when the command names no download target, so a stray
  `aria2c --help` on the clipboard is not treated as a download.

`join_continuations` folds `^` and `\` line continuations into one logical line
before `tokenize` runs. The tokenizer is quote-aware in a deliberately
platform-friendly way:

- single and double quotes group a token;
- backslashes stay **literal** outside quotes, because a Windows path such as
  `C:\Downloads` must survive;
- inside double quotes, `\` escapes only `"` and `\`.

`split_option` accepts both `--name=value` and the attached short form
(`-oNAME`, `-dNAME`, `-UNAME`, `-HNAME`), and `take_value` reads the value from
the inline form or the following token.

Evidence: `repo://crates/limedl-native/src/aria2_cli.rs#L1-L16`,
`repo://crates/limedl-native/src/aria2_cli.rs#L77-L100`,
`repo://crates/limedl-native/src/aria2_cli.rs#L189-L264`,
`repo://crates/limedl-native/src/aria2_cli.rs#L266-L288`.

## What is honored, and what is only reported

The parser produces an `Aria2Command` of the fields limedl can actually honor:

- `--out` / `-o` → file name
- `--dir` / `-d` → destination directory
- `--header` / `-H` / `--http-header` (repeatable) → request headers
- `--referer` / `--referrer` / `--http-referer` → a synthesized `Referer: …`
  header in the same list
- `--user-agent` / `-U` → user agent
- `--checksum=TYPE=DIGEST` → `(ChecksumMode, digest)` with the digest lowercased;
  an algorithm limedl cannot verify (`sha1`, `md5`, …) is recorded as an ignored
  option, while a *malformed* digest is dropped silently
- `-Z` / `--force-sequential`

Two other option classes exist so that parsing stays honest:

- **Proxy options** (`--all-proxy`, `--http-proxy`, `--proxy`,
  `--*-proxy-user`/`-passwd`) are consumed but ignored. They carry URLs, and
  without consuming their value a proxy URI would be mistaken for a download
  target.
- **Materially unsupported options** (`--input-file`/`-i`, `--load-cookies`,
  `--save-cookies`, `--metalink-file`/`-M`, `--torrent-file`/`-T`,
  `--http-user`, `--http-passwd`, the FTP user/password pair, `--no-proxy`,
  `--no-check-certificate`) are recorded in `unsupported` with a canonical label,
  so an alias is reported as its long form and the list can be deduplicated.

Anything else is a known-nothing option: its value is consumed only when the next
token is neither another option nor a URL. That is what lets a valueless flag
such as `--continue` sit directly before the target without swallowing it.

Evidence: `repo://crates/limedl-native/src/aria2_cli.rs#L18-L56`,
`repo://crates/limedl-native/src/aria2_cli.rs#L88-L180`,
`repo://crates/limedl-native/src/aria2_cli.rs#L299-L326`.

## From parsed command to dialog state

`apply_aria2_command` fills the file name, directory, headers and checksum fields
and then builds the dialog's `aria2_hint` from three numbers: the link count, the
header count, and the ignored-option list (joined and localized in
`i18n::format_aria2_hint`). The hint is the contract that nothing is dropped
silently.

Two asymmetries are deliberate:

- The URL field keeps the **raw command text**. That is what the user pasted and
  can keep editing, so the target and its mirrors are re-derived from it at
  submit time rather than stored in a hidden parallel field.
- A single-mode dialog can start exactly one download, so several targets become
  **mirrors** of that one download. `-Z` asked for separate downloads, which this
  field cannot express, so it is appended to the ignored list instead of
  silently changing the meaning.

When the text stops being an aria2c command, `url_changed` clears the transient
state (headers, hint, probe result) so a `Cookie:` from the previous paste cannot
leak into the next target.

Evidence: `repo://crates/limedl-native/src/handlers/new_task/mod.rs#L35-L52`,
`repo://crates/limedl-native/src/handlers/new_task/mod.rs#L56-L100`,
`repo://crates/limedl-native/src/i18n/dialogs.rs#L70-L96`,
`repo://crates/limedl-native/src/ui_tests/new_task.rs#L165-L202`.

At submit time `submit_single` re-parses the URL field and takes the first target
as the download and the rest as `mirror_urls`, plus the parsed user agent and
checksum. Checksum precedence is: the manual field (what the user sees and can
edit, validated by `parse_user_checksum`, which also accepts GNU/BSD formats and
`algo:` prefixes) → the parsed `--checksum` → a detected probe hash. The field
value is written with an explicit `algo:` prefix so a 64-hex BLAKE3 is not read
back as a SHA-256.

Evidence: `repo://crates/limedl-native/src/handlers/new_task/submit.rs#L79-L140`,
`repo://crates/limedl-native/src/handlers/new_task/submit.rs#L413-L470`.

## Batch mode and the expansion rules

The clipboard classifier decides single versus batch: it walks the text line by
line, keeps HTTP(S)/`magnet:` links (and bracketed range URLs), and treats an
`aria2c` command as one entry that is kept **verbatim** — surrounding double
quotes are not stripped, because trimming them would cut the closing quote off
the last option.

The batch textarea is parsed by `parse_batch_entries`: blank lines and `#`
comments are skipped, and each remaining line is either a full `aria2c` command
or one or more plain URLs. `entries_from_aria2` then applies the two rules that
decide how many tasks a line becomes:

- **Separate downloads** when `-Z`/`--force-sequential` was given, or when range
  expansion grew the URL count (a `[01-20]` range means the extra targets are
  siblings, not mirrors) — mirror lists stay empty.
- **One download with mirrors** otherwise: `aria2c url1 url2` is a single
  transfer that may fall back to the second URL.

``--out`` is only kept when the command yields exactly one task, since two tasks
cannot share a file name; each entry also carries its own `--dir`, headers, user
agent and checksum. At submit, per-entry `--out` wins over the file name derived
from the URL (the last percent-decoded path segment of an HTTP(S) URL), and the
per-entry `--dir` wins over the dialog's directory. Entries are started
concurrently (one spawned task each) and the flow reports a done toast with the
success count.

`expand_url_ranges` expands only the **first** `[start-end]` occurrence, pads to
the width of the start token, refuses to expand when the range is malformed or
descending, and caps a range at 1000 entries so a mistyped range cannot queue
thousands of tasks. A URL without a valid range is returned unchanged.

Evidence: `repo://crates/limedl-native/src/bridge/clipboard.rs#L1-L52`,
`repo://crates/limedl-native/src/url_utils.rs#L1-L61`,
`repo://crates/limedl-native/src/url_utils.rs#L63-L99`,
`repo://crates/limedl-native/src/url_utils.rs#L100-L160`,
`repo://crates/limedl-native/src/handlers/new_task/submit.rs#L294-L380`,
`repo://crates/limedl-native/src/ui_tests/new_task.rs#L76-L104`.

## Clipboard monitoring

`start_clipboard_monitor` is a background poller, not an auto-downloader: every
1500 ms it reads the clipboard, skips empty or unchanged text, and classifies it
with the same `parse_clipboard_download_text` used by the payload path. A single
link produces a toast naming the target — for an `aria2c` command, the toast
shows its first target rather than the whole command line, truncated
char-aware (never byte-sliced, which could split a UTF-8 sequence) — and a batch
produces a count toast. Turning that toast into tasks is always a user action.

Evidence: `repo://crates/limedl-native/src/event_stream/pollers.rs#L21-L90`.

## Long inputs must not break the dialog

Loading a 500-character URL (or a long `aria2c` command with a long header) into
the dialog must not widen the modal or produce a horizontal scrollbar. The fix is
two-sided:

- `CustomTextInput` can shrink: it sets `horizontal-stretch: 1; min-width: 0px;`
  on itself and on the inner layout holding the text, so a long value is clipped
  by the input rather than by the window.
- The dialog's `ScrollView` pins `content-width: self.visible-width`, so the
  scrolling body only ever scrolls vertically.

The in-process UI test pins this by asserting the modal, URL input, probe button
and submit button all stay inside the window and that no horizontal scrollbar
exists — for both a long URL and a long aria2c command.

Evidence: `repo://crates/limedl-native/ui/components/common.slint#L410-L432`,
`repo://crates/limedl-native/ui/components/new_task_dialog.slint#L118-L132`,
`repo://crates/limedl-native/src/ui_tests/new_task.rs#L203-L258`.

## Tests that pin the contract

- `aria2_cli`'s unit tests cover the tokenizer (quotes, continuations, Windows
  paths), the honored options, the ignored-option labels and the checksum spec.
- `url_utils`'s unit tests cover batch parsing (comments, mixed lines), the
  mirrors-versus-sequential decision and range expansion.
- `src/ui_tests/new_task.rs` drives the real dialog: reopening resets batch mode
  and the probe, editing the URL drops a checksum that belonged to the old one,
  the batch counter follows the parser, an `aria2c` command fills the visible
  fields while a plain URL clears them, and the long-input layout test above.
- `src/ui_tests/async_contracts/new_task.rs` covers the event-loop side of the
  payload entry point (see [Slint UI Testing](../testing/slint-ui-testing.md)).

Once a request leaves the dialog it is an ordinary `StartDownloadRequest`; how
the engine turns it into a running transfer is
[HTTP Download Lifecycle](../workflows/http-download-lifecycle.md)'s subject, and
magnet/metalink handling belongs to
[Metalink and Mirror Selection](../workflows/metalink-and-mirror-selection.md).

Related pages: [Native Desktop UI (Slint)](native-ui-architecture.md),
[HTTP Download Lifecycle](../workflows/http-download-lifecycle.md),
[Metalink and Mirror Selection](../workflows/metalink-and-mirror-selection.md),
[Slint UI Testing](../testing/slint-ui-testing.md),
[Aria2 JSON-RPC Compatibility Server](../integrations/aria2-rpc-server.md).
