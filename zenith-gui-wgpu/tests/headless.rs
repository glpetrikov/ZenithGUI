//! Renders a real UI offscreen and checks pixels. Skips (passes) when no GPU /
//! software Vulkan adapter is available. Set `ZENITH_SNAPSHOT_DIR` to also
//! write PNGs.

use zenith_gui_core::prelude::*;
use zenith_gui_wgpu::{RendererConfig, WgpuRenderer};

fn renderer(size: (u32, u32), scale: f32) -> Option<WgpuRenderer> {
    match WgpuRenderer::new_headless(size, scale, RendererConfig::default()) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("skipping GPU test: {e}");
            None
        }
    }
}

fn save(name: &str, size: (u32, u32), rgba: &[u8]) {
    let Ok(dir) = std::env::var("ZENITH_SNAPSHOT_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let file = std::fs::File::create(format!("{dir}/{name}.png")).unwrap();
    let mut enc = png::Encoder::new(file, size.0, size.1);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

fn px(img: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn near(a: [u8; 4], b: [u8; 3], tol: i32) -> bool {
    (0..3).all(|i| (a[i] as i32 - b[i] as i32).abs() <= tol)
}

#[test]
fn renders_widgets_text_curves_and_clips() {
    let size = (400u32, 240u32);
    let Some(mut gpu) = renderer(size, 1.0) else {
        return;
    };

    let mut app = ZenithApp::new();
    app.insert_resource(Viewport {
        size: Vec2::new(400.0, 240.0),
        scale_factor: 1.0,
    });
    app.insert_resource(TextMeasurer(gpu.text_measure()));

    let root = app
        .world
        .spawn((
            Panel,
            BackgroundColor(Color::hex(0x20222c)),
            Style {
                padding: UiRect::all(16.0),
                gap: 8.0,
                ..Default::default()
            },
        ))
        .id();
    app.world
        .spawn((label("Hello, Zenith! Привет"), ChildOf(root)));
    let btn = app.world.spawn((button("Click me"), ChildOf(root))).id();
    app.world.get_mut::<Style>(btn).unwrap().width = Val::Px(160.0);
    // A clipped child that pokes out of its parent.
    let clip = app
        .world
        .spawn((
            Node,
            ClipChildren,
            BackgroundColor(Color::hex(0x103010)),
            Style {
                height: Val::Px(40.0),
                ..Default::default()
            },
            ChildOf(root),
        ))
        .id();
    app.world.spawn((
        Node,
        BackgroundColor(Color::hex(0xff0000)),
        CornerRadius(8.0),
        Style {
            position: Position::Absolute {
                left: 300.0,
                top: 10.0,
            },
            ..Style {
                width: Val::Px(200.0),
                height: Val::Px(200.0),
                ..Default::default()
            }
        },
        ChildOf(clip),
    ));
    app.world.spawn((
        Node,
        CustomDraw(vec![DrawCommand::Bezier {
            p: [
                Vec2::new(20.0, 200.0),
                Vec2::new(150.0, 120.0),
                Vec2::new(250.0, 280.0),
                Vec2::new(380.0, 200.0),
            ],
            width: 3.0,
            color: Color::hex(0x3fa9ff),
        }]),
        Style {
            position: Position::Absolute {
                left: 0.0,
                top: 0.0,
            },
            ..Default::default()
        },
        ChildOf(root),
    ));
    app.update();

    let list = app.world.resource::<DrawList>();
    gpu.resize(size.0, size.1, 1.0);
    let img = gpu.render_to_rgba(list).expect("render");
    save("widgets", size, &img);
    assert_eq!(img.len(), (size.0 * size.1 * 4) as usize);

    // Panel background.
    assert!(
        near(px(&img, size.0, 390, 5), [0x20, 0x22, 0x2c], 2),
        "panel bg: {:?}",
        px(&img, size.0, 390, 5)
    );
    // Button body (inside rounded corners), normal color.
    let r = app.world.get::<ComputedLayout>(btn).unwrap().rect;
    let p = px(
        &img,
        size.0,
        (r.pos.x + 4.0) as u32,
        (r.pos.y + r.size.y / 2.0) as u32,
    );
    assert!(near(p, [0x33, 0x34, 0x3f], 2), "button: {p:?}");
    // Text rendered: some pixel in the label row differs from the panel bg.
    let label_y = 16..32u32;
    let lit = (16..200u32).any(|x| {
        label_y
            .clone()
            .any(|y| !near(px(&img, size.0, x, y), [0x20, 0x22, 0x2c], 30))
    });
    assert!(lit, "no text pixels found");
    // Clip: red box is visible inside the clip parent, but not beyond its right edge.
    let c = app.world.get::<ComputedLayout>(clip).unwrap().rect;
    let inside = px(
        &img,
        size.0,
        (c.pos.x + c.size.x - 5.0) as u32,
        (c.pos.y + 20.0) as u32,
    );
    assert!(
        near(inside, [0xff, 0x00, 0x00], 3),
        "red inside clip: {inside:?}"
    );
    let outside = px(
        &img,
        size.0,
        (c.pos.x + c.size.x + 5.0).min(399.0) as u32,
        (c.pos.y + 20.0) as u32,
    );
    assert!(
        near(outside, [0x20, 0x22, 0x2c], 3),
        "red leaked outside clip: {outside:?}"
    );
    // Bezier endpoint region has the line color.
    let p = px(&img, size.0, 21, 200);
    assert!(p[2] > 150 && p[0] < 120, "bezier start: {p:?}");
}

#[test]
fn hidpi_scale_doubles_physical_output() {
    let Some(mut gpu) = renderer((200, 100), 2.0) else {
        return;
    };
    let list = DrawList {
        commands: vec![DrawCommand::Rect {
            rect: Rect::new(10.0, 10.0, 20.0, 20.0),
            color: Color::hex(0x00ff00),
            radius: 0.0,
            border: None,
        }],
    };
    let img = gpu.render_to_rgba(&list).unwrap();
    save("hidpi", (200, 100), &img);
    // Logical 10..30 -> physical 20..60.
    assert!(near(px(&img, 200, 40, 40), [0, 255, 0], 2));
    assert!(near(px(&img, 200, 15, 40), [0x16, 0x16, 0x1d], 2));
    assert!(near(px(&img, 200, 65, 40), [0x16, 0x16, 0x1d], 2));
}
