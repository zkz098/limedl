fn main() {
    // MiSans VF is bundled via `import "../assets/fonts/MiSansVF.ttf"` in
    // appwindow.slint but is NOT committed (MiSans font license forbids
    // re-distributing the font file itself — embedding in the app is fine).
    // Fail early with instructions instead of the slint compiler's raw error.
    let misans = std::path::Path::new("assets/fonts/MiSansVF.ttf");
    if !misans.exists() {
        panic!(
            "MiSans VF font missing at crates/limedl-native/assets/fonts/MiSansVF.ttf.\n\
             Fetch it from the official source (one-time, about 15 MB):\n\
             \x20   cargo xtask fetch-font"
        );
    }
    println!("cargo:rerun-if-changed=assets/fonts/MiSansVF.ttf");
    println!("cargo:rerun-if-changed=ui/assets/icon.ico");
    // `embed-resource` never emits this itself (its preprocessing step cannot
    // produce dependency lines, see the crate's README errata); because this
    // script prints any `rerun-if-changed` at all, cargo would otherwise stop
    // rescanning the crate root and a `.rc` edit would not relink.
    println!("cargo:rerun-if-changed=limedl-native.rc");
    // Not read by this build script, but the macOS release bundle is assembled
    // from them (`scripts/package-macos.sh` runs post-build), so a change to the
    // bundle identity or the icon source has to invalidate the cached binary.
    println!("cargo:rerun-if-changed=ui/assets/icon.png");
    println!("cargo:rerun-if-changed=../../packaging/macos/Info.plist.in");

    // Windows PE file resources: embed app icon (.ico), product name, description,
    // copyright, and the four-integer VERSIONINFO that Explorer reads.
    //
    // NOTE the cfg is `windows`, NOT a runtime `CARGO_CFG_TARGET_OS == "windows"`
    // check. A build script is itself compiled for the host, and `embed-resource`
    // lives in `[target.'cfg(windows)'.build-dependencies]` — so on macOS/Linux the
    // crate is simply not linked into the build script, and a runtime check would
    // compile `embed_resource::compile` anyway (E0433) while only *skipping* it at
    // run time. The env var is only correct in cargo's own cfg plumbing; a `#[cfg]`
    // here reads the same signal at compile time and keeps the dependency graph
    // honest.
    #[cfg(windows)]
    {
        // Guard against the silent breakage this replaced: if cargo ever stops
        // setting a host cfg that does not match the target, the resources would
        // stop being embedded without a word.
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
            eprintln!(
                "cargo:warning=built for a non-Windows target on a Windows host; \
                 skipping the PE resource embedding"
            );
        }

        // `CARGO_PKG_VERSION_*` are the parsed components; `CARGO_PKG_VERSION` is
        // the dotted string the string table shows. A pre-release suffix is
        // already absent from the numeric parts, which is what VERSIONINFO wants.
        let macro_of = |name: &str, var: &str| {
            format!(
                "{name}={}",
                std::env::var(var).expect("cargo sets CARGO_PKG_VERSION*")
            )
        };
        let version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
        let macros = [
            macro_of("VERSION_MAJOR", "CARGO_PKG_VERSION_MAJOR"),
            macro_of("VERSION_MINOR", "CARGO_PKG_VERSION_MINOR"),
            macro_of("VERSION_PATCH", "CARGO_PKG_VERSION_PATCH"),
            // VERSIONINFO has no build number to source here; 0 matches what
            // winres derived from a plain `major.minor.patch` package version.
            String::from("VERSION_BUILD=0"),
            format!("VERSION_STR=\"{version}\""),
        ];
        // Warn instead of failing the build, exactly like the winres call this
        // replaced: a missing RC.EXE is a packaging defect, not a reason to make
        // `cargo check` unusable. `NotAttempted` (no compiler found) is called out
        // separately — `manifest_optional()` would swallow it silently.
        match embed_resource::compile("limedl-native.rc", macros) {
            embed_resource::CompilationResult::Ok
            | embed_resource::CompilationResult::NotWindows => {}
            other => eprintln!("cargo:warning=Failed to embed Windows resources: {other}"),
        }
    }

    // slint-build only tracks .slint sources/assets — without this, editing the
    // .po catalogs does not trigger regeneration of the bundled translations.
    println!("cargo:rerun-if-changed=lang");

    // Element introspection needs the debug metadata the compiler only embeds on
    // request: without it `ElementHandle::find_by_element_id` (the in-process UI
    // tests) and every id lookup over the MCP server silently match nothing and
    // print a warning instead of failing.
    //
    // Two ways in, deliberately:
    //   * debug builds always get it — `PROFILE` is the profile of the crate
    //     being built (not of this build script), so `cargo test`/`cargo run`
    //     work with no setup;
    //   * an explicit `SLINT_EMIT_DEBUG_INFO=1` is left alone. It is already read
    //     by `CompilerConfiguration::new()` (which forwards to the compiler's
    //     env-reading constructor), so *not* calling `with_debug_info` here is
    //     what keeps `cargo run --release` drivable by the MCP server. Passing
    //     `false` for release profiles instead would silently override the very
    //     variable the tool documents.
    //
    // The shipped release binary therefore carries no metadata unless someone
    // asks for it.
    println!("cargo:rerun-if-env-changed=PROFILE");
    let mut config = slint_build::CompilerConfiguration::new();
    if std::env::var("PROFILE").is_ok_and(|profile| profile == "debug") {
        config = config.with_debug_info(true);
    }

    let config = config
        // Rasterize @image-url SVG assets at 2x: the compiler embeds SVGs as
        // bitmaps rasterized at this scale factor. Without it they render at
        // 1x and look blurry when the window scale factor is > 1.
        .with_scale_factor(2.0)
        // The bundled catalogs (lang/*/LC_MESSAGES/limedl-native.po) are plain
        // msgid/msgstr pairs without `msgctxt`. Slint's default translation
        // context is the enclosing component name, which makes every runtime
        // lookup miss and fall back to the English source strings. Disable the
        // default context so `@tr("...")` matches context-less entries.
        // NOTE: if regenerating catalogs with slint-tr-extractor, pass
        // --no-default-translation-context to keep them context-less.
        .with_default_translation_context(slint_build::DefaultTranslationContext::None)
        .with_bundled_translations("lang");
    slint_build::compile_with_config("ui/appwindow.slint", config).expect("Slint build failed");
}
