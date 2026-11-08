//! Renderer backend for Zenith GUI.
//!
//! Today this crate only contains the backend-independent half: an ECS system
//! that extracts a flat, paint-ordered [`DrawList`] from the laid-out UI tree,
//! and the [`Renderer`] trait a GPU backend implements. The actual `wgpu`
//! device/surface/pipeline code is not wired up yet.

use zenith_gui_core::prelude::*;

pub mod prelude {
    pub use crate::{DrawCommand, DrawList, RenderPlugin, Renderer};
}

#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    Rect {
        rect: Rect,
        color: Color,
    },
    Text {
        rect: Rect,
        text: String,
        color: Color,
    },
}

/// Paint-ordered (back to front) commands for one frame; rebuilt every frame.
#[derive(Resource, Debug, Default)]
pub struct DrawList {
    pub commands: Vec<DrawCommand>,
}

/// Implemented by GPU backends; call from the windowing loop after
/// [`ZenithApp::update`].
pub trait Renderer {
    fn resize(&mut self, viewport: Viewport);
    fn render(&mut self, draw_list: &DrawList);
}

type Drawable = (
    &'static ComputedLayout,
    &'static Visibility,
    Option<&'static BackgroundColor>,
    Option<&'static Text>,
    Option<&'static TextColor>,
);

fn extract_draw_list(mut list: ResMut<DrawList>, nodes: Query<Drawable, With<Node>>) {
    let mut visible: Vec<_> = nodes
        .iter()
        .filter(|(_, vis, ..)| **vis == Visibility::Visible)
        .collect();
    visible.sort_by_key(|(layout, ..)| layout.order);

    list.commands.clear();
    for (layout, _, bg, text, text_color) in visible {
        if let Some(bg) = bg.filter(|bg| bg.0.a > 0.0) {
            list.commands.push(DrawCommand::Rect {
                rect: layout.rect,
                color: bg.0,
            });
        }
        if let Some(text) = text {
            list.commands.push(DrawCommand::Text {
                rect: layout.rect,
                text: text.0.clone(),
                color: text_color.copied().unwrap_or_default().0,
            });
        }
    }
}

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut ZenithApp) {
        app.init_resource::<DrawList>().add_systems(
            ZenithSchedule::Main,
            extract_draw_list.in_set(UiSet::Render),
        );
    }
}
