//! A small flexbox-style layout engine driven by the ECS hierarchy.
//!
//! Two passes per root: `measure` (bottom-up preferred sizes), then `place`
//! (top-down, distributes free space and writes [`ComputedLayout`]).
//! This is a skeleton: no wrapping, min/max sizes, or absolute positioning yet.

use bevy_ecs::prelude::*;
use std::collections::HashMap;

use crate::geometry::{Rect, Vec2};
use crate::input::Viewport;
use crate::node::{ComputedLayout, Node};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum Val {
    /// Size from content (leaf: [`ContentSize`], container: children).
    #[default]
    Auto,
    Px(f32),
    /// Percent (0..=100) of the parent's content box.
    Percent(f32),
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct UiRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl UiRect {
    pub const fn all(v: f32) -> Self {
        Self {
            left: v,
            top: v,
            right: v,
            bottom: v,
        }
    }

    fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    #[default]
    Column,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum JustifyContent {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AlignItems {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct Style {
    pub direction: FlexDirection,
    pub width: Val,
    pub height: Val,
    pub padding: UiRect,
    pub margin: UiRect,
    /// Space between children along the main axis.
    pub gap: f32,
    pub flex_grow: f32,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
}

/// Intrinsic size of a leaf (e.g. measured text). Widgets keep it up to date.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct ContentSize(pub Vec2);

type NodeQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Style,
        Option<&'static ContentSize>,
        Option<&'static Children>,
    ),
    With<Node>,
>;

fn is_row(s: &Style) -> bool {
    s.direction == FlexDirection::Row
}

fn main_of(v: Vec2, row: bool) -> f32 {
    if row { v.x } else { v.y }
}

fn cross_of(v: Vec2, row: bool) -> f32 {
    if row { v.y } else { v.x }
}

fn from_axes(main: f32, cross: f32, row: bool) -> Vec2 {
    if row {
        Vec2::new(main, cross)
    } else {
        Vec2::new(cross, main)
    }
}

/// Outer size (border box + margin) a node wants, ignoring percentages.
fn measure(e: Entity, nodes: &NodeQuery, cache: &mut HashMap<Entity, Vec2>) -> Vec2 {
    let Ok((style, content, children)) = nodes.get(e) else {
        return Vec2::ZERO;
    };
    let row = is_row(style);
    let mut main = 0.0f32;
    let mut cross = 0.0f32;
    let mut count = 0usize;
    for &child in children.map(|c| &**c).unwrap_or(&[]) {
        if nodes.get(child).is_err() {
            continue;
        }
        let outer = measure(child, nodes, cache);
        main += main_of(outer, row);
        cross = cross.max(cross_of(outer, row));
        count += 1;
    }
    if count > 1 {
        main += style.gap * (count - 1) as f32;
    }
    let mut inner = from_axes(main, cross, row);
    if count == 0 {
        inner = content.map_or(Vec2::ZERO, |c| c.0);
    }
    let auto = Vec2::new(
        inner.x + style.padding.horizontal(),
        inner.y + style.padding.vertical(),
    );
    let border_box = Vec2::new(
        match style.width {
            Val::Px(px) => px,
            _ => auto.x,
        },
        match style.height {
            Val::Px(px) => px,
            _ => auto.y,
        },
    );
    cache.insert(e, border_box);
    Vec2::new(
        border_box.x + style.margin.horizontal(),
        border_box.y + style.margin.vertical(),
    )
}

struct Placed {
    entity: Entity,
    rect: Rect,
}

fn place(
    e: Entity,
    rect: Rect,
    nodes: &NodeQuery,
    cache: &HashMap<Entity, Vec2>,
    out: &mut Vec<Placed>,
) {
    out.push(Placed { entity: e, rect });
    let Ok((style, _, children)) = nodes.get(e) else {
        return;
    };
    let kids: Vec<Entity> = children
        .map(|c| c.iter().filter(|k| nodes.get(*k).is_ok()).collect())
        .unwrap_or_default();
    if kids.is_empty() {
        return;
    }

    let row = is_row(style);
    let pad = style.padding;
    let inner = Rect::new(
        rect.pos.x + pad.left,
        rect.pos.y + pad.top,
        (rect.size.x - pad.horizontal()).max(0.0),
        (rect.size.y - pad.vertical()).max(0.0),
    );

    // Resolve each child's border-box size (percentages against our content box).
    let sizes: Vec<(Vec2, &Style)> = kids
        .iter()
        .map(|&k| {
            let (ks, _, _) = nodes.get(k).unwrap();
            let base = cache.get(&k).copied().unwrap_or(Vec2::ZERO);
            let w = match ks.width {
                Val::Percent(p) => inner.size.x * p / 100.0,
                _ => base.x,
            };
            let h = match ks.height {
                Val::Percent(p) => inner.size.y * p / 100.0,
                _ => base.y,
            };
            (Vec2::new(w, h), ks)
        })
        .collect();

    let outer_main = |s: &(Vec2, &Style)| {
        main_of(s.0, row)
            + if row {
                s.1.margin.horizontal()
            } else {
                s.1.margin.vertical()
            }
    };
    let gaps = style.gap * (kids.len() - 1) as f32;
    let used: f32 = sizes.iter().map(outer_main).sum::<f32>() + gaps;
    let free = main_of(inner.size, row) - used;
    let total_grow: f32 = sizes.iter().map(|s| s.1.flex_grow).sum();

    let (mut cursor, extra_gap) = if free > 0.0 && total_grow == 0.0 {
        match style.justify_content {
            JustifyContent::Start => (0.0, 0.0),
            JustifyContent::Center => (free / 2.0, 0.0),
            JustifyContent::End => (free, 0.0),
            JustifyContent::SpaceBetween if kids.len() > 1 => (0.0, free / (kids.len() - 1) as f32),
            JustifyContent::SpaceBetween => (0.0, 0.0),
        }
    } else {
        (0.0, 0.0)
    };

    for (&k, (size, ks)) in kids.iter().zip(&sizes) {
        let mut main = main_of(*size, row);
        if free > 0.0 && total_grow > 0.0 {
            main += free * ks.flex_grow / total_grow;
        }
        let (m_main_start, m_main_end, m_cross_start, m_cross_end) = if row {
            (
                ks.margin.left,
                ks.margin.right,
                ks.margin.top,
                ks.margin.bottom,
            )
        } else {
            (
                ks.margin.top,
                ks.margin.bottom,
                ks.margin.left,
                ks.margin.right,
            )
        };
        let cross_avail = cross_of(inner.size, row) - m_cross_start - m_cross_end;
        let cross_auto = if row { ks.height } else { ks.width };
        let mut cross = cross_of(*size, row);
        if style.align_items == AlignItems::Stretch && cross_auto == Val::Auto {
            cross = cross_avail.max(0.0);
        }
        let cross_off = m_cross_start
            + match style.align_items {
                AlignItems::Start | AlignItems::Stretch => 0.0,
                AlignItems::Center => (cross_avail - cross) / 2.0,
                AlignItems::End => cross_avail - cross,
            };

        cursor += m_main_start;
        let local = from_axes(cursor, cross_off, row);
        let child_rect = Rect {
            pos: Vec2::new(inner.pos.x + local.x, inner.pos.y + local.y),
            size: from_axes(main, cross, row),
        };
        place(k, child_rect, nodes, cache, out);
        cursor += main + m_main_end + style.gap + extra_gap;
    }
}

pub(crate) fn compute_layout(
    viewport: Res<Viewport>,
    roots: Query<Entity, (With<Node>, Without<ChildOf>)>,
    nodes: NodeQuery,
    mut layouts: Query<&mut ComputedLayout>,
) {
    let mut cache = HashMap::new();
    let mut placed = Vec::new();
    for root in &roots {
        measure(root, &nodes, &mut cache);
        let size = {
            let (style, _, _) = nodes.get(root).unwrap();
            let base = cache[&root];
            // Roots fill the viewport unless given an explicit size.
            Vec2::new(
                match style.width {
                    Val::Px(_) => base.x,
                    Val::Percent(p) => viewport.size.x * p / 100.0,
                    Val::Auto => viewport.size.x,
                },
                match style.height {
                    Val::Px(_) => base.y,
                    Val::Percent(p) => viewport.size.y * p / 100.0,
                    Val::Auto => viewport.size.y,
                },
            )
        };
        place(
            root,
            Rect {
                pos: Vec2::ZERO,
                size,
            },
            &nodes,
            &cache,
            &mut placed,
        );
    }
    // `placed` is in depth-first pre-order, so index == paint order.
    for (order, p) in placed.into_iter().enumerate() {
        if let Ok(mut layout) = layouts.get_mut(p.entity) {
            layout.set_if_neq(ComputedLayout {
                rect: p.rect,
                order: order as u32,
            });
        }
    }
}
