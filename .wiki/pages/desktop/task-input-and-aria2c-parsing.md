---
id: desktop/task-input-and-aria2c-parsing
title: Task Input Surfaces and aria2c Command Parsing
status: verified
generated_at: 2026-10-31T14:52:31Z
commit: b5102b9f00e4d40e84fa3ca5da17fc3e95e4c45b
evidence:
- path: crates/limedl-native/src/handlers/new_task/payload.rs
  lines:
  - 1
  - 31
  blob: bc1075ddf2471f3f069317f3e333a69f169642df
  snippet_sha1: 226211b144c4cecf69611a1c8c0f0d5b92a6ba30
- path: crates/limedl-native/src/main.rs
  lines:
  - 262
  - 272
  blob: de0b47270c1be4ea918f8af5fa13252cae4f38e5
  snippet_sha1: acdc267c8182a1523cf0c7fde934d71c0cefad82
- path: crates/limedl-native/src/platform_adapter.rs
  lines:
  - 12
  - 41
  blob: 32600c5c372b794f131fec32efc95acd57f81a04
  snippet_sha1: cd2c6638df1600dc0b2146142149ca3cbbd4af9e
- path: crates/limedl-native/src/platform_adapter.rs
  lines:
  - 162
  - 231
  blob: 32600c5c372b794f131fec32efc95acd57f81a04
  snippet_sha1: 4bd22a5c81dbb97ebcd2479d9657470a89458872
- path: crates/limedl-native/src/event_stream/pollers.rs
  lines:
  - 22
  - 111
  blob: 68207609a6d3d73695ae8157ff83bf0220a13cf8
  snippet_sha1: 3f5b3dc509868b01a159f82fd915196c0ab59f22
- path: crates/limedl-native/src/handlers/new_task/dialog.rs
  lines:
  - 46
  - 57
  blob: 7f98801fdafac328c9a802978a53eef86b40250b
  snippet_sha1: 33f3028d818f81bb33a5b3bbca36a0ff27517c4d
- path: crates/limedl-native/src/handlers/new_task/payload.rs
  lines:
  - 30
  - 40
  blob: bc1075ddf2471f3f069317f3e333a69f169642df
  snippet_sha1: 41ff47d67d5a0d6237f4bbaccaa1c5dfb44553d9
- path: crates/limedl-native/src/handlers/new_task/payload.rs
  lines:
  - 76
  - 91
  blob: bc1075ddf2471f3f069317f3e333a69f169642df
  snippet_sha1: c318d255bf23defbac875c8032f01e122cd5f642
- path: crates/limedl-native/src/handlers/new_task/payload.rs
  lines:
  - 42
  - 62
  blob: bc1075ddf2471f3f069317f3e333a69f169642df
  snippet_sha1: e454169e5a9a2913ec81bf4dbf368f897ffd39df
- path: crates/limedl-native/src/handlers/new_task/payload.rs
  lines:
  - 115
  - 148
  blob: bc1075ddf2471f3f069317f3e333a69f169642df
  snippet_sha1: 816cbae3d88ff4a218192c1e70ccc3bbd3098f9a
- path: crates/limedl-native/src/handlers/new_task/payload.rs
  lines:
  - 253
  - 297
  blob: bc1075ddf2471f3f069317f3e333a69f169642df
  snippet_sha1: 6e8f6c1be8b015c9d6f32f1abc96e78a494881d8
- path: crates/limedl-native/src/handlers/new_task/payload.rs
  lines:
  - 64
  - 67
  blob: bc1075ddf2471f3f069317f3e333a69f169642df
  snippet_sha1: 9f6f8bf04f036c7038517f1beadf8f499bfa425b
- path: crates/limedl-native/src/handlers/new_task/payload.rs
  lines:
  - 150
  - 181
  blob: bc1075ddf2471f3f069317f3e333a69f169642df
  snippet_sha1: fe735735e8587457bc18ea5f090c7bb67a6cdce8
- path: crates/limedl-native/src/handlers/new_task/payload.rs
  lines:
  - 69
  - 73
  blob: bc1075ddf2471f3f069317f3e333a69f169642df
  snippet_sha1: 7d2e6a8a790e6425267ce9fb2e7637d677c187e7
