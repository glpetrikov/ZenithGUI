//! Horizontal slider: click or drag anywhere on the track.

use zenith_gui_core::prelude::*;

use crate::theme;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(Node, BackgroundColor(theme::CANVAS), CornerRadius(4.0), ClipChildren, Border { width: 1.0, color: theme::BORDER })]
pub struct Slider {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    /// Snap to multiples of `step` (from `min`) when set.
    pub step: Option<f32>,
}

impl Slider {
    pub fn fraction(&self) -> f32 {
        if self.max > self.min {
            ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn set_from_fraction(&mut self, f: f32) {
        let mut v = self.min + f.clamp(0.0, 1.0) * (self.max - self.min);
        if let Some(step) = self.step.filter(|s| *s > 0.0) {
            v = self.min + ((v - self.min) / step).round() * step;
        }
        self.value = v.clamp(self.min, self.max);
    }
}

/// Fired (on the slider entity) whenever the user changes the value.
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct SliderChanged {
    pub entity: Entity,
    pub value: f32,
}

#[derive(Component, Debug, Default, Clone, Copy)]
#[require(Node, PassThrough, BackgroundColor(theme::ACCENT))]
struct SliderFill;

/// A slider spanning the available width. `value` is clamped into `min..=max`.
pub fn slider(value: f32, min: f32, max: f32) -> impl Bundle {
    let s = Slider {
        value: value.clamp(min, max),
        min,
        max,
        step: None,
    };
    (
        s,
        Style {
            height: Val::Px(20.0),
            ..Default::default()
        },
        children![(
            SliderFill,
            Style {
                position: Position::Absolute {
                    left: 0.0,
                    top: 0.0
                },
                width: Val::Percent(s.fraction() * 100.0),
                height: Val::Percent(100.0),
                ..Default::default()
            },
        )],
    )
}

fn apply(
    entity: Entity,
    x: f32,
    sliders: &mut Query<(&mut Slider, &ComputedLayout)>,
    commands: &mut Commands,
) {
    let Ok((mut s, layout)) = sliders.get_mut(entity) else {
        return;
    };
    let r = layout.rect;
    if r.size.x <= 0.0 {
        return;
    }
    let before = s.value;
    s.set_from_fraction((x - r.pos.x) / r.size.x);
    if s.value != before {
        commands.trigger(SliderChanged {
            entity,
            value: s.value,
        });
    }
}

fn on_press(
    ev: On<PointerPressed>,
    mut sliders: Query<(&mut Slider, &ComputedLayout)>,
    mut commands: Commands,
) {
    apply(ev.entity, ev.position.x, &mut sliders, &mut commands);
}

fn on_drag(
    ev: On<Dragged>,
    mut sliders: Query<(&mut Slider, &ComputedLayout)>,
    mut commands: Commands,
) {
    apply(ev.entity, ev.position.x, &mut sliders, &mut commands);
}

fn sync_fill(
    sliders: Query<(&Slider, &Children), Changed<Slider>>,
    mut fills: Query<&mut Style, With<SliderFill>>,
) {
    for (slider, children) in &sliders {
        for child in children.iter() {
            if let Ok(mut style) = fills.get_mut(child) {
                style.width = Val::Percent(slider.fraction() * 100.0);
            }
        }
    }
}

pub(crate) struct SliderPlugin;

impl Plugin for SliderPlugin {
    fn build(&self, app: &mut ZenithApp) {
        app.add_observer(on_press)
            .add_observer(on_drag)
            .add_systems(ZenithSchedule::Main, sync_fill.in_set(UiSet::Update));
    }
}
