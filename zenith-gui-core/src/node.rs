//! The base UI element components.

use bevy_ecs::prelude::*;

use crate::geometry::{Color, Rect};
use crate::interaction::Interaction;
use crate::layout::Style;

/// Marker for a UI element. Spawn it (or a widget that requires it) and
/// parent it with [`ChildOf`] to build the tree. Entities without a parent are
/// layout roots and fill the [`Viewport`](crate::input::Viewport).
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(Style, ComputedLayout, Visibility, Interaction)]
pub struct Node;

/// Output of the layout pass, in absolute (window) logical pixels.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct ComputedLayout {
    pub rect: Rect,
    /// Paint/hit-test order: higher is drawn later (on top).
    pub order: u32,
    /// Intersection of all ancestor [`ClipChildren`] rects, if any.
    pub clip: Option<Rect>,
    /// `false` if this node or any ancestor is [`Visibility::Hidden`].
    pub visible: bool,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    #[default]
    Visible,
    /// Not drawn and not hit-testable, together with its whole subtree, but
    /// still takes part in layout.
    Hidden,
}

/// Children are clipped to this node's rect (drawing and hit-testing).
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct ClipChildren;

/// Pointer events skip this node and hit whatever is below it (labels,
/// decorations).
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct PassThrough;

#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct BackgroundColor(pub Color);

#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct CornerRadius(pub f32);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Border {
    pub width: f32,
    pub color: Color,
}