- path: crates/limedl-native/src/bridge/clipboard.rs
  lines:
  - 1
  - 53
  blob: 710a6cbf037dffd21d580f238d15611dd994a60c
  snippet_sha1: 626ace6e188f84889818069cf8396cbdc38d72d7
- path: crates/limedl-native/src/handlers/new_task/payload.rs
  lines:
  - 204
  - 238
  blob: bc1075ddf2471f3f069317f3e333a69f169642df
  snippet_sha1: 3c0a0ee4be3479cb7fc6bad67093439ed471f15f
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 1
  - 11
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 15a31217c844a4627a4112414acae1986ce25f8d
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 76
  - 82
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 609b6913989797f2d95db21cf5de035aca379f06
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 311
  - 315
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: afb480691045cede1ed399796bba3c8dace8aeee
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 84
  - 108
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 0b7d775fb7f34fb0e7d51c56e1e39435da9eeac8
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 217
  - 224
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 490fb9413d1a792685dc05a9e50db3feeab943f5
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 233
  - 250
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: d3a68de036e8c7dafed7a3f8be71235188ec53f2
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 252
  - 309
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 10a01eb3b616b7052a34ab43f6542cd7c8e8a3eb
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 317
  - 342
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 6bfb7eb413f6d8f1f49024621bd8a95c439afed4
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 55
  - 74
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 0af88c5180295828913712db680b7e93d667b1b4
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 132
  - 161
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 3a4407a30dcdc8c442f3b37b13838d60304e1803
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 192
  - 203
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 2f530d7b168f27ae341861978d8d2721ee356ab9
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 353
  - 371
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 4e0ed219d69fab8e7e896da156a64c1cb727495d
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 37
  - 53
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 65f8ff660000a7c2204da45d17a7dbb5ed0c252c
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 115
  - 130
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 26e10a2d9a2979134635ed649fdcb63f8abebf72
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 15
  - 35
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 6182e19c6497da1c0306b78f54c09e3fdd406b95
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 217
  - 231
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 153ad309370fbde6302ea34f15a32a9f508ddef2
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 205
  - 215
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: 871aedce845e929929d8f7d541300f150a25adb6
- path: crates/limedl-native/src/handlers/new_task/mod.rs
  lines:
  - 66
  - 97
  blob: 0fe8bee1011d960fba99b0a6a3e718e7629acb4e
  snippet_sha1: e62b95b653d23436360c0755e3b025dd13a3bb39
- path: crates/limedl-native/src/i18n/dialogs.rs
  lines:
  - 36
  - 64
  blob: bfcebc8e867b1f14d4747cba2bfb01f43655898e
  snippet_sha1: 7195578069663ae3f457a0a131211cdda2d186e0
- path: crates/limedl-native/src/handlers/new_task/mod.rs
  lines:
  - 66
  - 71
  blob: 0fe8bee1011d960fba99b0a6a3e718e7629acb4e
  snippet_sha1: 3a3731127ac02ff5f04676c67875e2e7f108fb51
- path: crates/limedl-native/src/handlers/new_task/submit.rs
  lines:
  - 99
  - 116
  blob: 7c732ad86ff55ab57d8629bb80b440aed4640ab0
  snippet_sha1: 1a31144ef173705cb72a2146dffa4949ab6d9791
- path: crates/limedl-native/src/handlers/new_task/mod.rs
  lines:
  - 84
  - 96
  blob: 0fe8bee1011d960fba99b0a6a3e718e7629acb4e
  snippet_sha1: f0b0fbaa4b2bd50e2011754f690807477fb00721
- path: crates/limedl-native/src/handlers/new_task/dialog.rs
  lines:
  - 46
  - 76
  blob: 7f98801fdafac328c9a802978a53eef86b40250b
  snippet_sha1: a0b1e6b9b6c510d44ddd5284933e67fb3781e17b
- path: crates/limedl-native/src/handlers/new_task/mod.rs
  lines:
  - 35
  - 49
  blob: 0fe8bee1011d960fba99b0a6a3e718e7629acb4e
  snippet_sha1: 4a31d72f8f93350240fc145da82aab725cbe18a5
