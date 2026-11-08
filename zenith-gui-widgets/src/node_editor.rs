//! imnodes-style node graph editor.
//!
//! ```text
//! NodeEditor (clipped canvas, pans when dragged)
//! ├── GraphNode (absolute; dragged by its body/title)
//! │   ├── title bar
//! │   └── rows: [input Pin · label]      [label · Pin output]
//! └── Link { from: output Pin, to: input Pin }   (bezier, drawn by the editor)
//! ```
//!
//! Interaction: drag a node to move it, drag empty canvas to pan, drag from a
//! pin to another (opposite-kind) pin to create a [`Link`]. An input accepts a
//! single link; connecting a new one replaces the old. React to edits with
//! observers on [`LinkCreated`] / [`LinkRemoved`]; remove a link by despawning
//! its entity. Links whose pins were despawned are cleaned up automatically.

use bevy_ecs::world::CommandQueue;
use zenith_gui_core::prelude::*;

use crate::theme;

const GRID_STEP: f32 = 32.0;
const PIN_SIZE: f32 = 12.0;
/// How close (px) a dragged link must end to a pin center to snap to it.
const PIN_SNAP_RADIUS: f32 = 16.0;

/// The canvas. Spawn it with a size (or let it fill its parent), then add
/// nodes with [`spawn_graph_node`].
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(
    Node,
    ClipChildren,
    BackgroundColor(theme::CANVAS),
    CustomDraw,
    EditorView,
    LinkDrag
)]
pub struct NodeEditor;

/// Canvas pan offset in pixels (graph space → editor-local space).
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct EditorView {
    pub pan: Vec2,
}

/// In-progress link being dragged out of a pin.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct LinkDrag {
    from: Option<Entity>,
    cursor: Vec2,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[require(Node, BackgroundColor(theme::SURFACE), CornerRadius(6.0), Border { width: 1.0, color: theme::BORDER }, GraphPos)]
pub struct GraphNode {
    pub title: String,
}

