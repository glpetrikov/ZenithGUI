//! CPU half of the renderer: turns a [`DrawList`] into GPU-ready batches.
//! Pure functions, no GPU access, so it is unit-testable.

use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use zenith_gui_core::prelude::*;

/// Scissor rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl PhysRect {
    fn intersect(self, o: PhysRect) -> PhysRect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        PhysRect {
            x: x0,
            y: y0,
            w: x1.saturating_sub(x0),
            h: y1.saturating_sub(y0),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }
}

/// One rounded rectangle (instanced quad). Layout matches `shader.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct RectInstance {
    /// x, y, w, h in physical px.
    pub rect: [f32; 4],
    pub color: [f32; 4],
    pub border_color: [f32; 4],
    /// radius, border width, unused, unused.
    pub params: [f32; 4],
}

/// Triangle-list vertex for curves/lines. Layout matches `shader.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CurveVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextItem {
    /// Top-left, physical px.
    pub pos: [f32; 2],
    pub text: String,
    /// Font size in physical px.
    pub size_px: f32,
    pub color: [u8; 4],
    /// Clip, physical px (`left, top, right, bottom`).
    pub bounds: [i32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    Rects {
        range: Range<u32>,
        scissor: PhysRect,
    },
    Curves {
        range: Range<u32>,
        scissor: PhysRect,
    },
    /// Index range into [`Frame::texts`]; clipping is per item.
    Text { range: Range<usize> },
}

#[derive(Debug, Default)]
pub struct Frame {
    pub rects: Vec<RectInstance>,
    pub curve_vertices: Vec<CurveVertex>,
    pub texts: Vec<TextItem>,
    pub segments: Vec<Segment>,
}

fn rgba(c: Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}

fn rgba8(c: Color) -> [u8; 4] {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    [q(c.r), q(c.g), q(c.b), q(c.a)]
}

fn to_phys(r: Rect, scale: f32, target: (u32, u32)) -> PhysRect {
    let (tw, th) = (target.0 as f32, target.1 as f32);
    let x0 = (r.pos.x * scale).floor().clamp(0.0, tw);
    let y0 = (r.pos.y * scale).floor().clamp(0.0, th);
    let x1 = ((r.pos.x + r.size.x) * scale).ceil().clamp(0.0, tw);
    let y1 = ((r.pos.y + r.size.y) * scale).ceil().clamp(0.0, th);
    PhysRect {
        x: x0 as u32,
        y: y0 as u32,
        w: (x1 - x0) as u32,
        h: (y1 - y0) as u32,
    }
}

/// Width of the anti-aliasing fringe around lines, in physical px.
const FEATHER: f32 = 1.0;

/// Triangulates a polyline into a feathered ribbon (triangle list).
pub fn tessellate_polyline(
    points: &[[f32; 2]],
    width: f32,
    color: [f32; 4],
    out: &mut Vec<CurveVertex>,
) {
    if points.len() < 2 {
        return;
    }
    let half = (width * 0.5).max(0.5);
    // Per-point unit normals, averaged over the two adjacent segments.
    let seg_normal = |a: [f32; 2], b: [f32; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy);
        if len < 1e-6 {
            [0.0, 0.0]
        } else {
            [-dy / len, dx / len]
        }
    };
    let mut normals = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        let prev = (i > 0).then(|| seg_normal(points[i - 1], points[i]));
        let next = (i + 1 < points.len()).then(|| seg_normal(points[i], points[i + 1]));
        let n = match (prev, next) {
            (Some(a), Some(b)) => {
                let (x, y) = (a[0] + b[0], a[1] + b[1]);
                let len = x.hypot(y);
                if len < 1e-6 { a } else { [x / len, y / len] }
            }
            (Some(n), None) | (None, Some(n)) => n,
            (None, None) => [0.0, 0.0],
        };
        normals.push(n);
    }
    // Four rows per point: outer-, inner-, inner+, outer+ with alpha 0,1,1,0.
    let offsets = [
        (-(half + FEATHER), 0.0),
        (-half, 1.0),
        (half, 1.0),
        (half + FEATHER, 0.0),
    ];
    let row = |i: usize| -> [CurveVertex; 4] {
        offsets.map(|(o, a)| CurveVertex {
            pos: [
                points[i][0] + normals[i][0] * o,
                points[i][1] + normals[i][1] * o,
            ],
            color: [color[0], color[1], color[2], color[3] * a],
        })
    };
    for i in 0..points.len() - 1 {
        let (r0, r1) = (row(i), row(i + 1));
        for k in 0..3 {
            out.extend_from_slice(&[r0[k], r0[k + 1], r1[k], r1[k], r0[k + 1], r1[k + 1]]);
        }
    }
}