- path: crates/limedl-native/src/ui_tests/new_task.rs
  lines:
  - 162
  - 197
  blob: 31cea15864fd8641d0a75d29298d714c235bce27
  snippet_sha1: 5d63f28832a7af0f00ace547b4ba20f06a8dda73
- path: crates/limedl-native/src/handlers/new_task/submit.rs
  lines:
  - 118
  - 146
  blob: 7c732ad86ff55ab57d8629bb80b440aed4640ab0
  snippet_sha1: e31f7b4ddae37392824d9e4c4ae5d2bfe5396c09
- path: crates/limedl-native/src/handlers/new_task/submit.rs
  lines:
  - 410
  - 418
  blob: 7c732ad86ff55ab57d8629bb80b440aed4640ab0
  snippet_sha1: f4a2e9856f1b25b524597e2da123b4c5c90bc376
- path: crates/limedl-native/src/handlers/new_task/mod.rs
  lines:
  - 99
  - 109
  blob: 0fe8bee1011d960fba99b0a6a3e718e7629acb4e
  snippet_sha1: 7e8f5f84b165e9fb36cb16169b7a08ca8f33db32
- path: crates/limedl-native/src/bridge/clipboard.rs
  lines:
  - 9
  - 53
  blob: 710a6cbf037dffd21d580f238d15611dd994a60c
  snippet_sha1: ec9484d21999bcd2018f7574d39ae1d369b2aee9
- path: crates/limedl-native/src/url_utils.rs
  lines:
  - 33
  - 55
  blob: 756a5c7ce35737b9e798a7dbec53d506bce47bd7
  snippet_sha1: 5c7eb8f762aaa271f926502eaa1f7095bbf87a81
- path: crates/limedl-native/src/url_utils.rs
  lines:
  - 57
  - 95
  blob: 756a5c7ce35737b9e798a7dbec53d506bce47bd7
  snippet_sha1: a040655fce0b704bec496b5090f83410daa31384
- path: crates/limedl-native/src/url_utils.rs
  lines:
  - 57
  - 83
  blob: 756a5c7ce35737b9e798a7dbec53d506bce47bd7
  snippet_sha1: 1b50b5c2dd96fa37e87bd5a33257f56c13464478
- path: crates/limedl-native/src/handlers/new_task/submit.rs
  lines:
  - 327
  - 408
  blob: 7c732ad86ff55ab57d8629bb80b440aed4640ab0
  snippet_sha1: 01c914f4beb593200cea211c6808f10263c94b71
- path: crates/limedl-native/src/url_utils.rs
  lines:
  - 141
  - 158
  blob: 756a5c7ce35737b9e798a7dbec53d506bce47bd7
  snippet_sha1: e4fa8fc3803ea7e1e36181c44451cfc832ae1451
- path: crates/limedl-native/src/url_utils.rs
  lines:
  - 97
  - 139
  blob: 756a5c7ce35737b9e798a7dbec53d506bce47bd7
  snippet_sha1: 242bf78a3d7e1c8622f901c0193d62b639b6cc06
- path: crates/limedl-native/ui/components/common.slint
  lines:
  - 406
  - 432
  blob: 6a5a3f8e109d0a08ba540ff62d0523e41a56d72c
  snippet_sha1: 1fa23afe8806b36cf557ec1d9c4585947b541c69
- path: crates/limedl-native/ui/components/new_task_dialog.slint
  lines:
  - 124
  - 127
  blob: ce885d5bd255c246512660c612067cac2981e4be
  snippet_sha1: 5b2cdf438f18dee8e278463091691d314c13d6b2
- path: crates/limedl-native/src/ui_tests/new_task.rs
  lines:
  - 199
  - 257
  blob: 31cea15864fd8641d0a75d29298d714c235bce27
  snippet_sha1: 8fbbbd8d9a13545ef73db36580d8e30571ea43ea
- path: crates/limedl-native/src/aria2_cli.rs
  lines:
  - 373
  - 414
  blob: 33f7b198e9f9c4b8aca1dab1cce7a516181ac51c
  snippet_sha1: f35e1210b93248a5ae595b66ea809db3cab16ee0
