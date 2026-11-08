//! Windowless runner: same app + wgpu backend, rendering to an offscreen
//! texture. For snapshot tests and CI. Needs a Vulkan/Metal/DX12/GL adapter
//! (software ones like lavapipe work).

use std::path::Path;

use zenith_gui_core::prelude::*;
use zenith_gui_wgpu::{RendererConfig, WgpuRenderer};
use zenith_gui_widgets::WidgetsPlugin;

use crate::AppError;

pub struct HeadlessApp {
    pub app: ZenithApp,
    renderer: WgpuRenderer,
    size_px: (u32, u32),
}

impl HeadlessApp {
    /// `size` is logical px; the output is `size * scale_factor` physical px.
    pub fn new(size: (f32, f32), scale_factor: f32) -> Result<Self, AppError> {
        let size_px = (
            (size.0 * scale_factor).round() as u32,
            (size.1 * scale_factor).round() as u32,
        );
        let renderer =
            WgpuRenderer::new_headless(size_px, scale_factor, RendererConfig::default())?;
        let mut app = ZenithApp::new();
        app.add_plugin(WidgetsPlugin);
        app.insert_resource(TextMeasurer(renderer.text_measure()));
        app.insert_resource(Viewport {
            size: Vec2::new(size.0, size.1),
            scale_factor,
        });
        Ok(Self {
            app,
            renderer,
            size_px,
        })
    }

    /// Runs one frame of the schedule.
    pub fn update(&mut self) {
        self.app.update();
    }

    pub fn size_px(&self) -> (u32, u32) {
        self.size_px
    }

    /// Runs a frame and returns RGBA8 pixels.
    pub fn render_rgba(&mut self) -> Result<Vec<u8>, AppError> {
        self.app.update();
        Ok(self
            .renderer
            .render_to_rgba(self.app.world.resource::<DrawList>())?)
    }

    /// Runs a frame and writes a PNG.
    pub fn save_png(&mut self, path: impl AsRef<Path>) -> Result<(), AppError> {
        let rgba = self.render_rgba()?;
        let io = |e: &dyn std::fmt::Display| AppError::Io(e.to_string());
        let file = std::fs::File::create(path.as_ref()).map_err(|e| io(&e))?;
        let mut enc = png::Encoder::new(file, self.size_px.0, self.size_px.1);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .and_then(|mut w| w.write_image_data(&rgba))
            .map_err(|e| io(&e))
    }
}
