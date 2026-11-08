//! Headless demo: builds a small UI, simulates a click, prints what happened.
//! A real app would drive `ZenithApp::update` from a window loop instead.

use zenith_gui_core::prelude::*;
use zenith_gui_widgets::prelude::*;

fn main() {
    let mut app = ZenithApp::new();
    app.add_plugin(WidgetsPlugin);
    app.add_observer(|c: On<Clicked>| println!("clicked {:?}", c.entity));

    let root = app
        .world
        .spawn((
            Panel,
            Style {
                padding: UiRect::all(16.0),
                gap: 8.0,
                ..Default::default()
            },
        ))
        .id();
    app.world.spawn((label("Hello, Zenith"), ChildOf(root)));
    let ok = app.world.spawn((button("OK"), ChildOf(root))).id();

    app.update();
    let at = app.world.get::<ComputedLayout>(ok).unwrap().rect;
    println!("button rect: {at:?}");

    let click_pos = Vec2::new(at.pos.x + 1.0, at.pos.y + 1.0);
    {
        let mut p = app.world.resource_mut::<PointerState>();
        p.position = Some(click_pos);
        p.set_button(PointerButtonState::Pressed);
    }
    app.update();
    app.world
        .resource_mut::<PointerState>()
        .set_button(PointerButtonState::Released);
    app.update();
}
