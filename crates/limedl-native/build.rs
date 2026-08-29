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
