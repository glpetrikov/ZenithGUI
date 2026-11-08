use std::sync::{Arc, Mutex};

use zenith_gui_core::prelude::*;
use zenith_gui_widgets::prelude::*;

fn app() -> ZenithApp {
    let mut app = ZenithApp::new();
    app.add_plugin(WidgetsPlugin);
    app
}

fn at(app: &mut ZenithApp, x: f32, y: f32) {
    app.world.resource_mut::<PointerState>().position = Some(Vec2::new(x, y));
    app.update();
}

fn press(app: &mut ZenithApp) {
    app.world
        .resource_mut::<PointerState>()
        .set_button(PointerButtonState::Pressed);
    app.update();
}

fn release(app: &mut ZenithApp) {
    app.world
        .resource_mut::<PointerState>()
        .set_button(PointerButtonState::Released);
    app.update();
}

/// Press at `from`, move in a few steps to `to`, release.
fn drag(app: &mut ZenithApp, from: Vec2, to: Vec2) {
    at(app, from.x, from.y);
    press(app);
    for i in 1..=4 {
        let t = i as f32 / 4.0;
        at(
            app,
            from.x + (to.x - from.x) * t,
            from.y + (to.y - from.y) * t,
        );
    }
    release(app);
}

/// Drag from the center of `from` to the center of `to` (+ `off`).
fn drag_between(app: &mut ZenithApp, from: Entity, to: Entity, off: Vec2) {
    let a = rect(app, from).center();
    let b = rect(app, to).center() + off;
    drag(app, a, b);
}

fn rect(app: &ZenithApp, e: Entity) -> Rect {
    app.world.get::<ComputedLayout>(e).unwrap().rect
}

#[test]
fn slider_sets_value_on_press_and_drag_and_fires_event() {
    let mut app = app();
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    app.add_observer(move |e: On<SliderChanged>| sink.lock().unwrap().push(e.value));
    let s = app.world.spawn(slider(0.0, 0.0, 100.0)).id();
    app.update();
    let r = rect(&app, s);

    drag(
        &mut app,
        Vec2::new(r.pos.x + r.size.x * 0.25, r.pos.y + 5.0),
        Vec2::new(r.pos.x + r.size.x * 0.75, r.pos.y + 5.0),
    );
    let v = app.world.get::<Slider>(s).unwrap().value;
    assert!((v - 75.0).abs() < 0.5, "value {v}");
    assert!(events.lock().unwrap().len() >= 2);

    // Dragging past the end clamps.
    drag(
        &mut app,
        Vec2::new(r.pos.x + 10.0, r.pos.y + 5.0),
        Vec2::new(r.pos.x + r.size.x + 500.0, r.pos.y + 5.0),
    );
    assert_eq!(app.world.get::<Slider>(s).unwrap().value, 100.0);

    // The fill follows the value.
    app.update();
    let fill = app.world.get::<Children>(s).unwrap()[0];
    assert_eq!(
        app.world.get::<Style>(fill).unwrap().width,
        Val::Percent(100.0)
    );
}

#[test]
fn slider_step_snaps() {
    let mut app = app();
    let s = app.world.spawn(slider(0.0, 0.0, 10.0)).id();
    app.world.get_mut::<Slider>(s).unwrap().step = Some(5.0);
    app.update();
    let r = rect(&app, s);
    at(&mut app, r.pos.x + r.size.x * 0.4, r.pos.y + 5.0);
    press(&mut app);
    release(&mut app);
    assert_eq!(app.world.get::<Slider>(s).unwrap().value, 5.0);
}

#[test]
fn checkbox_toggles_on_click_and_mark_follows() {
    let mut app = app();
    let toggles = Arc::new(Mutex::new(Vec::new()));
    let sink = toggles.clone();
    app.add_observer(move |e: On<Toggled>| sink.lock().unwrap().push(e.checked));
    let cb = app.world.spawn(checkbox("Enable", false)).id();
    app.update();
    let mark = |app: &mut ZenithApp| {
        let frame = app.world.get::<Children>(cb).unwrap()[0];
        let mark = app.world.get::<Children>(frame).unwrap()[0];
        *app.world.get::<Visibility>(mark).unwrap()
    };
    assert_eq!(mark(&mut app), Visibility::Hidden);

    let r = rect(&app, cb);
    at(&mut app, r.pos.x + 3.0, r.pos.y + 3.0);
    press(&mut app);
    release(&mut app);
    app.update();
    assert_eq!(*toggles.lock().unwrap(), [true]);
    assert_eq!(mark(&mut app), Visibility::Visible);
}

struct Graph {
    editor: Entity,
    a: GraphNodeHandle,
    b: GraphNodeHandle,
}

fn graph(app: &mut ZenithApp) -> Graph {
    let editor = app.world.spawn(NodeEditor).id();
    let a = spawn_graph_node_in(
        &mut app.world,
        editor,
        GraphNodeDesc::new("Source", Vec2::new(20.0, 20.0)).outputs(&["out", "alpha"]),
    );
    let b = spawn_graph_node_in(
        &mut app.world,
        editor,
        GraphNodeDesc::new("Sink", Vec2::new(400.0, 120.0)).inputs(&["in", "mask"]),
    );
    app.update();
    Graph { editor, a, b }
}

