use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use zenith_gui_core::prelude::*;
use zenith_gui_wgpu::{RendererConfig, WgpuRenderer};
use zenith_gui_widgets::WidgetsPlugin;

use crate::AppError;

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    /// Initial inner size, logical px.
    pub size: (f32, f32),
    pub min_size: Option<(f32, f32)>,
    pub resizable: bool,
    pub vsync: bool,
    pub clear_color: Color,
    /// Exit after this many frames (smoke tests, benchmarks).
    pub max_frames: Option<u64>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Zenith GUI".into(),
            size: (1024.0, 720.0),
            min_size: None,
            resizable: true,
            vsync: true,
            clear_color: Color::hex(0x16161d),
            max_frames: None,
        }
    }
}

type SetupFn = Box<dyn FnOnce(&mut ZenithApp)>;
type FrameFn = Box<dyn FnMut(&mut ZenithApp)>;

/// Builder for a windowed Zenith app. Consumed by [`ZenithWindow::run`].
pub struct ZenithWindow {
    config: WindowConfig,
    app: ZenithApp,
    setup: Vec<SetupFn>,
    on_frame: Vec<FrameFn>,
}

impl ZenithWindow {
    /// App with `CorePlugin` + `WidgetsPlugin` installed.
    pub fn new(title: impl Into<String>) -> Self {
        let mut app = ZenithApp::new();
        app.add_plugin(WidgetsPlugin);
        Self {
            config: WindowConfig {
                title: title.into(),
                ..Default::default()
            },
            app,
            setup: Vec::new(),
            on_frame: Vec::new(),
        }
    }

    pub fn config(mut self, config: WindowConfig) -> Self {
        self.config = config;
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.config.size = (width, height);
        self
    }

    pub fn min_size(mut self, width: f32, height: f32) -> Self {
        self.config.min_size = Some((width, height));
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.config.resizable = resizable;
        self
    }

    pub fn vsync(mut self, vsync: bool) -> Self {
        self.config.vsync = vsync;
        self
    }

    pub fn clear_color(mut self, color: Color) -> Self {
        self.config.clear_color = color;
        self
    }

    pub fn max_frames(mut self, frames: u64) -> Self {
        self.config.max_frames = Some(frames);
        self
    }

    pub fn plugin(mut self, plugin: impl Plugin) -> Self {
        self.app.add_plugin(plugin);
        self
    }

    /// Runs once, with the real text measurer already installed, before the
    /// first frame. Spawn your UI here.
    pub fn setup(mut self, f: impl FnOnce(&mut ZenithApp) + 'static) -> Self {
        self.setup.push(Box::new(f));
        self
    }

    /// Runs every frame, right before [`ZenithApp::update`].
    pub fn on_frame(mut self, f: impl FnMut(&mut ZenithApp) + 'static) -> Self {
        self.on_frame.push(Box::new(f));
        self
    }

    /// Opens the window and blocks until it is closed.
    pub fn run(self) -> Result<(), AppError> {
        let event_loop = EventLoop::new().map_err(|e| AppError::EventLoop(e.to_string()))?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut runner = Runner {
            config: self.config,
            app: self.app,
            setup: self.setup,
            on_frame: self.on_frame,
            gfx: None,
            frames: 0,
            error: None,
        };
        event_loop
            .run_app(&mut runner)
            .map_err(|e| AppError::EventLoop(e.to_string()))?;
        match runner.error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

struct Gfx {
    window: Arc<Window>,
    renderer: WgpuRenderer,
}

struct Runner {
    config: WindowConfig,
    app: ZenithApp,
    setup: Vec<SetupFn>,
    on_frame: Vec<FrameFn>,
    gfx: Option<Gfx>,
    frames: u64,
    error: Option<AppError>,
}

impl Runner {
    fn init(&mut self, el: &ActiveEventLoop) -> Result<(), AppError> {
        let mut attrs = Window::default_attributes()
            .with_title(self.config.title.clone())
            .with_inner_size(LogicalSize::new(
                self.config.size.0 as f64,
                self.config.size.1 as f64,
            ))
            .with_resizable(self.config.resizable);
        if let Some((w, h)) = self.config.min_size {
            attrs = attrs.with_min_inner_size(LogicalSize::new(w as f64, h as f64));
        }
        let window = Arc::new(
            el.create_window(attrs)
                .map_err(|e| AppError::Window(e.to_string()))?,
        );

        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(el.owned_display_handle()),
            ));
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| AppError::Window(format!("create surface: {e}")))?;
        let size = window.inner_size();
        let renderer = WgpuRenderer::new(
            &instance,
            surface,
            (size.width, size.height),
            window.scale_factor() as f32,
            RendererConfig {
                clear_color: self.config.clear_color,
                vsync: self.config.vsync,
                ..Default::default()
            },
        )?;

