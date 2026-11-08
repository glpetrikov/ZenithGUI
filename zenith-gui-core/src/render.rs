//! Backend-independent rendering contract: the [`DrawList`] extracted from the
//! laid-out tree each frame, and the [`Renderer`] trait that consumes it.

use std::sync::Arc;

use bevy_ecs::prelude::*;

use crate::geometry::{Color, Rect, Vec2};
use crate::node::{
    BackgroundColor, Border, ClipChildren, ComputedLayout, CornerRadius, Node, Visibility,
};
use crate::text::{Text, TextColor, TextMeasure};

/// Coordinates are logical pixels, origin top-left.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    Rect {
        rect: Rect,
        color: Color,
        radius: f32,
        border: Option<(f32, Color)>,
    },
    /// Single-line text with its top-left at `pos`.
    Text {
        pos: Vec2,
        text: String,
        size: f32,
        color: Color,
    },
    Line {
        a: Vec2,
        b: Vec2,
        width: f32,
        color: Color,
    },
    /// Cubic bezier through control points `p`.
    Bezier {
        p: [Vec2; 4],
        width: f32,
        color: Color,
    },
    /// Subsequent commands are clipped to `rect` (intersected with outer clips)
    /// until the matching [`DrawCommand::PopClip`].
    PushClip(Rect),
    PopClip,
}

/// Paint-ordered (back to front) commands for one frame; rebuilt every frame.
#[derive(Resource, Debug, Default)]
pub struct DrawList {
    pub commands: Vec<DrawCommand>,
}

/// Extra commands a widget wants drawn right after its own background and
/// before its children (grids, links, ...). Rewritten by the widget each frame
/// in [`UiSet::PreRender`](crate::app::UiSet::PreRender).
#[derive(Component, Debug, Default, Clone)]
pub struct CustomDraw(pub Vec<DrawCommand>);

#[derive(Debug)]
pub enum RenderError {
    /// The surface/device is gone and could not be recovered.
    Lost(String),
    Other(String),
}

impl core::fmt::Display for RenderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Lost(m) => write!(f, "render target lost: {m}"),
            Self::Other(m) => write!(f, "render error: {m}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// A GPU (or software) backend. The windowing layer owns one and calls it
/// after [`ZenithApp::update`](crate::app::ZenithApp::update).
pub trait Renderer {
    /// Target size in physical pixels plus the logical→physical scale.
    fn resize(&mut self, width_px: u32, height_px: u32, scale_factor: f32);
    fn render(&mut self, draw_list: &DrawList) -> Result<(), RenderError>;
    /// Text metrics backed by the renderer's fonts; install it as the
    /// [`TextMeasurer`](crate::text::TextMeasurer) resource.
    fn text_measure(&self) -> Arc<dyn TextMeasure>;
}

type Drawable = (
    &'static ComputedLayout,
    &'static Visibility,
    Option<&'static BackgroundColor>,
    Option<&'static CornerRadius>,
    Option<&'static Border>,
    Option<&'static Text>,
    Option<&'static TextColor>,
    Option<&'static CustomDraw>,
    Option<&'static Children>,
    Has<ClipChildren>,
);

fn emit(e: Entity, nodes: &Query<Drawable, With<Node>>, out: &mut Vec<DrawCommand>) {
    let Ok((layout, vis, bg, radius, border, text, text_color, custom, children, clips)) =
        nodes.get(e)
    else {
        return;
    };
    if *vis == Visibility::Hidden {
        return;
    }
    let radius = radius.map_or(0.0, |r| r.0);
    let border = border.map(|b| (b.width, b.color));
    let bg = bg.filter(|bg| bg.0.a > 0.0);
    if bg.is_some() || border.is_some() {
        out.push(DrawCommand::Rect {
            rect: layout.rect,
            color: bg.map_or(Color::TRANSPARENT, |bg| bg.0),
            radius,
            border,
        });
    }
    if clips {
        out.push(DrawCommand::PushClip(layout.rect));
    }
    if let Some(custom) = custom {
        out.extend(custom.0.iter().cloned());
    }
    if let Some(text) = text.filter(|t| !t.value.is_empty()) {
        out.push(DrawCommand::Text {
            pos: layout.rect.pos,
            text: text.value.clone(),
            size: text.size,
            color: text_color.copied().unwrap_or_default().0,
        });
    }
    for &child in children.map(|c| &**c).unwrap_or(&[]) {
        emit(child, nodes, out);
    }
    if clips {
        out.push(DrawCommand::PopClip);
    }
}

pub(crate) fn extract_draw_list(
    mut list: ResMut<DrawList>,
    roots: Query<Entity, (With<Node>, Without<ChildOf>)>,
    nodes: Query<Drawable, With<Node>>,
) {
    list.commands.clear();
    for root in &roots {
        emit(root, &nodes, &mut list.commands);
    }
}
