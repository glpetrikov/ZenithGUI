//! `wgpu` renderer backend for Zenith GUI.
//!
//! [`WgpuRenderer`] implements [`zenith_gui_core::render::Renderer`]: it takes
//! the frame's [`DrawList`] and draws SDF rounded rectangles, anti-aliased
//! lines/béziers and `glyphon` text, honoring clip regions via scissor rects
//! (text via per-item bounds).
//!
//! Two kinds of target:
//! - a window surface ([`WgpuRenderer::new`]), presented every frame;
//! - an offscreen texture ([`WgpuRenderer::new_headless`]), readable with
//!   [`WgpuRenderer::render_to_rgba`] (tests, screenshots, CI).

pub mod batch;
mod renderer;
pub mod text;

pub use renderer::{RendererConfig, RendererInitError, WgpuRenderer};

pub mod prelude {
    pub use crate::{RendererConfig, WgpuRenderer};
}
