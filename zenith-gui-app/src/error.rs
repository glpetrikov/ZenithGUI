use zenith_gui_core::prelude::RenderError;
use zenith_gui_wgpu::RendererInitError;

#[derive(Debug)]
pub enum AppError {
    EventLoop(String),
    Window(String),
    Renderer(RendererInitError),
    Render(RenderError),
    Io(String),
}

impl core::fmt::Display for AppError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EventLoop(m) => write!(f, "event loop error: {m}"),
            Self::Window(m) => write!(f, "window error: {m}"),
            Self::Renderer(e) => write!(f, "{e}"),
            Self::Render(e) => write!(f, "{e}"),
            Self::Io(m) => write!(f, "io error: {m}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<RendererInitError> for AppError {
    fn from(e: RendererInitError) -> Self {
        Self::Renderer(e)
    }
}

impl From<RenderError> for AppError {
    fn from(e: RenderError) -> Self {
        Self::Render(e)
    }
}
