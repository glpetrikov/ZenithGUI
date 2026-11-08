//! Font handling: one shared cosmic-text `FontSystem` used both for measuring
//! (during layout) and for shaping/rendering (inside the renderer).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};
use zenith_gui_core::prelude::*;

/// Line height as a multiple of the font size. Must match [`ApproxTextMeasure`]-style
/// expectations in layout: measure and render both use it.
pub const LINE_HEIGHT_FACTOR: f32 = 1.3;

pub type SharedFonts = Arc<Mutex<FontSystem>>;

pub fn metrics(font_size_px: f32) -> Metrics {
    Metrics::new(font_size_px, (font_size_px * LINE_HEIGHT_FACTOR).round())
}

/// Shapes `text` into `buffer` (single line, no wrapping).
pub fn shape(fonts: &mut FontSystem, buffer: &mut Buffer, text: &str, size_px: f32) {
    buffer.set_metrics(metrics(size_px));
    buffer.set_size(None, None);
    buffer.set_text(
        text,
        &Attrs::new().family(Family::SansSerif),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(fonts, false);
}

/// Logical-pixel text metrics backed by the real fonts.
pub struct FontMeasure {
    fonts: SharedFonts,
    cache: Mutex<HashMap<(String, u32), Vec2>>,
}

impl FontMeasure {
    pub fn new(fonts: SharedFonts) -> Self {
        Self {
            fonts,
            cache: Mutex::new(HashMap::new()),
        }
    }
}

impl TextMeasure for FontMeasure {
    fn measure(&self, text: &str, font_size: f32) -> Vec2 {
        let key = (text.to_owned(), font_size.to_bits());
        let mut cache = self.cache.lock().unwrap();
        if let Some(v) = cache.get(&key) {
            return *v;
        }
        let mut fonts = self.fonts.lock().unwrap();
        let mut buffer = Buffer::new_empty(metrics(font_size));
        shape(&mut fonts, &mut buffer, text, font_size);
        let width = buffer.layout_runs().map(|r| r.line_w).fold(0.0, f32::max);
        let size = Vec2::new(width.ceil(), metrics(font_size).line_height);
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(key, size);
        size
    }
}