/// Flattens a cubic bezier into `n + 1` points.
pub fn flatten_bezier(p: [[f32; 2]; 4], n: usize) -> Vec<[f32; 2]> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            [
                a * p[0][0] + b * p[1][0] + c * p[2][0] + d * p[3][0],
                a * p[0][1] + b * p[1][1] + c * p[2][1] + d * p[3][1],
            ]
        })
        .collect()
}

pub fn build_frame(list: &DrawList, scale: f32, target: (u32, u32)) -> Frame {
    let mut frame = Frame::default();
    let full = PhysRect {
        x: 0,
        y: 0,
        w: target.0,
        h: target.1,
    };
    let mut clips = vec![full];
    let phys = |v: Vec2| [v.x * scale, v.y * scale];

    for cmd in &list.commands {
        let clip = *clips.last().unwrap();
        match cmd {
            DrawCommand::PushClip(r) => clips.push(clip.intersect(to_phys(*r, scale, target))),
            DrawCommand::PopClip => {
                if clips.len() > 1 {
                    clips.pop();
                }
            }
            _ if clip.is_empty() => {}
            DrawCommand::Rect {
                rect,
                color,
                radius,
                border,
            } => {
                let (bw, bc) = border.map_or((0.0, Color::TRANSPARENT), |(w, c)| (w * scale, c));
                let idx = frame.rects.len() as u32;
                frame.rects.push(RectInstance {
                    rect: [
                        rect.pos.x * scale,
                        rect.pos.y * scale,
                        rect.size.x * scale,
                        rect.size.y * scale,
                    ],
                    color: rgba(*color),
                    border_color: rgba(bc),
                    params: [radius * scale, bw, 0.0, 0.0],
                });
                match frame.segments.last_mut() {
                    Some(Segment::Rects { range, scissor }) if *scissor == clip => {
                        range.end = idx + 1
                    }
                    _ => frame.segments.push(Segment::Rects {
                        range: idx..idx + 1,
                        scissor: clip,
                    }),
                }
            }
            DrawCommand::Line { .. } | DrawCommand::Bezier { .. } => {
                let start = frame.curve_vertices.len() as u32;
                match cmd {
                    DrawCommand::Line { a, b, width, color } => tessellate_polyline(
                        &[phys(*a), phys(*b)],
                        width * scale,
                        rgba(*color),
                        &mut frame.curve_vertices,
                    ),
                    DrawCommand::Bezier { p, width, color } => {
                        let pts = p.map(phys);
                        let len: f32 = pts
                            .windows(2)
                            .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
                            .sum();
                        let n = ((len / 8.0) as usize).clamp(8, 96);
                        tessellate_polyline(
                            &flatten_bezier(pts, n),
                            width * scale,
                            rgba(*color),
                            &mut frame.curve_vertices,
                        );
                    }
                    _ => unreachable!(),
                }
                let end = frame.curve_vertices.len() as u32;
                match frame.segments.last_mut() {
                    Some(Segment::Curves { range, scissor }) if *scissor == clip => range.end = end,
                    _ => frame.segments.push(Segment::Curves {
                        range: start..end,
                        scissor: clip,
                    }),
                }
            }
            DrawCommand::Text {
                pos,
                text,
                size,
                color,
            } => {
                let idx = frame.texts.len();
                frame.texts.push(TextItem {
                    pos: phys(*pos),
                    text: text.clone(),
                    size_px: size * scale,
                    color: rgba8(*color),
                    bounds: [
                        clip.x as i32,
                        clip.y as i32,
                        (clip.x + clip.w) as i32,
                        (clip.y + clip.h) as i32,
                    ],
                });
                match frame.segments.last_mut() {
                    Some(Segment::Text { range }) => range.end = idx + 1,
                    _ => frame.segments.push(Segment::Text {
                        range: idx..idx + 1,
                    }),
                }
            }
        }
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(cmds: Vec<DrawCommand>) -> DrawList {
        DrawList { commands: cmds }
    }

    fn rect(x: f32) -> DrawCommand {
        DrawCommand::Rect {
            rect: Rect::new(x, 0.0, 10.0, 10.0),
            color: Color::WHITE,
            radius: 2.0,
            border: Some((1.0, Color::BLACK)),
        }
    }

    #[test]
    fn rects_merge_until_clip_or_kind_changes() {
        let f = build_frame(
            &list(vec![
                rect(0.0),
                rect(20.0),
                DrawCommand::PushClip(Rect::new(0.0, 0.0, 50.0, 50.0)),
                rect(40.0),
                DrawCommand::PopClip,
                DrawCommand::Text {
                    pos: Vec2::ZERO,
                    text: "a".into(),
                    size: 10.0,
                    color: Color::WHITE,
                },
                rect(60.0),
            ]),
            2.0,
            (400, 200),
        );
        assert_eq!(f.segments.len(), 4);
        assert!(
            matches!(&f.segments[0], Segment::Rects { range, scissor } if *range == (0..2) && scissor.w == 400)
        );
        assert!(
            matches!(&f.segments[1], Segment::Rects { range, scissor } if *range == (2..3) && scissor.w == 100)
        );
        assert!(matches!(&f.segments[2], Segment::Text { range } if *range == (0..1)));
        assert!(matches!(&f.segments[3], Segment::Rects { range, .. } if *range == (3..4)));
        // Logical -> physical.
        assert_eq!(f.rects[1].rect, [40.0, 0.0, 20.0, 20.0]);
        assert_eq!(f.rects[1].params, [4.0, 2.0, 0.0, 0.0]);
        assert_eq!(f.texts[0].size_px, 20.0);
    }

    #[test]
    fn fully_clipped_commands_are_dropped() {
        let f = build_frame(
            &list(vec![
                DrawCommand::PushClip(Rect::new(0.0, 0.0, 0.0, 10.0)),
                rect(0.0),
                DrawCommand::PopClip,
            ]),
            1.0,
            (100, 100),
        );
        assert!(f.rects.is_empty() && f.segments.is_empty());
    }

    #[test]
    fn nested_clips_intersect() {
        let f = build_frame(
            &list(vec![
                DrawCommand::PushClip(Rect::new(0.0, 0.0, 50.0, 50.0)),
                DrawCommand::PushClip(Rect::new(25.0, 25.0, 100.0, 100.0)),
                DrawCommand::Text {
                    pos: Vec2::ZERO,
                    text: "a".into(),
                    size: 10.0,
                    color: Color::WHITE,
                },
            ]),
            1.0,
            (100, 100),
        );
        assert_eq!(f.texts[0].bounds, [25, 25, 50, 50]);
    }

    #[test]
    fn polyline_makes_three_quads_per_segment() {
        let mut out = Vec::new();
        tessellate_polyline(
            &[[0.0, 0.0], [10.0, 0.0], [20.0, 0.0]],
            2.0,
            [1.0; 4],
            &mut out,
        );
        assert_eq!(out.len(), 2 * 18);
        // Outer rows fade out, inner rows are opaque.
        assert_eq!(out[0].color[3], 0.0);
        assert_eq!(out[1].color[3], 1.0);
        // Horizontal line -> vertical offsets: inner rows at y = -1 and +1.
        assert!((out[1].pos[1] - -1.0).abs() < 1e-5);
    }

    #[test]
    fn bezier_flattening_hits_endpoints() {
        let pts = flatten_bezier([[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [20.0, 10.0]], 16);
        assert_eq!(pts.first(), Some(&[0.0, 0.0]));
        assert_eq!(pts.last(), Some(&[20.0, 10.0]));
        assert_eq!(pts.len(), 17);
    }
}
