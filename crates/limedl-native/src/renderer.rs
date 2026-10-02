//! The renderer name this build advertises.
//!
//! Slint's renderer is a compile-time choice, and this crate compiles in exactly
//! one pair: FemtoVG on OpenGL, with Slint's software rasterizer behind it. The
//! name still lives in one place because it is a user-visible claim — the About
//! tab's core-tech line and the startup log both print it, and neither call site
//! should spell it out itself.
//!
//! Why this pair rather than a single renderer: FemtoVG/GL is the primary because
//! it rasterizes a 1000-task list at 108 fps / 8.0 ms CPU per frame, and the
//! software rasterizer is kept *compiled in* because 0.4.0 shipped FemtoVG alone
//! and a machine whose GL stack fails to initialize (RDP session, VM, broken
//! driver) then had no renderer left to open a window with — the one failure mode
//! a user cannot work around. On that path software gives 39 fps / 20.4 ms but
//! starts faster (341 ms to window vs 593 ms) and uses less memory (171 MB vs
//! 236 MB); it costs 0.47 MB of executable.
//!
//! [`NAME`] names the *preferred* renderer, not a runtime fact: when GL init
//! fails, Slint drops to the software rasterizer silently and the About tab still
//! says FemtoVG/OpenGL. The startup log's own `Slint: ... Backend:` line, printed
//! only under `SLINT_DEBUG_PERFORMANCE`, is the authoritative record. Slint
//! re-exports no accessor for the renderer that actually got created
//! (`slint::platform` exposes only `WinitWindowAccessor`), so there is nothing to
//! query — do not read this constant as a guarantee that the GPU path was taken.

/// Renderer name shown in the About tab (`arch_info` / the core-tech line) and
/// logged at startup. Kept short — it is rendered inside a `(…)` in the
/// "Target Arch & Graphics" row.
pub const NAME: &str = "FemtoVG/OpenGL";