- path: crates/limedl-native/src/url_utils.rs
  lines:
  - 181
  - 296
  blob: 756a5c7ce35737b9e798a7dbec53d506bce47bd7
  snippet_sha1: aadecea119b1f18d8956eb1a4a3b90560e8262ec
- path: crates/limedl-native/src/ui_tests/new_task.rs
  lines:
  - 16
  - 103
  blob: 31cea15864fd8641d0a75d29298d714c235bce27
  snippet_sha1: cee7fd400a4b23abc278d98aa2b78d0c1af66dac
- path: crates/limedl-native/src/ui_tests/async_contracts/new_task.rs
  lines:
  - 1
  - 16
  blob: 26bdd219e51b0d768c3f993ef9079054897621e9
  snippet_sha1: d29924588fad5501ef1844d1ae6025820db7dba1
type: desktop
---
# Task Input Surfaces and aria2c Command Parsing

The desktop client accepts a download from far more places than the New Task
dialog's URL box, and users copy whole `aria2c` command lines out of browser
extensions and forum posts rather than bare links. One entry point absorbs all
of that — `handlers::new_task::open_new_task_with_payload` — and classifies the
payload by its *content* rather than by the surface it arrived on.
[[cite:crates/limedl-native/src/handlers/new_task/payload.rs:L1-L31]]

## One entry point, several producers

| Producer | Path into the dialog |
| --- | --- |
| Cold start CLI argument | `run()` passes the positional argument after the listeners are up |
| Second instance | the primary's activation listener receives the payload the secondary sent over the instance socket (`WM_COPYDATA` on Windows when it can find the HWND), shows the window and opens the dialog |
| Windows `WM_DROPFILES` drag-and-drop | `platform_adapter` forwards dropped files; a sole `.torrent`/`.metalink`/`.meta4` among several files wins, otherwise the list becomes a batch |
| Clipboard monitor | a 1500 ms poll toasts the detected link; it never starts a download by itself |
| The dialog itself | the `url_changed` callback on the URL field, so typing/pasting into the box goes through the same parser as an external payload |

[[cite:crates/limedl-native/src/main.rs:L262-L272]]
[[cite:crates/limedl-native/src/platform_adapter.rs:L12-L41]]
[[cite:crates/limedl-native/src/platform_adapter.rs:L162-L231]]
[[cite:crates/limedl-native/src/event_stream/pollers.rs:L22-L111]]
[[cite:crates/limedl-native/src/handlers/new_task/dialog.rs:L46-L57]]

`open_new_task_with_payload(raw_payload)` trims the payload (including stray
surrounding quotes) and then decides, in order:

1. `limedl://download?url=…` is percent-decoded; `limedl://magnet:…` has its
   prefix stripped back to `magnet:…`; any other `limedl://` path is used as-is.
   [[cite:crates/limedl-native/src/handlers/new_task/payload.rs:L30-L40]]
   [[cite:crates/limedl-native/src/handlers/new_task/payload.rs:L76-L91]]
2. A local path (or `file://` URL) whose extension is `.torrent` opens the
   torrent preview; `.metalink`/`.meta4` is parsed and its first file's best
   mirror is pre-filled with the file name.
   [[cite:crates/limedl-native/src/handlers/new_task/payload.rs:L42-L62]]
   [[cite:crates/limedl-native/src/handlers/new_task/payload.rs:L115-L148]]
   [[cite:crates/limedl-native/src/handlers/new_task/payload.rs:L253-L297]]
3. `magnet:?` fills the URL field and uses the `dn=` parameter
   (percent-decoded) as the file name when present.
   [[cite:crates/limedl-native/src/handlers/new_task/payload.rs:L64-L67]]
   [[cite:crates/limedl-native/src/handlers/new_task/payload.rs:L150-L181]]
4. Everything else goes through the clipboard classifier
   (`bridge::parse_clipboard_download_text`), which answers `SingleUrl`,
   `BatchUrls` or `Empty`.
   [[cite:crates/limedl-native/src/handlers/new_task/payload.rs:L69-L73]]
   [[cite:crates/limedl-native/src/bridge/clipboard.rs:L1-L53]]
