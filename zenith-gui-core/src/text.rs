//! Text primitives. Shaping/measuring is the renderer's job; layout only sees
//! [`ContentSize`](crate::layout::ContentSize).

use bevy_ecs::prelude::*;

use crate::geometry::Color;

#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct Text(pub String);

impl Text {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct TextColor(pub Color);

impl Default for TextColor {
    fn default() -> Self {
        Self(Color::WHITE)
    }
}
