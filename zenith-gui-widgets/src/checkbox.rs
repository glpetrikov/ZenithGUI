//! Checkbox with a label; click anywhere on it to toggle.

use zenith_gui_core::prelude::*;

use crate::theme;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[require(Node)]
pub struct Checkbox {
    pub checked: bool,
}

/// Fired (on the checkbox entity) after the user toggles it.
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct Toggled {
    pub entity: Entity,
    pub checked: bool,
}

#[derive(Component, Debug, Default, Clone, Copy)]
#[require(Node, PassThrough)]
struct CheckMark;

pub fn checkbox(text: impl Into<String>, checked: bool) -> impl Bundle {
    (
        Checkbox { checked },
        Style {
            direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            gap: 8.0,
            ..Default::default()
        },
        children![
            (
                Node,
                PassThrough,
                BackgroundColor(theme::CANVAS),
                CornerRadius(4.0),
                Border {
                    width: 1.0,
                    color: theme::BORDER
                },
                Style {
                    width: Val::Px(18.0),
                    height: Val::Px(18.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..Default::default()
                },
                children![(
                    CheckMark,
                    BackgroundColor(theme::ACCENT),
                    CornerRadius(2.0),
                    Style {
                        width: Val::Px(10.0),
                        height: Val::Px(10.0),
                        ..Default::default()
                    },
                )],
            ),
            label(text),
        ],
    )
}

fn on_click(ev: On<Clicked>, mut boxes: Query<&mut Checkbox>, mut commands: Commands) {
    if let Ok(mut cb) = boxes.get_mut(ev.entity) {
        cb.checked = !cb.checked;
        commands.trigger(Toggled {
            entity: ev.entity,
            checked: cb.checked,
        });
    }
}

fn sync_mark(
    mut marks: Query<(&ChildOf, &mut Visibility), With<CheckMark>>,
    parents: Query<&ChildOf>,
    boxes: Query<&Checkbox>,
) {
    for (frame, mut vis) in &mut marks {
        let checked = parents
            .get(frame.parent())
            .ok()
            .and_then(|root| boxes.get(root.parent()).ok())
            .is_some_and(|cb| cb.checked);
        vis.set_if_neq(if checked {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
}

pub(crate) struct CheckboxPlugin;

impl Plugin for CheckboxPlugin {
    fn build(&self, app: &mut ZenithApp) {
        app.add_observer(on_click)
            .add_systems(ZenithSchedule::Main, sync_mark.in_set(UiSet::Update));
    }
}
