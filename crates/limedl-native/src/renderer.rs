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
//!
//! The default build pairs FemtoVG/GL with the **additive**
//! `renderer-software-fallback` feature, which the assertion deliberately does
//! not count. That makes [`NAME`] the *preferred* renderer rather than a runtime
//! fact: if GL init fails, Slint silently drops to its software rasterizer (the
//! startup log's own `Slint: ... Backend:` line, printed only under
//! `SLINT_DEBUG_PERFORMANCE`, is the authoritative record) and the About tab will
//! still name FemtoVG. Slint re-exports no accessor for the renderer that
//! actually got created (`slint::platform` exposes only `WinitWindowAccessor`),
//! so there is nothing to query — do not read this constant as a guarantee that
//! the GPU path was taken, only that it was requested.

const _: () = assert!(
    (cfg!(feature = "renderer-skia") as u8
        + cfg!(feature = "renderer-skia-opengl") as u8
        + cfg!(feature = "renderer-femtovg") as u8
        + cfg!(feature = "renderer-femtovg-wgpu") as u8)
        == 1,
    "enable exactly one renderer feature: renderer-femtovg (default), renderer-femtovg-wgpu, renderer-skia or renderer-skia-opengl"
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