5. A `SingleUrl` is offered to the aria2c parser **first**: if it is a command
   line, the visible fields are filled from it instead of leaving unparseable
   text in the URL box. HTTP(S) links additionally get an automatic checksum
   probe. [[cite:crates/limedl-native/src/handlers/new_task/payload.rs:L204-L238]]

## The aria2c tokenizer

`aria2_cli` parses exactly one command; batch handling (one command per line)
is `url_utils`'s job. [[cite:crates/limedl-native/src/aria2_cli.rs:L1-L11]]
Two cheap gates keep the parser from hijacking the URL field:

- `looks_like_aria2_command` normalizes the first token's basename
  (`aria2c`/`aria2c.exe`, path and quotes stripped) — that is what the clipboard
  classifier and the batch parser test before spending a full parse.
  [[cite:crates/limedl-native/src/aria2_cli.rs:L76-L82]]
  [[cite:crates/limedl-native/src/aria2_cli.rs:L311-L315]]
- `parse` returns `None` when the command names no download target, so a stray
  `aria2c --help` on the clipboard is not treated as a download.
  [[cite:crates/limedl-native/src/aria2_cli.rs:L84-L108]]
  [[cite:crates/limedl-native/src/aria2_cli.rs:L217-L224]]

`join_continuations` folds `^` and `\` line continuations into one logical line
before `tokenize` runs. [[cite:crates/limedl-native/src/aria2_cli.rs:L233-L250]]
The tokenizer is quote-aware in a deliberately platform-friendly way: single
and double quotes group a token; backslashes stay **literal** outside quotes,
because a Windows path such as `C:\Downloads` must survive; inside double
quotes, `\` escapes only `"` and `\`. [[cite:crates/limedl-native/src/aria2_cli.rs:L252-L309]]

`split_option` accepts both `--name=value` and the attached short form
(`-oNAME`, `-dNAME`, `-UNAME`, `-HNAME`), and `take_value` reads the value from
the inline form or the following token. [[cite:crates/limedl-native/src/aria2_cli.rs:L317-L342]]

## What is honored, and what is only reported

The parser produces an `Aria2Command` of the fields limedl can actually honor
(`--out`/`-o`, `--dir`/`-d`, repeatable `--header`/`-H`/`--http-header`,
`--user-agent`/`-U`, `-Z`/`--force-sequential`); `--referer`/`--referrer`/
`--http-referer` becomes a synthesized `Referer: …` entry in the same header
list. [[cite:crates/limedl-native/src/aria2_cli.rs:L55-L74]]
[[cite:crates/limedl-native/src/aria2_cli.rs:L132-L161]]
`--checksum=TYPE=DIGEST` normalizes to `(ChecksumMode, digest)` with the digest
lowercased; an algorithm limedl cannot verify (`sha1`, `md5`, …) is recorded as
an ignored option, while a *malformed* digest is dropped silently.
[[cite:crates/limedl-native/src/aria2_cli.rs:L192-L203]]
[[cite:crates/limedl-native/src/aria2_cli.rs:L353-L371]]

Two other option classes exist so that parsing stays honest:

- **Proxy options** (`--all-proxy`, `--http-proxy`, `--proxy`,
  `--*-proxy-user`/`-passwd`) are consumed but ignored. They carry URLs, and
  without consuming their value a proxy URI would be mistaken for a download
  target. [[cite:crates/limedl-native/src/aria2_cli.rs:L37-L53]]
  [[cite:crates/limedl-native/src/aria2_cli.rs:L115-L130]]
- **Materially unsupported options** (`--input-file`/`-i`, `--load-cookies`,
  `--save-cookies`, `--metalink-file`/`-M`, `--torrent-file`/`-T`,
  `--http-user`, `--http-passwd`, the FTP user/password pair, `--no-proxy`,
  `--no-check-certificate`) are recorded in `unsupported` with a canonical
  label, so an alias is reported as its long form and the list can be
  deduplicated. [[cite:crates/limedl-native/src/aria2_cli.rs:L15-L35]]
  [[cite:crates/limedl-native/src/aria2_cli.rs:L217-L231]]

