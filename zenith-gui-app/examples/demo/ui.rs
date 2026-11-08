//! Demo UI shared by the `demo` example and the snapshot test.

use zenith_gui_app::prelude::*;

#[derive(Resource)]
struct Ids {
    progress: Entity,
}

pub fn build(app: &mut ZenithApp) {
    let root = app
        .world
        .spawn((
            Node,
            BackgroundColor(Color::hex(0x16161d)),
            Style {
                direction: FlexDirection::Row,
                padding: UiRect::all(12.0),
                gap: 12.0,
                ..Default::default()
            },
        ))
        .id();

    // ---- left: controls -------------------------------------------------
    let side = app
        .world
        .spawn((
            Panel,
            BackgroundColor(Color::hex(0x20222c)),
            CornerRadius(8.0),
            Border {
                width: 1.0,
                color: Color::hex(0x34374a),
            },
            Style {
                width: Val::Px(260.0),
                padding: UiRect::all(14.0),
                gap: 12.0,
                ..Default::default()
            },
            ChildOf(root),
        ))
        .id();
    app.world.spawn((
        Label,
        Text::new("Zenith GUI").with_size(20.0),
        ChildOf(side),
    ));
    app.world.spawn((label("Volume"), ChildOf(side)));
    let volume = app
        .world
        .spawn((slider(35.0, 0.0, 100.0), ChildOf(side)))
        .id();
    app.world.spawn((label("Progress"), ChildOf(side)));
    let progress = app.world.spawn((progress_bar(0.35), ChildOf(side))).id();
    app.world
        .spawn((checkbox("Snap to grid", true), ChildOf(side)));
    app.world
        .spawn((checkbox("Show minimap", false), ChildOf(side)));
    let reset = app.world.spawn((button("Reset"), ChildOf(side))).id();

    app.insert_resource(Ids { progress });
    // Plain ECS: observers connect widgets, no callbacks stored on them.
    app.add_observer(
        move |e: On<SliderChanged>, ids: Res<Ids>, mut bars: Query<&mut ProgressBar>| {
            if e.entity == volume
                && let Ok(mut bar) = bars.get_mut(ids.progress)
            {
                bar.0 = e.value / 100.0;
            }
        },
    );
    app.add_observer(
        move |e: On<Clicked>,
              mut sliders: Query<&mut Slider>,
              ids: Res<Ids>,
              mut bars: Query<&mut ProgressBar>| {
            if e.entity == reset {
                sliders.get_mut(volume).unwrap().value = 35.0;
                bars.get_mut(ids.progress).unwrap().0 = 0.35;
            }
        },
    );

    // ---- right: node editor ----------------------------------------------
    let editor = app
        .world
        .spawn((
            NodeEditor,
            CornerRadius(8.0),
            Border {
                width: 1.0,
                color: Color::hex(0x34374a),
            },
            Style {
                flex_grow: 1.0,
                ..Default::default()
            },
            ChildOf(root),
        ))
        .id();
    let tex = spawn_graph_node_in(
        &mut app.world,
        editor,
        GraphNodeDesc::new("Texture", Vec2::new(30.0, 40.0)).outputs(&["color", "alpha"]),
    );
    let mul = spawn_graph_node_in(
        &mut app.world,
        editor,
        GraphNodeDesc::new("Multiply", Vec2::new(280.0, 110.0))
            .inputs(&["a", "b"])
            .outputs(&["result"]),
    );
    let out = spawn_graph_node_in(
        &mut app.world,
        editor,
        GraphNodeDesc::new("Output", Vec2::new(520.0, 70.0)).inputs(&["surface"]),
    );
    for (from, to) in [
        (tex.outputs[0], mul.inputs[0]),
        (tex.outputs[1], mul.inputs[1]),
        (mul.outputs[0], out.inputs[0]),
    ] {
        app.world
            .spawn((Link { editor, from, to }, ChildOf(editor)));
    }
}
