//! Which graphics renderer this build compiles in.
//!
//! Slint picks its renderer at compile time (the three mutually exclusive
//! `renderer-*` features in `Cargo.toml`); `SLINT_BACKEND` can only re-select a
//! renderer that is already compiled in, never add one. So this module is the
//! single source of truth for every user-visible claim about the rasterizer
//! (About tab, startup log) — `"Skia"` must not be hardcoded at a call site, or
//! a FemtoVG build would advertise the wrong backend.
//!
//! The const assertion is load-bearing: with two renderers compiled in, Slint's
//! backend selector silently prefers one of them (Skia > FemtoVG/wgpu >
//! FemtoVG/GL), and a single label would then describe a rasterizer that may not
//! be the active one. Failing the build is the honest option for a build option
//! that exists to compare the two.

const _: () = assert!(
    (cfg!(feature = "renderer-skia") as u8
        + cfg!(feature = "renderer-skia-opengl") as u8
        + cfg!(feature = "renderer-femtovg") as u8
        + cfg!(feature = "renderer-femtovg-wgpu") as u8)
        == 1,
    "enable exactly one renderer feature: renderer-skia (default), renderer-skia-opengl, renderer-femtovg or renderer-femtovg-wgpu"
);

/// Renderer name shown in the About tab (`arch_info` / the core-tech line) and
/// logged at startup. Kept short — it is rendered inside a `(…)` in the
/// "Target Arch & Graphics" row.
///
/// The order mirrors Slint's own renderer precedence (Skia > FemtoVG/wgpu >
/// FemtoVG/GL), so even the state the assertion above rejects would report the
/// renderer Slint would actually have picked.
pub const NAME: &str = if cfg!(feature = "renderer-skia") {
    "Skia"
} else if cfg!(feature = "renderer-skia-opengl") {
    "Skia/OpenGL"
} else if cfg!(feature = "renderer-femtovg-wgpu") {
    "FemtoVG/wgpu"
} else {
    "FemtoVG/OpenGL"
};
