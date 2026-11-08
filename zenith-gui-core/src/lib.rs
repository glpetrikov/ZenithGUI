//! Zenith GUI core: ECS-driven UI primitives, layout and interaction.
//!
//! The UI tree is a regular [`bevy_ecs`] world: every UI element is an entity
//! with a [`Node`], parent/child links use bevy's [`ChildOf`]/[`Children`]
//! relationship, and every frame is a [`bevy_ecs::schedule::Schedule`] split
//! into ordered [`UiSet`]s.
//!
//! ```
//! use zenith_gui_core::prelude::*;
//!
//! let mut app = ZenithApp::new();
//! app.world.spawn((
//!     Node,
//!     Style { width: Val::Px(100.0), height: Val::Px(40.0), ..Default::default() },
//! ));
//! app.update();
//! ```

pub mod app;
pub mod geometry;
pub mod input;
pub mod interaction;
pub mod layout;
pub mod node;
pub mod text;

pub mod prelude {
    pub use crate::app::{CorePlugin, Plugin, UiSet, ZenithApp, ZenithSchedule};
    pub use crate::geometry::{Color, Rect, Vec2};
    pub use crate::input::{PointerButtonState, PointerState, Viewport};
    pub use crate::interaction::{Clicked, Interaction, PointerEntered, PointerLeft};
    pub use crate::layout::{
        AlignItems, ContentSize, FlexDirection, JustifyContent, Style, UiRect, Val,
    };
    pub use crate::node::{BackgroundColor, ComputedLayout, Node, Visibility};
    pub use crate::text::{Text, TextColor};
    pub use bevy_ecs::prelude::*;
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    fn sized(w: f32, h: f32) -> Style {
        Style {
            width: Val::Px(w),
            height: Val::Px(h),
            ..Default::default()
        }
    }

    fn rect_of(app: &ZenithApp, e: Entity) -> Rect {
        app.world.get::<ComputedLayout>(e).unwrap().rect
    }

    #[test]
    fn column_stacks_children_with_gap_and_padding() {
        let mut app = ZenithApp::new();
        let root = app
            .world
            .spawn((
                Node,
                Style {
                    padding: UiRect::all(10.0),
                    gap: 4.0,
                    ..Default::default()
                },
            ))
            .id();
        let a = app
            .world
            .spawn((Node, sized(50.0, 20.0), ChildOf(root)))
            .id();
        let b = app
            .world
            .spawn((Node, sized(30.0, 20.0), ChildOf(root)))
            .id();
        app.update();

        assert_eq!(rect_of(&app, root), Rect::new(0.0, 0.0, 800.0, 600.0));
        // align_items defaults to Stretch, but explicit widths are kept.
        assert_eq!(rect_of(&app, a), Rect::new(10.0, 10.0, 50.0, 20.0));
        assert_eq!(rect_of(&app, b), Rect::new(10.0, 34.0, 30.0, 20.0));
    }

    #[test]
    fn flex_grow_splits_free_space_in_a_row() {
        let mut app = ZenithApp::new();
        let root = app
            .world
            .spawn((
                Node,
                Style {
                    direction: FlexDirection::Row,
                    ..Default::default()
                },
            ))
            .id();
        let fixed = app
            .world
            .spawn((Node, sized(200.0, 10.0), ChildOf(root)))
            .id();
        let grow = app
            .world
            .spawn((
                Node,
                Style {
                    flex_grow: 1.0,
                    height: Val::Px(10.0),
                    ..Default::default()
                },
                ChildOf(root),
            ))
            .id();
        app.update();

        assert_eq!(rect_of(&app, fixed).size.x, 200.0);
        assert_eq!(rect_of(&app, grow).pos.x, 200.0);
        assert_eq!(rect_of(&app, grow).size.x, 600.0);
    }

    #[test]
    fn click_hits_topmost_node_and_fires_event() {
        use std::sync::{Arc, Mutex};

        let mut app = ZenithApp::new();
        let root = app.world.spawn(Node).id();
        let child = app
            .world
            .spawn((Node, sized(100.0, 100.0), ChildOf(root)))
            .id();
        let clicks = Arc::new(Mutex::new(Vec::new()));
        let sink = clicks.clone();
        app.add_observer(move |c: On<Clicked>| sink.lock().unwrap().push(c.entity));

        app.update(); // first layout
        {
            let mut p = app.world.resource_mut::<PointerState>();
            p.position = Some(Vec2::new(10.0, 10.0));
            p.set_button(PointerButtonState::Pressed);
        }
        app.update();
        assert_eq!(
            *app.world.get::<Interaction>(child).unwrap(),
            Interaction::Pressed
        );
        app.world
            .resource_mut::<PointerState>()
            .set_button(PointerButtonState::Released);
        app.update();

        assert_eq!(*clicks.lock().unwrap(), vec![child]);
        assert_eq!(
            *app.world.get::<Interaction>(child).unwrap(),
            Interaction::Hovered
        );
        assert_eq!(
            *app.world.get::<Interaction>(root).unwrap(),
            Interaction::None
        );
    }
}
