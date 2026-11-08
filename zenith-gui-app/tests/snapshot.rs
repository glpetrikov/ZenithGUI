//! Renders the demo UI offscreen. Skips when there is no GPU adapter.
//! `ZENITH_SNAPSHOT_DIR=/some/dir cargo test -p zenith-gui-app` also writes PNGs.

#[path = "../examples/demo/ui.rs"]
mod ui;

use zenith_gui_app::prelude::*;

fn headless(size: (f32, f32), scale: f32) -> Option<HeadlessApp> {
    match HeadlessApp::new(size, scale) {
        Ok(h) => Some(h),
        Err(e) => {
            eprintln!("skipping GPU test: {e}");
            None
        }
    }
}

fn px(img: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

#[test]
fn demo_renders_and_reacts_to_input() {
    let Some(mut h) = headless((1100.0, 640.0), 1.0) else {
        return;
    };
    ui::build(&mut h.app);
    let (w, _) = h.size_px();
    let dir = std::env::var("ZENITH_SNAPSHOT_DIR").ok();
    let path = |n: &str| {
        dir.as_ref().map(|d| {
            std::fs::create_dir_all(d).unwrap();
            format!("{d}/{n}.png")
        })
    };

    let before = h.render_rgba().unwrap();
    if let Some(p) = path("demo") {
        h.save_png(p).unwrap();
    }
    // Left panel and node editor backgrounds are different colors.
    assert_ne!(px(&before, w, 100, 300), px(&before, w, 900, 600));

    // Click the slider track at ~80%: the progress bar follows (ECS observer chain).
    let slider = h
        .app
        .world
        .query_filtered::<Entity, With<Slider>>()
        .single(&h.app.world)
        .unwrap();
    let r = h.app.world.get::<ComputedLayout>(slider).unwrap().rect;
    {
        let mut p = h.app.world.resource_mut::<PointerState>();
        p.position = Some(Vec2::new(r.pos.x + r.size.x * 0.8, r.center().y));
        p.set_button(PointerButtonState::Pressed);
    }
    h.update();
    h.app
        .world
        .resource_mut::<PointerState>()
        .set_button(PointerButtonState::Released);
    h.update();
    let bar = h
        .app
        .world
        .query::<&ProgressBar>()
        .single(&h.app.world)
        .unwrap()
        .0;
    assert!((bar - 0.8).abs() < 0.02, "progress {bar}");

    let after = h.render_rgba().unwrap();
    if let Some(p) = path("demo_after") {
        h.save_png(p).unwrap();
    }
    assert_ne!(before, after);
}

#[test]
fn hidpi_output_scales_physical_size() {
    let Some(mut h) = headless((400.0, 300.0), 2.0) else {
        return;
    };
    ui::build(&mut h.app);
    assert_eq!(h.size_px(), (800, 600));
    let img = h.render_rgba().unwrap();
    assert_eq!(img.len(), 800 * 600 * 4);
}
