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
pub mod render;
pub mod text;
pub mod widgets;

pub mod prelude {
    pub use crate::app::{CorePlugin, Plugin, UiSet, ZenithApp, ZenithSchedule};
    pub use crate::geometry::{Color, Rect, Vec2};
    pub use crate::input::{PointerButtonState, PointerState, Viewport};
    pub use crate::interaction::{
        Clicked, DragEnded, Dragged, Interaction, PointerEntered, PointerLeft, PointerPressed,
    };
    pub use crate::layout::{
        AlignItems, ContentSize, FlexDirection, JustifyContent, Position, Style, UiRect, Val,
    };
    pub use crate::node::{
        BackgroundColor, Border, ClipChildren, ComputedLayout, CornerRadius, Node, PassThrough,
        Visibility,
    };
    pub use crate::render::{CustomDraw, DrawCommand, DrawList, RenderError, Renderer};
    pub use crate::text::{
        ApproxTextMeasure, DEFAULT_FONT_SIZE, Text, TextColor, TextMeasure, TextMeasurer,
    };
    pub use crate::widgets::{Button, ButtonColors, Label, Panel, button, label};
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

    #[test]
    fn absolute_children_leave_the_flow_and_are_clipped_for_hit_testing() {
        let mut app = ZenithApp::new();
        let root = app
            .world
            .spawn((Node, ClipChildren, sized(100.0, 100.0)))
            .id();
        let flow = app
            .world
            .spawn((Node, sized(10.0, 10.0), ChildOf(root)))
            .id();
        let abs = app
            .world
            .spawn((
                Node,
                Style {
                    position: Position::Absolute {
                        left: 90.0,
                        top: 5.0,
                    },
                    ..sized(50.0, 20.0)
                },
                ChildOf(root),
            ))
            .id();
        app.update();

        assert_eq!(rect_of(&app, flow), Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(rect_of(&app, abs), Rect::new(90.0, 5.0, 50.0, 20.0));

        // Inside the absolute child but outside the parent's clip: no hit.
        app.world.resource_mut::<PointerState>().position = Some(Vec2::new(120.0, 10.0));
        app.update();
        assert_eq!(
            *app.world.get::<Interaction>(abs).unwrap(),
            Interaction::None
        );
        // Inside both.
        app.world.resource_mut::<PointerState>().position = Some(Vec2::new(95.0, 10.0));
        app.update();
        assert_eq!(
            *app.world.get::<Interaction>(abs).unwrap(),
            Interaction::Hovered
        );
    }

    #[test]
    fn dragging_emits_dragged_then_drag_ended_and_no_click() {
        use std::sync::{Arc, Mutex};

        let mut app = ZenithApp::new();
        let a = app.world.spawn((Node, sized(50.0, 50.0))).id();
        let log = Arc::new(Mutex::new(Vec::<String>::new()));
        let l = log.clone();
        app.add_observer(move |d: On<Dragged>| {
            l.lock()
                .unwrap()
                .push(format!("drag {} {}", d.delta.x, d.entity == a))
        });
        let l = log.clone();
        app.add_observer(move |_: On<DragEnded>| l.lock().unwrap().push("end".into()));
        let l = log.clone();
        app.add_observer(move |_: On<Clicked>| l.lock().unwrap().push("click".into()));

        app.update();
        {
            let mut p = app.world.resource_mut::<PointerState>();
            p.position = Some(Vec2::new(5.0, 5.0));
            p.set_button(PointerButtonState::Pressed);
        }
        app.update();
        app.world.resource_mut::<PointerState>().position = Some(Vec2::new(15.0, 5.0));
        app.update();
        app.world.resource_mut::<PointerState>().position = Some(Vec2::new(20.0, 5.0));
        app.update();
        app.world
            .resource_mut::<PointerState>()
            .set_button(PointerButtonState::Released);
        app.update();

        assert_eq!(*log.lock().unwrap(), ["drag 10 true", "drag 5 true", "end"]);
    }

    #[test]
    fn draw_list_is_paint_ordered_with_clip_and_custom_draw() {
        let mut app = ZenithApp::new();
        let root = app
            .world
            .spawn((
                Node,
                ClipChildren,
                BackgroundColor(Color::BLACK),
                CustomDraw(vec![DrawCommand::Line {
                    a: Vec2::ZERO,
                    b: Vec2::new(1.0, 1.0),
                    width: 1.0,
                    color: Color::WHITE,
                }]),
            ))
            .id();
        app.world.spawn((button("OK"), ChildOf(root)));
        app.update();

        let kinds: Vec<&str> = app
            .world
            .resource::<DrawList>()
            .commands
            .iter()
            .map(|c| match c {
                DrawCommand::Rect { .. } => "rect",
                DrawCommand::Text { .. } => "text",
                DrawCommand::Line { .. } => "line",
                DrawCommand::PushClip(_) => "push",
                DrawCommand::PopClip => "pop",
                DrawCommand::Bezier { .. } => "bezier",
            })
            .collect();
        assert_eq!(kinds, ["rect", "push", "line", "rect", "text", "pop"]);
    }
}
