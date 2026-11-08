//! Widget library: thin component + system layers on top of `zenith-gui-core`.
//!
//! Each widget is a component that `#[require]`s the primitives it is made of,
//! so `commands.spawn(Button)` is a complete, valid node.

use zenith_gui_core::prelude::*;

pub mod prelude {
    pub use crate::{Button, ButtonColors, Label, Panel, WidgetsPlugin, button, label};
}

/// Plain container with a background.
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(Node, BackgroundColor)]
pub struct Panel;

/// Single run of text. Size is derived from [`Text`] (see [`measure_labels`]).
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(Node, Text, TextColor, ContentSize)]
pub struct Label;

/// Clickable container; listen for [`Clicked`] on it. Colors follow [`Interaction`].
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(Node, BackgroundColor, ButtonColors)]
pub struct Button;

#[derive(Component, Debug, Clone, Copy)]
pub struct ButtonColors {
    pub normal: Color,
    pub hovered: Color,
    pub pressed: Color,
}

impl Default for ButtonColors {
    fn default() -> Self {
        Self {
            normal: Color::rgb(0.20, 0.20, 0.25),
            hovered: Color::rgb(0.28, 0.28, 0.36),
            pressed: Color::rgb(0.14, 0.14, 0.18),
        }
    }
}

/// A [`Label`] bundle.
pub fn label(text: impl Into<String>) -> impl Bundle {
    (Label, Text::new(text))
}

/// A [`Button`] with a [`Label`] child, padded.
pub fn button(text: impl Into<String>) -> impl Bundle {
    (
        Button,
        Style {
            padding: UiRect::all(8.0),
            align_items: AlignItems::Center,
            ..Default::default()
        },
        children![label(text)],
    )
}

/// Placeholder text metrics (monospace-ish). The real renderer should replace
/// this with shaped-text measurement writing the same [`ContentSize`].
const CHAR_WIDTH: f32 = 8.0;
const LINE_HEIGHT: f32 = 16.0;

type ChangedLabels = (With<Label>, Changed<Text>);
type DirtyButtons = (With<Button>, Or<(Changed<Interaction>, Added<Button>)>);

fn measure_labels(mut labels: Query<(&Text, &mut ContentSize), ChangedLabels>) {
    for (text, mut size) in &mut labels {
        let chars = text.0.chars().count() as f32;
        size.set_if_neq(ContentSize(Vec2::new(chars * CHAR_WIDTH, LINE_HEIGHT)));
    }
}

fn button_visuals(
    mut buttons: Query<(&Interaction, &ButtonColors, &mut BackgroundColor), DirtyButtons>,
) {
    for (interaction, colors, mut bg) in &mut buttons {
        bg.0 = match interaction {
            Interaction::None => colors.normal,
            Interaction::Hovered => colors.hovered,
            Interaction::Pressed => colors.pressed,
        };
    }
}

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut ZenithApp) {
        app.add_systems(
            ZenithSchedule::Main,
            (measure_labels, button_visuals).in_set(UiSet::Update),
        );
    }
}