Anything else is a known-nothing option: its value is consumed only when the
next token is neither another option nor a URL. That is what lets a valueless
flag such as `--continue` sit directly before the target without swallowing it.
[[cite:crates/limedl-native/src/aria2_cli.rs:L205-L215]]

## From parsed command to dialog state

`apply_aria2_command` fills the file name, directory, headers and checksum
fields and then builds the dialog's `aria2_hint` from three numbers: the link
count, the header count, and the ignored-option list (joined and localized in
`i18n::format_aria2_hint`). The hint is the contract that nothing is dropped
silently. [[cite:crates/limedl-native/src/handlers/new_task/mod.rs:L66-L97]]
[[cite:crates/limedl-native/src/i18n/dialogs.rs:L36-L64]]

Two asymmetries are deliberate:

- The URL field keeps the **raw command text**. That is what the user pasted
  and can keep editing, so the target and its mirrors are re-derived from it at
  submit time rather than stored in a hidden parallel field.
  [[cite:crates/limedl-native/src/handlers/new_task/mod.rs:L66-L71]]
  [[cite:crates/limedl-native/src/handlers/new_task/submit.rs:L99-L116]]
- A single-mode dialog can start exactly one download, so several targets
  become **mirrors** of that one download. `-Z` asked for separate downloads,
  which this field cannot express, so it is appended to the ignored list
  instead of silently changing the meaning.
  [[cite:crates/limedl-native/src/handlers/new_task/mod.rs:L84-L96]]

When the text stops being an aria2c command, the `url_changed` handler clears
the headers and hint (and every open/reset path runs `reset_transient_state`) so
a `Cookie:` from the previous paste cannot leak into the next target; a probe
hash is dropped by the companion `reset_new_task_probe` callback the dialog
invokes on every URL edit.
[[cite:crates/limedl-native/src/handlers/new_task/dialog.rs:L46-L76]]
[[cite:crates/limedl-native/src/handlers/new_task/mod.rs:L35-L49]]
[[cite:crates/limedl-native/src/ui_tests/new_task.rs:L162-L197]]

At submit time `submit_single` re-parses the URL field and takes the first
target as the download and the rest as `mirror_urls`, plus the parsed user agent
and checksum. Checksum precedence is: the manual field (what the user sees and
can edit, validated by `parse_user_checksum`, which also accepts GNU/BSD formats
and `algo:` prefixes) → the parsed `--checksum` → a detected probe hash.
[[cite:crates/limedl-native/src/handlers/new_task/submit.rs:L118-L146]]
[[cite:crates/limedl-native/src/handlers/new_task/submit.rs:L410-L418]]
The parsed digest is written into the field with an explicit `algo:` prefix so a
64-hex BLAKE3 is not read back as a SHA-256.
[[cite:crates/limedl-native/src/handlers/new_task/mod.rs:L99-L109]]

## Batch mode and the expansion rules

The clipboard classifier decides single versus batch: it walks the text line by
line, keeps HTTP(S)/`magnet:` links (and bracketed range URLs), and treats an
`aria2c` command as one entry that is kept **verbatim** — surrounding double
quotes are not stripped, because trimming them would cut the closing quote off
the last option. [[cite:crates/limedl-native/src/bridge/clipboard.rs:L9-L53]]

The batch textarea is parsed by `parse_batch_entries`: blank lines and `#`
comments are skipped, and each remaining line is either a full `aria2c` command
or one or more plain URLs. `entries_from_aria2` then applies the two rules that
decide how many tasks a line becomes:
[[cite:crates/limedl-native/src/url_utils.rs:L33-L55]]

- **Separate downloads** when `-Z`/`--force-sequential` was given, or when
  range expansion grew the URL count (a `[01-20]` range means the extra targets
  are siblings, not mirrors) — mirror lists stay empty.
- **One download with mirrors** otherwise: `aria2c url1 url2` is a single
  transfer that may fall back to the second URL.

[[cite:crates/limedl-native/src/url_utils.rs:L57-L95]]

