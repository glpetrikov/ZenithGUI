//! `cargo run -p zenith-gui-app --example demo`
//!
//! Drag nodes, drag the canvas to pan, drag from a pin to another pin to link.

mod ui;

use zenith_gui_app::prelude::*;

fn main() -> Result<(), AppError> {
    let mut window = ZenithWindow::new("Zenith GUI — demo")
        .size(1100.0, 640.0)
        .min_size(640.0, 400.0);
    if let Some(n) = std::env::var("ZENITH_MAX_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        window = window.max_frames(n);
    }
    window.setup(ui::build).run()
}
