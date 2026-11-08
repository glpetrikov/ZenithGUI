//! Pointer hit-testing and the interaction events built on it.
//!
//! Events are [`EntityEvent`]s targeting the node the pointer *pressed on*
//! (drags keep targeting it even when the pointer leaves). Use observers:
//!
//! ```
//! # use zenith_gui_core::prelude::*;
//! # let mut app = ZenithApp::new();
//! app.add_observer(|click: On<Clicked>| println!("clicked {:?}", click.entity));
//! ```

use bevy_ecs::prelude::*;

use crate::geometry::Vec2;
use crate::input::{PointerButtonState, PointerState};
use crate::node::{ComputedLayout, PassThrough};

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Interaction {
    #[default]
    None,
    Hovered,
    Pressed,
}

/// Pointer pressed and released on the same node (without dragging).
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct Clicked {
    pub entity: Entity,
}

#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct PointerEntered {
    pub entity: Entity,
}

#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct PointerLeft {
    pub entity: Entity,
}

/// Button went down on `entity`.
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct PointerPressed {
    pub entity: Entity,
    pub position: Vec2,
}

/// The pointer moved while held, after exceeding [`DRAG_THRESHOLD`].
/// `over` is the node currently under the pointer (a drop target candidate).
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct Dragged {
    pub entity: Entity,
    /// Movement since the previous `Dragged` (or since the press).
    pub delta: Vec2,
    pub position: Vec2,
    pub over: Option<Entity>,
}

/// Button released after a drag started on `entity`.
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct DragEnded {
    pub entity: Entity,
    pub position: Vec2,
    pub over: Option<Entity>,
}

/// Pointer travel (logical px) before a press becomes a drag.
pub const DRAG_THRESHOLD: f32 = 3.0;

#[derive(Resource, Debug, Default)]
pub struct InteractionState {
    /// Node that received the current press (until release).
    pub pressed: Option<Entity>,
    pub hovered: Option<Entity>,
    press_origin: Vec2,
    last_drag_pos: Vec2,
    dragging: bool,
}

type HitNode = (
    Entity,
    &'static ComputedLayout,
    &'static mut Interaction,
    Has<PassThrough>,
);

/// Topmost visible, non-pass-through node containing `p`.
pub fn hit_test<'a>(
    nodes: impl Iterator<Item = (Entity, &'a ComputedLayout, bool)>,
    p: Vec2,
) -> Option<Entity> {
    nodes
        .filter(|(_, l, pass)| {
            !pass && l.visible && l.rect.contains(p) && l.clip.is_none_or(|c| c.contains(p))
        })
        .max_by_key(|(_, l, _)| l.order)
        .map(|(e, ..)| e)
}

pub(crate) fn update_interaction(
    pointer: Res<PointerState>,
    mut state: ResMut<InteractionState>,
    mut nodes: Query<HitNode>,
    mut commands: Commands,
) {
    let top = pointer
        .position
        .and_then(|p| hit_test(nodes.iter().map(|(e, l, _, pass)| (e, l, pass)), p));
    state.hovered = top;
    let held = pointer.button == PointerButtonState::Pressed;

    if pointer.just_pressed {
        state.pressed = top;
        state.dragging = false;
        if let (Some(e), Some(p)) = (top, pointer.position) {
            state.press_origin = p;
            state.last_drag_pos = p;
            commands.trigger(PointerPressed {
                entity: e,
                position: p,
            });
        }
    }

    if let (Some(pressed), Some(p), true) = (state.pressed, pointer.position, held) {
        if !state.dragging {
            let d = p - state.press_origin;
            state.dragging = d.x.hypot(d.y) >= DRAG_THRESHOLD;
        }
        if state.dragging && p != state.last_drag_pos {
            let delta = p - state.last_drag_pos;
            state.last_drag_pos = p;
            commands.trigger(Dragged {
                entity: pressed,
                delta,
                position: p,
                over: top,
            });
        }
    }

    if pointer.just_released
        && let Some(pressed) = state.pressed
    {
        if state.dragging {
            let position = pointer.position.unwrap_or(state.last_drag_pos);
            commands.trigger(DragEnded {
                entity: pressed,
                position,
                over: top,
            });
        } else if top == Some(pressed) {
            commands.trigger(Clicked { entity: pressed });
        }
    }
    if !held {
        state.pressed = None;
        state.dragging = false;
    }

    for (entity, _, mut interaction, _) in &mut nodes {
        let next = if held && state.pressed == Some(entity) {
            Interaction::Pressed
        } else if top == Some(entity) {
            Interaction::Hovered
        } else {
            Interaction::None
        };
        if *interaction == next {
            continue;
        }
        let was_hovered = *interaction != Interaction::None;
        let is_hovered = next != Interaction::None;
        *interaction = next;
        if !was_hovered && is_hovered {
            commands.trigger(PointerEntered { entity });
        } else if was_hovered && !is_hovered {
            commands.trigger(PointerLeft { entity });
        }
    }
}