`--out` is only kept when the command yields exactly one task, since two tasks
cannot share a file name; each entry also carries its own `--dir`, headers, user
agent and checksum. [[cite:crates/limedl-native/src/url_utils.rs:L57-L83]]
At submit, per-entry `--out` wins over the file name derived from the URL (the
last percent-decoded path segment of an HTTP(S) URL), and the per-entry `--dir`
wins over the dialog's directory. Entries are started concurrently (one spawned
task each) and the flow reports a done toast with the success count.
[[cite:crates/limedl-native/src/handlers/new_task/submit.rs:L327-L408]]
[[cite:crates/limedl-native/src/url_utils.rs:L141-L158]]

`expand_url_ranges` expands only the **first** `[start-end]` occurrence, pads to
the width of the start token, refuses to expand when the range is malformed or
descending, and caps a range at 1000 entries so a mistyped range cannot queue
thousands of tasks. A URL without a valid range is returned unchanged.
[[cite:crates/limedl-native/src/url_utils.rs:L97-L139]]

## Clipboard monitoring

`start_clipboard_monitor` is a background poller, not an auto-downloader: every
1500 ms it reads the clipboard, skips empty or unchanged text, and classifies it
with the same `parse_clipboard_download_text` used by the payload path (it
polls only while the window is visible). A single link produces a toast naming
the target — for an `aria2c` command, the toast shows its first target rather
than the whole command line, truncated char-aware (never byte-sliced, which
could split a UTF-8 sequence) — and a batch produces a count toast. Turning that
toast into tasks is always a user action.
[[cite:crates/limedl-native/src/event_stream/pollers.rs:L22-L111]]

## Long inputs must not break the dialog

Loading a 500-character URL (or a long `aria2c` command with a long header) into
the dialog must not widen the modal or produce a horizontal scrollbar. The fix
is two-sided:

- `CustomTextInput` can shrink: it sets `horizontal-stretch: 1; min-width: 0px;`
  on itself and on the inner layout holding the text, so a long value is clipped
  by the input rather than by the window.
  [[cite:crates/limedl-native/ui/components/common.slint:L406-L432]]
- The dialog's `ScrollView` pins `content-width: self.visible-width`, so the
  scrolling body only ever scrolls vertically.
  [[cite:crates/limedl-native/ui/components/new_task_dialog.slint:L124-L127]]

The in-process UI test pins this by asserting the modal, URL input, probe button
and submit button all stay inside the window and that no horizontal scrollbar
exists — for both a long URL and a long aria2c command.
[[cite:crates/limedl-native/src/ui_tests/new_task.rs:L199-L257]]

## Tests that pin the contract

- `aria2_cli`'s unit tests cover the tokenizer (quotes, continuations, Windows
  paths), the honored options, the ignored-option labels and the checksum spec.
  [[cite:crates/limedl-native/src/aria2_cli.rs:L373-L414]]
- `url_utils`'s unit tests cover batch parsing (comments, mixed lines), the
  mirrors-versus-sequential decision and range expansion.
  [[cite:crates/limedl-native/src/url_utils.rs:L181-L296]]
- `src/ui_tests/new_task.rs` drives the real dialog: reopening resets batch mode
  and the probe, editing the URL drops a checksum that belonged to the old one,
  the batch counter follows the parser, an `aria2c` command fills the visible
  fields while a plain URL clears them, and the long-input layout test above.
  [[cite:crates/limedl-native/src/ui_tests/new_task.rs:L16-L103]]
- `src/ui_tests/async_contracts/new_task.rs` covers the event-loop side of the
  payload entry point (see [[wiki:testing/slint-ui-testing]]).
  [[cite:crates/limedl-native/src/ui_tests/async_contracts/new_task.rs:L1-L16]]

Once a request leaves the dialog it is an ordinary `StartDownloadRequest`; how
the engine turns it into a running transfer is
[[wiki:workflows/http-download-lifecycle]]'s subject, and
magnet/metalink handling belongs to
[[wiki:workflows/metalink-and-mirror-selection]].

Related pages: [[wiki:desktop/native-ui-architecture]],
[[wiki:workflows/http-download-lifecycle]],
[[wiki:workflows/metalink-and-mirror-selection]],
[[wiki:testing/slint-ui-testing]],
[[wiki:integrations/aria2-rpc-server]].
