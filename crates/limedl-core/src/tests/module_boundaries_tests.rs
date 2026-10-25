//! Module-boundary guard: the engine's top-level modules must form a DAG *and*
//! must respect the layering they will be split along.
//!
//! `limedl-core` is deliberately one crate, so nothing but this test stops a
//! module from reaching into a higher layer.  The failure mode is quiet: a
//! module gains an `use crate::higher_layer::…` and the dependency direction
//! silently inverts.  Splitting the crate later then turns that inversion into
//! a compile error — at the worst possible moment.
//!
//! This test parses `src/**/*.rs` (production code only: `tests/`, `tests.rs`
//! and `*_tests.rs` are skipped), builds the module dependency graph at
//! top-level-module granularity, and fails when:
//!
//! 1. a new cycle appears.  Cycles that exist today are listed in
//!    [`ALLOWED_CYCLES`] with the reason they are tolerated; removing one is a
//!    deliberate act that must also delete its entry.
//! 2. an edge points upward in [`LAYERS`] — the crate split's own contract.
//!
//! The intent is *direction*, not beauty: `a -> b -> c` is fine, `a <-> b` is
//! not, and `b -> a` is not either.  Improving the graph never fails this test;
//! only regressing it does.
//!
//! Two crate extractions have already happened, so those modules are no longer
//! in this graph and an upward edge to them is a compile error instead:
//! `limedl-types` (`types`, `error`, `event_bus`, `manifest`, `protocol`,
//! `settings`, `url_rewrite`, `checksum`, `slot_guard`, `platform`) and
//! `limedl-disk` (`file_ops`, `buffer_pool`, `io_scheduler`, `services`) and
//! `limedl-store` (`database`, `alt_svc`), `limedl-http` (`http`,
//! `http_client_factory`, `rate_limiter`), `limedl-bt` (`bt_backend`),
//! `limedl-engine` (executor, scheduler, CDN, metalink, task lifecycle),
//! `limedl-rpc` (`aria2_rpc`) and `limedl-testkit` (`test_harness`).

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Non-trivial strongly connected components that are tolerated today.
///
/// Empty: the engine's top-level modules form a DAG.  There is no escape hatch
/// left — a new cycle must be fixed (extract the shared piece into a lower-layer
/// module, as `file_ops` now does for the write-coalescing primitives and the
/// fault hooks, or invert the dependency), not whitelisted.
const ALLOWED_CYCLES: &[&[&str]] = &[];

/// The layering the crate split follows, bottom-up.
///
/// Each entry names the crate a group of top-level modules is meant to become.
/// A module may depend on a module in its own layer or any layer *below* it,
/// and on nothing above — which is exactly the relation Cargo enforces the day
/// those modules become separate crates.  Keeping the table honest here is what
/// makes that split a mechanical step instead of a re-design.
///
/// Two entries are intentionally coarse:
///
/// * `test_harness` sits with `limedl-store` only because both are pure
///   *consumers* of the disk layer; the crate it becomes (`limedl-testkit`) has
///   no production dependents at all.
/// * `bootstrap` gets its own top layer rather than joining `limedl-core`: it is
///   the one module allowed to touch every backend, and after the split it is
///   the only file in the facade crate that names `limedl-rpc`.
///
/// When a module moves to another crate, delete its name from here in the same
/// commit — the stale-entry check below fails otherwise.
const LAYERS: &[(&str, &[&str])] = &[
    (
        "limedl-core",
        &[
            "manager",
            "context",
            "dispatcher",
            "backend_registry",
            "logging",
            // Test fixtures that need the manager facade; see `src/testing.rs`.
            "testing",
        ],
    ),
    ("limedl-core (bootstrap only)", &["bootstrap"]),
];

/// Layer index and crate name of a top-level module, if it is in [`LAYERS`].
fn layer_of(module: &str) -> Option<(usize, &'static str)> {
    LAYERS
        .iter()
        .enumerate()
        .find_map(|(index, (crate_name, members))| {
            members.contains(&module).then_some((index, *crate_name))
        })
}

/// One source file together with the module path it defines.
struct SourceFile {
    module: Vec<String>,
    path: PathBuf,
}

