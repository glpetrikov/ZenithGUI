//! Platform-agnostic input state. A windowing backend (winit, ...) writes
//! these resources before each frame; systems only read them.

use bevy_ecs::prelude::*;

use crate::geometry::Vec2;

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub size: Vec2,
    pub scale_factor: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            size: Vec2::new(800.0, 600.0),
            scale_factor: 1.0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PointerButtonState {
    #[default]
    Released,
    Pressed,
}

/// Primary pointer (mouse / first touch) state.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct PointerState {
    /// `None` when the pointer is outside the window.
    pub position: Option<Vec2>,
    pub button: PointerButtonState,
    /// Edge flags, valid for one frame; cleared by [`UiSet::Cleanup`](crate::app::UiSet).
    pub just_pressed: bool,
    pub just_released: bool,
}

impl PointerState {
    pub fn set_button(&mut self, state: PointerButtonState) {
        if state != self.button {
            match state {
                PointerButtonState::Pressed => self.just_pressed = true,
                PointerButtonState::Released => self.just_released = true,
            }
            self.button = state;
        }
    }
}

pub(crate) fn clear_pointer_edges(mut pointer: ResMut<PointerState>) {
    if pointer.just_pressed || pointer.just_released {
        pointer.just_pressed = false;
        pointer.just_released = false;
    }
}
