//! Batteries-included runner for Zenith GUI, in the spirit of `dear-app`:
//! open a window, wire input into the ECS world, run the frame schedule and
//! draw it with the wgpu backend.
//!
//! ```no_run
//! use zenith_gui_app::prelude::*;
//!
//! fn main() -> Result<(), AppError> {
//!     ZenithWindow::new("Hello")
//!         .size(800.0, 600.0)
//!         .setup(|app| {
//!             app.world.spawn((Panel, Style { padding: UiRect::all(16.0), ..Default::default() }));
//!         })
//!         .run()
//! }
//! ```
//!
//! [`ZenithWindow::new`] installs `CorePlugin` and `WidgetsPlugin`; add more
//! with [`ZenithWindow::plugin`]. For tests/CI without a window see
//! [`headless::HeadlessApp`].

mod error;
pub mod headless;
mod window;

pub use error::AppError;
pub use window::{WindowConfig, ZenithWindow};

pub mod prelude {
    pub use crate::headless::HeadlessApp;
    pub use crate::{AppError, WindowConfig, ZenithWindow};
    pub use zenith_gui_core::prelude::*;
    pub use zenith_gui_widgets::prelude::*;
}