fn collect_source_files(dir: &Path, module: &[String], out: &mut Vec<SourceFile>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // `tests/` inside `src/` holds integration-style test modules.
            if name == "tests" {
                continue;
            }
            let mut child = module.to_vec();
            child.push(name);
            collect_source_files(&path, &child, out);
            continue;
        }
        if path.extension().and_then(OsStr::to_str) != Some("rs") {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if is_test_file(&file_name) {
            continue;
        }
        let stem = file_name.trim_end_matches(".rs");
        let mut child = module.to_vec();
        if stem != "mod" && stem != "lib" {
            child.push(stem.to_string());
        }
        out.push(SourceFile {
            module: child,
            path,
        });
    }
}

fn is_test_file(file_name: &str) -> bool {
    file_name == "tests.rs" || file_name.ends_with("_tests.rs")
}

/// Skip a `//` line comment. `i` points at its first `/`.
fn skip_line_comment(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i] != '\n' {
        i += 1;
    }
    i
}

/// Skip a nesting-aware `/* … */` block comment. `i` points at its first `/`.
fn skip_block_comment(chars: &[char], mut i: usize) -> usize {
    let mut depth = 1;
    i += 2;
    while i < chars.len() && depth > 0 {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            depth += 1;
            i += 2;
        } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
            depth -= 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    i
}

/// Skip a string literal. `i` points at the opening `"`. Raw strings
/// (`r"…"`, `r#"..."#` and their `b`-prefixed forms) are recognised from the
/// character before it.
fn skip_string_literal(chars: &[char], mut i: usize) -> usize {
    if i > 0 && (chars[i - 1] == 'r' || chars[i - 1] == 'b') {
        i += 1;
        while i < chars.len() && chars[i] == '#' {
            i += 1;
        }
    }
    i += 1;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
        } else if chars[i] == '"' {
            return i + 1;
        } else {
            i += 1;
        }
    }
    i
}

/// Length of the char literal starting at `i` (the apostrophe), or `None` when
/// that apostrophe opens a lifetime — which must be kept, since a lifetime
/// cannot contain `use`.
fn char_literal_len(chars: &[char], i: usize) -> Option<usize> {
    // Same shape as the original inline check: a backslash means the closing
    // quote sits one character further out (and `Some('\\')` must not fall
    // through to the `Some(_)` arm).
    match chars.get(i + 1) {
        Some('\\') => (chars.get(i + 3) == Some(&'\'')).then_some(4),
        Some(_) => (chars.get(i + 2) == Some(&'\'')).then_some(3),
        None => None,
    }
}