#[test]
fn node_editor_builds_pins_in_layout() {
    let mut app = app();
    let g = graph(&mut app);
    let pin = rect(&app, g.a.outputs[0]);
    let node = rect(&app, g.a.node);
    assert_eq!(node.pos, Vec2::new(20.0, 20.0));
    assert_eq!(node.size.x, 170.0);
    // Output pins sit at the node's right edge (inside the padding), inputs at the left.
    assert!(
        pin.pos.x + pin.size.x <= node.pos.x + node.size.x
            && pin.pos.x > node.pos.x + node.size.x - 30.0
    );
    let in_pin = rect(&app, g.b.inputs[0]);
    assert!(in_pin.pos.x < rect(&app, g.b.node).pos.x + 30.0);
    // Two rows -> second pin is below the first.
    assert!(rect(&app, g.a.outputs[1]).pos.y > pin.pos.y);
}

#[test]
fn dragging_a_node_moves_it_and_panning_moves_everything() {
    let mut app = app();
    let g = graph(&mut app);
    let n = rect(&app, g.a.node);
    // Grab the node's title bar (a pass-through child -> hits the node itself).
    drag(
        &mut app,
        Vec2::new(n.pos.x + 60.0, n.pos.y + 8.0),
        Vec2::new(n.pos.x + 110.0, n.pos.y + 48.0),
    );
    app.update();
    assert_eq!(
        app.world.get::<GraphPos>(g.a.node).unwrap().0,
        Vec2::new(70.0, 60.0)
    );
    assert_eq!(rect(&app, g.a.node).pos, Vec2::new(70.0, 60.0));

    // Empty canvas drag pans the whole view.
    drag(&mut app, Vec2::new(300.0, 500.0), Vec2::new(340.0, 470.0));
    app.update();
    assert_eq!(
        app.world.get::<EditorView>(g.editor).unwrap().pan,
        Vec2::new(40.0, -30.0)
    );
    assert_eq!(rect(&app, g.a.node).pos, Vec2::new(110.0, 30.0));
    assert_eq!(rect(&app, g.b.node).pos, Vec2::new(440.0, 90.0));
}

#[test]
fn dragging_from_pin_to_pin_creates_link_and_replaces_on_same_input() {
    let mut app = app();
    let created = Arc::new(Mutex::new(Vec::new()));
    let removed = Arc::new(Mutex::new(0));
    let (c, r) = (created.clone(), removed.clone());
    app.add_observer(move |e: On<LinkCreated>| c.lock().unwrap().push((e.from, e.to)));
    app.add_observer(move |_: On<LinkRemoved>| *r.lock().unwrap() += 1);
    let g = graph(&mut app);

    let center = |app: &ZenithApp, e| rect(app, e).center();
    let (out0, out1, in0) = (g.a.outputs[0], g.a.outputs[1], g.b.inputs[0]);

    // Release a bit off-center: should still snap.
    drag_between(&mut app, out0, in0, Vec2::new(5.0, 4.0));
    assert_eq!(*created.lock().unwrap(), [(out0, in0)]);
    let links = |app: &mut ZenithApp| {
        app.world
            .query::<&Link>()
            .iter(&app.world)
            .copied()
            .collect::<Vec<_>>()
    };
    assert_eq!(links(&mut app).len(), 1);

    // Same pair again: no duplicate.
    drag_between(&mut app, out0, in0, Vec2::ZERO);
    assert_eq!(links(&mut app).len(), 1);

    // Reverse direction (input -> output) into an occupied input replaces the old link.
    drag_between(&mut app, in0, out1, Vec2::ZERO);
    app.update();
    let ls = links(&mut app);
    assert_eq!(ls.len(), 1);
    assert_eq!((ls[0].from, ls[0].to), (out1, in0));
    assert_eq!(*removed.lock().unwrap(), 1);

    // Dropping on empty space or an incompatible pin creates nothing.
    let from = center(&app, out0);
    drag(&mut app, from, Vec2::new(700.0, 500.0));
    drag_between(&mut app, out0, out1, Vec2::ZERO);
    assert_eq!(links(&mut app).len(), 1);

    // The link is drawn as a bezier between the two pin centers.
    app.update();
    let custom = app.world.get::<CustomDraw>(g.editor).unwrap();
    let curve = custom.0.iter().find_map(|c| match c {
        DrawCommand::Bezier { p, .. } => Some(*p),
        _ => None,
    });
    let p = curve.expect("link bezier");
    assert_eq!(p[0], center(&app, out1));
    assert_eq!(p[3], center(&app, in0));
}

#[test]
fn links_to_despawned_pins_are_cleaned_up_and_pressed_node_is_raised() {
    let mut app = app();
    let g = graph(&mut app);
    drag_between(&mut app, g.a.outputs[0], g.b.inputs[1], Vec2::ZERO);
    assert_eq!(app.world.query::<&Link>().iter(&app.world).count(), 1);

    // Pressing node `a` moves it to the end of the editor's children, exactly once.
    let n = rect(&app, g.a.node);
    at(&mut app, n.pos.x + 60.0, n.pos.y + 8.0);
    press(&mut app);
    release(&mut app);
    let kids: Vec<Entity> = app
        .world
        .get::<Children>(g.editor)
        .unwrap()
        .iter()
        .collect();
    assert_eq!(kids.iter().filter(|e| **e == g.a.node).count(), 1);
    assert!(kids.iter().position(|e| *e == g.a.node) > kids.iter().position(|e| *e == g.b.node));

    app.world.entity_mut(g.b.node).despawn();
    app.update();
    app.update();
    assert_eq!(app.world.query::<&Link>().iter(&app.world).count(), 0);
}