/// Node position in graph space (top-left, before pan).
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct GraphPos(pub Vec2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinKind {
    Input,
    Output,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[require(Node, BackgroundColor(theme::PIN), CornerRadius(PIN_SIZE / 2.0))]
pub struct Pin {
    pub kind: PinKind,
    pub node: Entity,
    /// Index among the node's pins of the same kind.
    pub slot: usize,
    pub name: String,
}

/// A connection from an output pin to an input pin. Not a layout node.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub editor: Entity,
    pub from: Entity,
    pub to: Entity,
}

/// A link was created by the user (`entity` is the editor).
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct LinkCreated {
    pub entity: Entity,
    pub from: Entity,
    pub to: Entity,
}

/// A link was replaced by a newer one on the same input.
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct LinkRemoved {
    pub entity: Entity,
    pub from: Entity,
    pub to: Entity,
}

#[derive(Debug, Clone)]
pub struct GraphNodeDesc {
    pub title: String,
    pub pos: Vec2,
    pub width: f32,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

impl GraphNodeDesc {
    pub fn new(title: impl Into<String>, pos: Vec2) -> Self {
        Self {
            title: title.into(),
            pos,
            width: 170.0,
            inputs: vec![],
            outputs: vec![],
        }
    }

    pub fn inputs(mut self, names: &[&str]) -> Self {
        self.inputs = names.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn outputs(mut self, names: &[&str]) -> Self {
        self.outputs = names.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
}

#[derive(Debug, Clone)]
pub struct GraphNodeHandle {
    pub node: Entity,
    pub inputs: Vec<Entity>,
    pub outputs: Vec<Entity>,
}

fn pin_dot(
    commands: &mut Commands,
    node: Entity,
    kind: PinKind,
    slot: usize,
    name: &str,
) -> Entity {
    commands
        .spawn((
            Pin {
                kind,
                node,
                slot,
                name: name.to_owned(),
            },
            Style {
                width: Val::Px(PIN_SIZE),
                height: Val::Px(PIN_SIZE),
                ..Default::default()
            },
        ))
        .id()
}

fn slot_row(commands: &mut Commands, kind: PinKind, pin: Option<(Entity, &str)>) -> Entity {
    let slot = commands
        .spawn((
            Node,
            PassThrough,
            Style {
                direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                gap: 6.0,
                ..Default::default()
            },
        ))
        .id();
    if let Some((pin, name)) = pin {
        let text = commands.spawn(label(name)).id();
        match kind {
            PinKind::Input => commands.entity(slot).add_children(&[pin, text]),
            PinKind::Output => commands.entity(slot).add_children(&[text, pin]),
        };
    }
    slot
}

/// Spawns a node (title, pins, labels) under `editor` using `commands`.
pub fn spawn_graph_node(
    commands: &mut Commands,
    editor: Entity,
    desc: GraphNodeDesc,
) -> GraphNodeHandle {
    let node = commands
        .spawn((
            GraphNode {
                title: desc.title.clone(),
            },
            GraphPos(desc.pos),
            Style {
                position: Position::Absolute {
                    left: desc.pos.x,
                    top: desc.pos.y,
                },
                width: Val::Px(desc.width),
                ..Default::default()
            },
            ChildOf(editor),
        ))
        .id();

    let title = commands
        .spawn((
            Node,
            PassThrough,
            BackgroundColor(theme::SURFACE_TITLE),
            Style {
                padding: UiRect {
                    left: 10.0,
                    right: 10.0,
                    top: 6.0,
                    bottom: 6.0,
                },
                ..Default::default()
            },
            children![label(desc.title)],
        ))
        .id();
    let body = commands
        .spawn((
            Node,
            PassThrough,
            Style {
                padding: UiRect::all(8.0),
                gap: 6.0,
                ..Default::default()
            },
        ))
        .id();
    commands.entity(node).add_children(&[title, body]);

    let inputs: Vec<Entity> = desc
        .inputs
        .iter()
        .enumerate()
        .map(|(i, n)| pin_dot(commands, node, PinKind::Input, i, n))
        .collect();
    let outputs: Vec<Entity> = desc
        .outputs
        .iter()
        .enumerate()
        .map(|(i, n)| pin_dot(commands, node, PinKind::Output, i, n))
        .collect();

    for i in 0..inputs.len().max(outputs.len()) {
        let row = commands
            .spawn((
                Node,
                PassThrough,
                Style {
                    direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    gap: 16.0,
                    ..Default::default()
                },
            ))
            .id();
        let left = slot_row(
            commands,
            PinKind::Input,
            inputs.get(i).map(|&p| (p, desc.inputs[i].as_str())),
        );
        let right = slot_row(
            commands,
            PinKind::Output,
            outputs.get(i).map(|&p| (p, desc.outputs[i].as_str())),
        );
        commands.entity(row).add_children(&[left, right]);
        commands.entity(body).add_child(row);
    }
    GraphNodeHandle {
        node,
        inputs,
        outputs,
    }
}

/// [`spawn_graph_node`] for code that has a `&mut World` (setup, tests).
pub fn spawn_graph_node_in(
    world: &mut World,
    editor: Entity,
    desc: GraphNodeDesc,
) -> GraphNodeHandle {
    let mut queue = CommandQueue::default();
    let handle = spawn_graph_node(&mut Commands::new(&mut queue, world), editor, desc);
    queue.apply(world);
    handle
}

// ---------------------------------------------------------------- systems

fn editor_of(
    mut e: Entity,
    parents: &Query<&ChildOf>,
    editors: &Query<(), With<NodeEditor>>,
) -> Option<Entity> {
    loop {
        if editors.contains(e) {
            return Some(e);
        }
        e = parents.get(e).ok()?.parent();
    }
}

#[allow(clippy::too_many_arguments)]
fn on_drag(
    ev: On<Dragged>,
    mut positions: Query<&mut GraphPos>,
    pins: Query<&Pin>,
    mut views: Query<(&mut EditorView, &mut LinkDrag), With<NodeEditor>>,
    editor_marks: Query<(), With<NodeEditor>>,
    parents: Query<&ChildOf>,
) {
    if let Ok(mut pos) = positions.get_mut(ev.entity) {
        pos.0 += ev.delta;
    } else if pins.contains(ev.entity) {
        if let Some(editor) = editor_of(ev.entity, &parents, &editor_marks)
            && let Ok((_, mut drag)) = views.get_mut(editor)
        {
            drag.from = Some(ev.entity);
            drag.cursor = ev.position;
        }
    } else if let Ok((mut view, _)) = views.get_mut(ev.entity) {
        view.pan += ev.delta;
    }
}

/// Raise a pressed node above its siblings.
fn on_press(
    ev: On<PointerPressed>,
    nodes: Query<&ChildOf, With<GraphNode>>,
    mut commands: Commands,
) {
    if let Ok(parent) = nodes.get(ev.entity) {
        commands.entity(ev.entity).insert(ChildOf(parent.parent()));
    }
}

#[allow(clippy::too_many_arguments)]
fn on_drag_end(
    ev: On<DragEnded>,
    pins: Query<(Entity, &Pin, &ComputedLayout)>,
    mut drags: Query<&mut LinkDrag, With<NodeEditor>>,
    editor_marks: Query<(), With<NodeEditor>>,
    parents: Query<&ChildOf>,
    links: Query<(Entity, &Link)>,
    mut commands: Commands,
) {
    let Ok((_, start, _)) = pins.get(ev.entity) else {
        return;
    };
    let Some(editor) = editor_of(ev.entity, &parents, &editor_marks) else {
        return;
    };
    if let Ok(mut drag) = drags.get_mut(editor) {
        *drag = LinkDrag::default();
    }
    // Snap to the closest compatible pin near the release point.
    let target = pins
        .iter()
        .filter(|(e, p, _)| *e != ev.entity && p.kind != start.kind && p.node != start.node)
        .map(|(e, _, l)| {
            let c = l.rect.center();
            (e, (c.x - ev.position.x).hypot(c.y - ev.position.y))
        })
        .filter(|(_, d)| *d <= PIN_SNAP_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e);
    let Some(target) = target else { return };
    let (from, to) = match start.kind {
        PinKind::Output => (ev.entity, target),
        PinKind::Input => (target, ev.entity),
    };
    if links.iter().any(|(_, l)| l.from == from && l.to == to) {
        return;
    }
    // An input takes one link: replace whatever is there.
    for (link_entity, link) in &links {
        if link.to == to {
            commands.trigger(LinkRemoved {
                entity: link.editor,
                from: link.from,
                to: link.to,
            });
            commands.entity(link_entity).despawn();
        }
    }
    commands.spawn((Link { editor, from, to }, ChildOf(editor)));
    commands.trigger(LinkCreated {
        entity: editor,
        from,
        to,
    });
}

fn sync_node_positions(
    mut nodes: Query<(&GraphPos, &ChildOf, &mut Style), With<GraphNode>>,
    views: Query<&EditorView>,
) {
    for (pos, parent, mut style) in &mut nodes {
        let pan = views.get(parent.parent()).map_or(Vec2::ZERO, |v| v.pan);
        let wanted = Position::Absolute {
            left: pos.0.x + pan.x,
            top: pos.0.y + pan.y,
        };
        if style.position != wanted {
            style.position = wanted;
        }
    }
}

fn cleanup_dangling_links(
    links: Query<(Entity, &Link)>,
    pins: Query<(), With<Pin>>,
    mut commands: Commands,
) {
    for (e, link) in &links {
        if !pins.contains(link.from) || !pins.contains(link.to) {
            commands.entity(e).despawn();
        }
    }
}

type ChangedPin = (With<Pin>, Changed<Interaction>);

fn pin_visuals(mut pins: Query<(&Interaction, &mut BackgroundColor), ChangedPin>) {
    for (interaction, mut bg) in &mut pins {
        bg.0 = if *interaction == Interaction::None {
            theme::PIN
        } else {
            theme::ACCENT
        };
    }
}

fn link_curve(a: Vec2, b: Vec2) -> [Vec2; 4] {
    let dx = ((b.x - a.x).abs() * 0.5).max(50.0);
    [a, Vec2::new(a.x + dx, a.y), Vec2::new(b.x - dx, b.y), b]
}

fn draw_editor(
    mut editors: Query<
        (
            Entity,
            &ComputedLayout,
            &EditorView,
            &LinkDrag,
            &mut CustomDraw,
        ),
        With<NodeEditor>,
    >,
    links: Query<&Link>,
    pins: Query<(&Pin, &ComputedLayout)>,
) {
    for (editor, layout, view, drag, mut custom) in &mut editors {
        let r = layout.rect;
        let mut cmds = Vec::new();

        // Grid, scrolling with the pan offset.
        let start_x = r.pos.x + view.pan.x.rem_euclid(GRID_STEP);
        let start_y = r.pos.y + view.pan.y.rem_euclid(GRID_STEP);
        let major = |origin: f32, pan: f32, pos: f32| {
            ((pos - origin - pan) / GRID_STEP).round() as i64 % 4 == 0
        };
        let mut x = start_x;
        while x < r.pos.x + r.size.x {
            let color = if major(r.pos.x, view.pan.x, x) {
                theme::GRID_MAJOR
            } else {
                theme::GRID
            };
            cmds.push(DrawCommand::Rect {
                rect: Rect::new(x, r.pos.y, 1.0, r.size.y),
                color,
                radius: 0.0,
                border: None,
            });
            x += GRID_STEP;
        }
        let mut y = start_y;
        while y < r.pos.y + r.size.y {
            let color = if major(r.pos.y, view.pan.y, y) {
                theme::GRID_MAJOR
            } else {
                theme::GRID
            };
            cmds.push(DrawCommand::Rect {
                rect: Rect::new(r.pos.x, y, r.size.x, 1.0),
                color,
                radius: 0.0,
                border: None,
            });
            y += GRID_STEP;
        }

        for link in links.iter().filter(|l| l.editor == editor) {
            if let (Ok((_, a)), Ok((_, b))) = (pins.get(link.from), pins.get(link.to)) {
                cmds.push(DrawCommand::Bezier {
                    p: link_curve(a.rect.center(), b.rect.center()),
                    width: 2.5,
                    color: theme::ACCENT,
                });
            }
        }
        if let Some(from) = drag.from
            && let Ok((pin, a)) = pins.get(from)
        {
            let (p0, p1) = match pin.kind {
                PinKind::Output => (a.rect.center(), drag.cursor),
                PinKind::Input => (drag.cursor, a.rect.center()),
            };
            cmds.push(DrawCommand::Bezier {
                p: link_curve(p0, p1),
                width: 2.5,
                color: Color::rgba(theme::ACCENT.r, theme::ACCENT.g, theme::ACCENT.b, 0.6),
            });
        }
        if custom.0 != cmds {
            custom.0 = cmds;
        }
    }
}

pub(crate) struct NodeEditorPlugin;

impl Plugin for NodeEditorPlugin {
    fn build(&self, app: &mut ZenithApp) {
        app.add_observer(on_drag)
            .add_observer(on_press)
            .add_observer(on_drag_end)
            .add_systems(
                ZenithSchedule::Main,
                (
                    (sync_node_positions, cleanup_dangling_links, pin_visuals)
                        .in_set(UiSet::Update),
                    draw_editor.in_set(UiSet::PreRender),
                ),
            );
    }
}
