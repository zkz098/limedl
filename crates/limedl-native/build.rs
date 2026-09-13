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
             \x20   pwsh scripts/fetch-misans.ps1"
        );
    }
    println!("cargo:rerun-if-changed=assets/fonts/MiSansVF.ttf");
    println!("cargo:rerun-if-changed=ui/assets/icon.ico");
    // Not read by this build script, but the macOS release bundle is assembled
    // from them (`scripts/package-macos.sh` runs post-build), so a change to the
    // bundle identity or the icon source has to invalidate the cached binary.
    println!("cargo:rerun-if-changed=ui/assets/icon.png");
    println!("cargo:rerun-if-changed=../../packaging/macos/Info.plist.in");

    // Windows PE file resources: embed app icon (.ico), product name, description,
    // copyright.
    //
    // NOTE the cfg is `windows`, NOT a runtime `CARGO_CFG_TARGET_OS == "windows"`
    // check. A build script is itself compiled for the host, and `winres` lives in
    // `[target.'cfg(windows)'.build-dependencies]` — so on macOS/Linux the crate is
    // simply not linked into the build script, and a runtime check would compile
    // `winres::WindowsResource` anyway (E0433) while only *skipping* it at run time.
    // The env var is only correct in cargo's own cfg plumbing; a `#[cfg]` here reads
    // the same signal at compile time and keeps the dependency graph honest.
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
        let mut res = winres::WindowsResource::new();
        res.set_icon("ui/assets/icon.ico");
        res.set("ProductName", "limedl");
        res.set("FileDescription", "limedl - Native High-Performance Download Manager");
        res.set("LegalCopyright", "Copyright (c) 2026 zkz098");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to compile Windows resources: {e}");
        }
    }

    // slint-build only tracks .slint sources/assets — without this, editing the
    // .po catalogs does not trigger regeneration of the bundled translations.
    println!("cargo:rerun-if-changed=lang");

    let config = slint_build::CompilerConfiguration::new()
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
