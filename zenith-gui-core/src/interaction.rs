//! Pointer hit-testing and the interaction events built on it.
//!
//! Events are [`EntityEvent`]s, so use observers (global or per-entity):
//!
//! ```
//! # use zenith_gui_core::prelude::*;
//! # let mut app = ZenithApp::new();
//! app.add_observer(|click: On<Clicked>| println!("clicked {:?}", click.entity));
//! ```

use bevy_ecs::prelude::*;

use crate::input::{PointerButtonState, PointerState};
use crate::node::{ComputedLayout, Visibility};

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Interaction {
    #[default]
    None,
    Hovered,
    Pressed,
}

/// Pointer pressed and released on the same node.
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

/// Node that received the current press (until release).
#[derive(Resource, Debug, Default)]
pub struct InteractionState {
    pub pressed: Option<Entity>,
}

pub(crate) fn update_interaction(
    pointer: Res<PointerState>,
    mut state: ResMut<InteractionState>,
    mut nodes: Query<(Entity, &ComputedLayout, &Visibility, &mut Interaction)>,
    mut commands: Commands,
) {
    // Topmost visible node under the pointer (highest paint order wins).
    let top = pointer.position.and_then(|p| {
        nodes
            .iter()
            .filter(|(_, layout, vis, _)| **vis == Visibility::Visible && layout.rect.contains(p))
            .max_by_key(|(_, layout, _, _)| layout.order)
            .map(|(e, ..)| e)
    });

    if pointer.just_pressed {
        state.pressed = top;
    }
    let held = pointer.button == PointerButtonState::Pressed;
    if pointer.just_released
        && let (Some(pressed), Some(top)) = (state.pressed, top)
        && pressed == top
    {
        commands.trigger(Clicked { entity: top });
    }
    if !held {
        state.pressed = None;
    }

    for (entity, _, _, mut interaction) in &mut nodes {
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
