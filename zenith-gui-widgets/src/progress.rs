//! Read-only progress bar (`value` in `0.0..=1.0`).

use zenith_gui_core::prelude::*;

use crate::theme;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(Node, BackgroundColor(theme::CANVAS), CornerRadius(4.0), ClipChildren, Border { width: 1.0, color: theme::BORDER })]
pub struct ProgressBar(pub f32);

#[derive(Component, Debug, Default, Clone, Copy)]
#[require(Node, PassThrough, BackgroundColor(theme::ACCENT))]
struct ProgressFill;

pub fn progress_bar(value: f32) -> impl Bundle {
    (
        ProgressBar(value.clamp(0.0, 1.0)),
        Style {
            height: Val::Px(10.0),
            ..Default::default()
        },
        children![(
            ProgressFill,
            Style {
                position: Position::Absolute {
                    left: 0.0,
                    top: 0.0
                },
                width: Val::Percent(value.clamp(0.0, 1.0) * 100.0),
                height: Val::Percent(100.0),
                ..Default::default()
            },
        )],
    )
}

fn sync_fill(
    bars: Query<(&ProgressBar, &Children), Changed<ProgressBar>>,
    mut fills: Query<&mut Style, With<ProgressFill>>,
) {
    for (bar, children) in &bars {
        for child in children.iter() {
            if let Ok(mut style) = fills.get_mut(child) {
                style.width = Val::Percent(bar.0.clamp(0.0, 1.0) * 100.0);
            }
        }
    }
}

pub(crate) struct ProgressPlugin;

impl Plugin for ProgressPlugin {
    fn build(&self, app: &mut ZenithApp) {
        app.add_systems(ZenithSchedule::Main, sync_fill.in_set(UiSet::Update));
    }
}
