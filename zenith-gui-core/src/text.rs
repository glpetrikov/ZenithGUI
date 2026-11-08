//! Text primitives. Shaping/rendering is the renderer's job; layout only sees
//! [`ContentSize`](crate::layout::ContentSize), produced through [`TextMeasure`].

use std::sync::Arc;

use bevy_ecs::prelude::*;

use crate::geometry::{Color, Vec2};
use crate::layout::ContentSize;

pub const DEFAULT_FONT_SIZE: f32 = 14.0;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Text {
    pub value: String,
    pub size: f32,
}

impl Default for Text {
    fn default() -> Self {
        Self::new("")
    }
}

impl Text {
    pub fn new(s: impl Into<String>) -> Self {
        Self {
            value: s.into(),
            size: DEFAULT_FONT_SIZE,
        }
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct TextColor(pub Color);

impl Default for TextColor {
    fn default() -> Self {
        Self(Color::hex(0xe6e6ee))
    }
}

/// Measures single-line text in logical pixels. The renderer provides the real
/// implementation (it owns the fonts); [`ApproxTextMeasure`] is the fallback.
pub trait TextMeasure: Send + Sync {
    fn measure(&self, text: &str, font_size: f32) -> Vec2;
}

/// Font-independent estimate: fixed advance per char.
#[derive(Debug, Default, Clone, Copy)]
pub struct ApproxTextMeasure;

impl TextMeasure for ApproxTextMeasure {
    fn measure(&self, text: &str, font_size: f32) -> Vec2 {
        Vec2::new(
            text.chars().count() as f32 * font_size * 0.55,
            font_size * 1.3,
        )
    }
}

#[derive(Resource, Clone)]
pub struct TextMeasurer(pub Arc<dyn TextMeasure>);

impl Default for TextMeasurer {
    fn default() -> Self {
        Self(Arc::new(ApproxTextMeasure))
    }
}

pub(crate) fn measure_text(
    measurer: Res<TextMeasurer>,
    mut texts: Query<(Ref<Text>, &mut ContentSize)>,
) {
    let all = measurer.is_changed();
    for (text, mut size) in &mut texts {
        if all || text.is_changed() {
            size.set_if_neq(ContentSize(measurer.0.measure(&text.value, text.size)));
        }
    }
}