/// Remove comments and string/char literals so a commented-out `use` or a URL
/// inside a string cannot be mistaken for code.
fn strip_noncode(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            i = skip_line_comment(&chars, i);
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i = skip_block_comment(&chars, i);
        } else if c == '"' {
            i = skip_string_literal(&chars, i);
        } else if let Some(len) = char_literal_len(&chars, i) {
            i += len;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Every `use …;` statement body in the file (the text between `use` and `;`).
fn use_statements(clean: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = clean.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Work on bytes: `i` may land inside a multi-byte character, and slicing
        // a `&str` there would panic (source files carry a UTF-8 BOM).
        let is_use = bytes[i..].starts_with(b"use")
            && (i == 0 || !is_ident_byte(bytes[i - 1]))
            && !bytes.get(i + 3).is_some_and(|b| is_ident_byte(*b));
        if !is_use {
            i += 1;
            continue;
        }
        // Scan to the terminating `;` at brace depth zero.  `use` and `;` are
        // ASCII, so `start`/`j` are always char boundaries.
        let start = i + 3;
        let mut j = start;
        let mut depth: i32 = 0;
        while j < bytes.len() {
            match bytes[j] {
                b'{' | b'(' | b'[' => depth += 1,
                b'}' | b')' | b']' => depth -= 1,
                b';' if depth <= 0 => break,
                _ => {}
            }
            j += 1;
        }
        out.push(clean[start..j].trim());
        i = j + 1;
    }
    out
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Split a comma-separated `use` tree at brace depth zero.
fn split_top(tree: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut start = 0;
    for (idx, c) in tree.char_indices() {
        match c {
            '{' | '(' | '[' | '<' => depth += 1,
            '}' | ')' | ']' | '>' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&tree[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(&tree[start..]);
    parts
}

/// Expand a `use` tree into the full token paths it imports, relative to the
/// `use` root (`crate`, `super`, `self`, or an external crate name).
fn expand_use_tree(tree: &str, base: &[String], out: &mut Vec<Vec<String>>) {
    for part in split_top(tree) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.starts_with('{') && part.ends_with('}') {
            expand_use_tree(&part[1..part.len() - 1], base, out);
            continue;
        }
        if let Some(open) = part.find('{') {
            let head: Vec<String> = part[..open]
                .trim()
                .trim_end_matches(':')
                .split("::")
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            let mut nested = base.to_vec();
            nested.extend(head);
            let inner = part[open..].trim();
            let inner = inner
                .strip_prefix('{')
                .unwrap_or(inner)
                .trim_end_matches('}');
            expand_use_tree(inner, &nested, out);
            continue;
        }
        let leaf = part.split(" as ").next().unwrap_or(part).trim();
        let mut tokens: Vec<String> = leaf
            .split("::")
            .filter(|s| !s.is_empty() && *s != "*")
            .map(str::to_string)
            .collect();
        if tokens.is_empty() {
            continue;
        }
        if tokens[0] == "self" && tokens.len() == 1 {
            out.push(base.to_vec());
            continue;
        }
        let mut full = base.to_vec();
        full.append(&mut tokens);
        out.push(full);
    }
}

/// Resolve a token path that starts with `crate`/`super`/`self` to an absolute
/// module path.  Anything else is an external crate and not an internal edge.
fn resolve_tokens(tokens: &[String], current: &[String]) -> Option<Vec<String>> {
    let first = tokens.first()?;
    match first.as_str() {
        "crate" => Some(tokens[1..].to_vec()),
        "self" => {
            let mut out = current.to_vec();
            out.extend_from_slice(&tokens[1..]);
            Some(out)
        }
        "super" => {
            let mut ups = 0;
            let mut rest = tokens;
            while rest.first().map(String::as_str) == Some("super") {
                ups += 1;
                rest = &rest[1..];
            }
            let mut base = current.to_vec();
            for _ in 0..ups {
                base.pop();
            }
            base.extend_from_slice(rest);
            Some(base)
        }
        _ => None,
    }
}

fn top_level(module: &[String]) -> Option<&str> {
    module.first().map(String::as_str)
}

/// Collect the `crate::` / `super::` / `self::` paths written inline in an
/// expression (`Arc<crate::io_scheduler::DiskDeviceManager>`) — a module can
/// depend on another without ever naming it in a `use`.
fn inline_paths(clean: &str, out: &mut Vec<Vec<String>>) {
    let bytes = clean.as_bytes();
    for keyword in ["crate", "super", "self"] {
        let needle = keyword.as_bytes();
        let mut i = 0;
        while i + needle.len() < bytes.len() {
            if &bytes[i..i + needle.len()] != needle {
                i += 1;
                continue;
            }
            let starts_word = i == 0 || !is_ident_byte(bytes[i - 1]);
            // The `::` check allows whitespace, the parse below deliberately does
            // not (matching the original scanner): `crate :: foo` yields no edge.
            let after = skip_whitespace(bytes, i + needle.len());
            if starts_word && bytes[after..].starts_with(b"::") {
                let mut tokens = vec![keyword.to_string()];
                parse_path_tail(clean, bytes, i + needle.len(), &mut tokens);
                out.push(tokens);
            }
            i += needle.len();
        }
    }
}

/// Position of the first non-whitespace byte at or after `i`.
fn skip_whitespace(bytes: &[u8], mut i: usize) -> usize {
    while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    i
}

/// Push every name of the `::name` chain starting at `j`, whitespace-tolerant.
fn parse_path_tail(clean: &str, bytes: &[u8], mut j: usize, tokens: &mut Vec<String>) {
    while bytes[j..].starts_with(b"::") {
        let name_start = skip_whitespace(bytes, j + 2);
        let mut name_end = name_start;
        while bytes.get(name_end).is_some_and(|b| is_ident_byte(*b)) {
            name_end += 1;
        }
        if name_end == name_start {
            break;
        }
        tokens.push(clean[name_start..name_end].to_string());
        j = skip_whitespace(bytes, name_end);
    }
}

/// Build the production-code dependency graph at top-level-module granularity.
fn module_graph(src_root: &Path) -> (BTreeSet<String>, BTreeMap<String, BTreeSet<String>>) {
    let mut files = Vec::new();
    collect_source_files(src_root, &[], &mut files);

    let nodes: BTreeSet<String> = files
        .iter()
        .filter_map(|file| top_level(&file.module).map(str::to_string))
        .collect();
    // `lib.rs` re-exports helpers (`now_ms`, `lock`, …) at the crate root; those
    // are items, not modules, and must not enter the graph as destinations.
    let module_names = nodes.clone();

    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in &files {
        let Some(from) = top_level(&file.module) else {
            continue;
        };
        let raw = std::fs::read_to_string(&file.path)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.path.display()));
        let clean = strip_noncode(&raw);
        let mut tokens = Vec::new();
        for stmt in use_statements(&clean) {
            expand_use_tree(stmt, &[], &mut tokens);
        }
        inline_paths(&clean, &mut tokens);
        for path in tokens {
            let Some(resolved) = resolve_tokens(&path, &file.module) else {
                continue;
            };
            let Some(to) = top_level(&resolved) else {
                continue;
            };
            if to == from || !module_names.contains(to) {
                continue;
            }
            edges
                .entry(from.to_string())
                .or_default()
                .insert(to.to_string());
        }
    }
    // No pruning: a top-level module with *zero* edges is legitimate now that
    // the foundation (`limedl-types`) and disk (`limedl-disk`) layers live in
    // other crates — `test_harness` has nothing left inside this crate to point
    // at. Dropping it here made it look like a stale `LAYERS` entry.
    (nodes, edges)
}

/// Tarjan's SCC algorithm; returns every component with more than one member.
fn cycles(
    nodes: &BTreeSet<String>,
    edges: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<Vec<String>> {
    struct State<'a> {
        edges: &'a BTreeMap<String, BTreeSet<String>>,
        index: BTreeMap<String, usize>,
        low: BTreeMap<String, usize>,
        on_stack: BTreeSet<String>,
        stack: Vec<String>,
        next: usize,
        found: Vec<Vec<String>>,
    }

    fn visit(state: &mut State<'_>, v: &str) {
        state.index.insert(v.to_string(), state.next);
        state.low.insert(v.to_string(), state.next);
        state.next += 1;
        state.stack.push(v.to_string());
        state.on_stack.insert(v.to_string());

        if let Some(targets) = state.edges.get(v) {
            for w in targets {
                if !state.index.contains_key(w) {
                    visit(state, w);
                    let low_w = state.low[w];
                    let low_v = state.low[v];
                    state.low.insert(v.to_string(), low_v.min(low_w));
                } else if state.on_stack.contains(w) {
                    let idx_w = state.index[w];
                    let low_v = state.low[v];
                    state.low.insert(v.to_string(), low_v.min(idx_w));
                }
            }
        }

        if state.low[v] == state.index[v] {
            let mut component = Vec::new();
            while let Some(w) = state.stack.pop() {
                state.on_stack.remove(&w);
                let done = w == v;
                component.push(w);
                if done {
                    break;
                }
            }
            if component.len() > 1 {
                component.sort();
                state.found.push(component);
            }
        }
    }

    let mut state = State {
        edges,
        index: BTreeMap::new(),
        low: BTreeMap::new(),
        on_stack: BTreeSet::new(),
        stack: Vec::new(),
        next: 0,
        found: Vec::new(),
    };
    for node in nodes {
        if !state.index.contains_key(node) {
            visit(&mut state, node);
        }
    }
    state.found.sort();
    state.found
}

fn render_edges(edges: &BTreeMap<String, BTreeSet<String>>) -> String {
    let mut out = String::new();
    for (from, targets) in edges {
        for to in targets {
            out.push_str(&format!("  {from} -> {to}\n"));
        }
    }
    out
}

#[test]
fn modules_form_a_dag_outside_the_allowed_cycles() {
    let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let (nodes, edges) = module_graph(&src_root);
    assert!(
        // The floor only exists to catch a broken parser, not to assert a
        // module count: after six extractions the facade holds eight modules,
        // so the bar had to come down with it.
        nodes.len() > 4,
        "module discovery found only {} modules under {} — the parser is broken, \
         not the dependency graph",
        nodes.len(),
        src_root.display()
    );

    let found = cycles(&nodes, &edges);
    let allowed: BTreeSet<Vec<String>> = ALLOWED_CYCLES
        .iter()
        .map(|cycle| cycle.iter().map(|s| s.to_string()).collect())
        .collect();

    let unexpected: Vec<&Vec<String>> = found.iter().filter(|c| !allowed.contains(*c)).collect();
    let stale: Vec<&Vec<String>> = allowed.iter().filter(|c| !found.contains(*c)).collect();

    assert!(
        unexpected.is_empty() && stale.is_empty(),
        "module dependency cycles changed.\n\
         \n\
         new cycles (fix the dependency direction, or extract a lower-layer module):\n{}\
         \n\
         stale whitelist entries (the cycle is gone — delete it from ALLOWED_CYCLES):\n{}\
         \n\
         current top-level module edges:\n{}",
        unexpected
            .iter()
            .map(|c| format!("  {}\n", c.join(" <-> ")))
            .collect::<String>(),
        stale
            .iter()
            .map(|c| format!("  {}\n", c.join(" <-> ")))
            .collect::<String>(),
        render_edges(&edges),
    );
}

#[test]
fn modules_only_depend_on_their_own_or_a_lower_layer() {
    let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let (nodes, edges) = module_graph(&src_root);
    assert!(
        // The floor only exists to catch a broken parser, not to assert a
        // module count: after six extractions the facade holds eight modules,
        // so the bar had to come down with it.
        nodes.len() > 4,
        "module discovery found only {} modules under {} — the parser is broken, \
         not the dependency graph",
        nodes.len(),
        src_root.display()
    );

    // A module that is in the graph but not in the table escapes the guard
    // silently, so a new top-level module has to declare its layer.
    let unclassified: Vec<&str> = nodes
        .iter()
        .map(String::as_str)
        .filter(|module| layer_of(module).is_none())
        .collect();
    assert!(
        unclassified.is_empty(),
        "top-level modules missing from LAYERS:\n{}\n\
         \n\
         Add each one to the layer it belongs to, or to the layer of the crate it \
         is going to become.\n\
         \n\
         current top-level module edges:\n{}",
        unclassified
            .iter()
            .map(|module| format!("  {module}\n"))
            .collect::<String>(),
        render_edges(&edges),
    );

    // The reverse direction: an entry whose module no longer exists means a
    // module was moved to another crate (or renamed) without updating the
    // contract.
    let stale: Vec<&str> = LAYERS
        .iter()
        .flat_map(|(_, members)| members.iter().copied())
        .filter(|module| !nodes.contains(*module))
        .collect();
    assert!(
        stale.is_empty(),
        "LAYERS names modules that no longer exist:\n{}\n\
         \n\
         Delete each entry in the same commit that moves the module out of this \
         crate.\n",
        stale
            .iter()
            .map(|module| format!("  {module}\n"))
            .collect::<String>(),
    );

    let mut upward: Vec<String> = Vec::new();
    for (from, targets) in &edges {
        let (from_layer, from_crate) = layer_of(from).expect("classified above");
        for to in targets {
            let (to_layer, to_crate) = layer_of(to).expect("classified above");
            if to_layer > from_layer {
                upward.push(format!(
                    "  {from} ({from_crate}, layer {from_layer}) \
                     -> {to} ({to_crate}, layer {to_layer})\n"
                ));
            }
        }
    }
    upward.sort();
    assert!(
        upward.is_empty(),
        "module dependency direction regressed: the following edges point at a \
         higher layer.\n\
         \n\
         Once these modules are separate crates, each of these is a Cargo cycle — \
         invert the dependency (pass the value down instead of reaching up, or \
         move the shared piece into the lower layer) instead of relaxing LAYERS.\n\
         \n\
         upward edges:\n{}\
         \n\
         layer table (bottom-up, from LAYERS):\n{}\
         \n\
         current top-level module edges:\n{}",
        upward.concat(),
        LAYERS
            .iter()
            .enumerate()
            .map(|(index, (crate_name, members))| format!(
                "  layer {index} ({crate_name}): {}\n",
                members.join(", ")
            ))
            .collect::<String>(),
        render_edges(&edges),
    );
}