        self.app
            .insert_resource(TextMeasurer(renderer.text_measure()));
        self.sync_viewport(&window);
        for f in self.setup.drain(..) {
            f(&mut self.app);
        }
        window.request_redraw();
        self.gfx = Some(Gfx { window, renderer });
        Ok(())
    }

    fn sync_viewport(&mut self, window: &Window) {
        let scale = window.scale_factor() as f32;
        let size = window.inner_size();
        self.app.insert_resource(Viewport {
            size: Vec2::new(size.width as f32 / scale, size.height as f32 / scale),
            scale_factor: scale,
        });
    }

    fn fail(&mut self, el: &ActiveEventLoop, e: AppError) {
        self.error = Some(e);
        el.exit();
    }

    fn redraw(&mut self, el: &ActiveEventLoop) {
        let Some(gfx) = self.gfx.as_mut() else { return };
        let (window, renderer) = (gfx.window.clone(), &mut gfx.renderer);
        let size = window.inner_size();
        renderer.resize(size.width, size.height, window.scale_factor() as f32);
        self.sync_viewport(&window);
        for f in &mut self.on_frame {
            f(&mut self.app);
        }
        self.app.update();
        let result = self
            .gfx
            .as_mut()
            .unwrap()
            .renderer
            .render(self.app.world.resource::<DrawList>());
        if let Err(e) = result {
            return self.fail(el, e.into());
        }
        self.frames += 1;
        if self.config.max_frames.is_some_and(|m| self.frames >= m) {
            el.exit();
        } else {
            window.request_redraw();
        }
    }

    fn pointer(&mut self) -> ptr::Pointer<'_> {
        ptr::Pointer(self.app.world.resource_mut::<PointerState>())
    }
}

/// Tiny wrapper so the event handler reads naturally.
mod ptr {
    use zenith_gui_core::prelude::*;

    pub struct Pointer<'w>(pub Mut<'w, PointerState>);

    impl Pointer<'_> {
        pub fn move_to(&mut self, logical: Vec2) {
            self.0.position = Some(logical);
        }
        pub fn leave(&mut self) {
            self.0.position = None;
        }
        pub fn button(&mut self, down: bool) {
            self.0.set_button(if down {
                PointerButtonState::Pressed
            } else {
                PointerButtonState::Released
            });
        }
    }
}

impl ApplicationHandler for Runner {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_none()
            && let Err(e) = self.init(el)
        {
            self.fail(el, e);
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => self.redraw(el),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(gfx) = &self.gfx {
                    gfx.window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.gfx.as_ref().map_or(1.0, |g| g.window.scale_factor());
                let PhysicalPosition { x, y } = position;
                self.pointer()
                    .move_to(Vec2::new((x / scale) as f32, (y / scale) as f32));
            }
            WindowEvent::CursorLeft { .. } => self.pointer().leave(),
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.pointer().button(state == ElementState::Pressed);
            }
            _ => {}
        }
    }
}
