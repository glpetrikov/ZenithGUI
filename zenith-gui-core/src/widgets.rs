//! The minimal built-in widgets: [`Panel`], [`Label`] and [`Button`].
//! Richer widgets live in `zenith-gui-widgets`.

use bevy_ecs::prelude::*;

use crate::geometry::Color;
use crate::interaction::Interaction;
use crate::layout::{AlignItems, ContentSize, Style, UiRect};
use crate::node::{BackgroundColor, CornerRadius, Node, PassThrough};
use crate::text::{Text, TextColor};

/// Plain container with a background.
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(Node, BackgroundColor)]
pub struct Panel;

/// Single run of text; size comes from [`Text`] via the
/// [`TextMeasurer`](crate::text::TextMeasurer). Not hit-testable.
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(Node, Text, TextColor, ContentSize, PassThrough)]
pub struct Label;

/// Clickable container; listen for [`Clicked`](crate::interaction::Clicked)
/// on it. Colors follow [`Interaction`].
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(Node, BackgroundColor, ButtonColors, CornerRadius(4.0))]
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
            normal: Color::hex(0x33343f),
            hovered: Color::hex(0x44465a),
            pressed: Color::hex(0x24252e),
        }
    }
}

/// A [`Label`] bundle.
pub fn label(text: impl Into<String>) -> impl Bundle {
    (Label, Text::new(text))
}

/// A [`Button`] with a centered [`Label`] child.
pub fn button(text: impl Into<String>) -> impl Bundle {
    (
        Button,
        Style {
            padding: UiRect {
                left: 12.0,
                right: 12.0,
                top: 6.0,
                bottom: 6.0,
            },
            align_items: AlignItems::Center,
            ..Default::default()
        },
        children![label(text)],
    )
}

type DirtyButtons = (With<Button>, Or<(Changed<Interaction>, Added<Button>)>);

pub(crate) fn button_visuals(
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
