use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bytemuck::cast_slice;
use glyphon::{
    Cache, ColorMode, FontSystem, Resolution, SwashCache, TextArea, TextAtlas, TextBounds,
    TextRenderer, Viewport,
};
use wgpu::util::DeviceExt;
use zenith_gui_core::prelude::*;

use crate::batch::{CurveVertex, Frame, PhysRect, RectInstance, Segment, TextItem, build_frame};
use crate::text::{FontMeasure, SharedFonts, shape};

#[derive(Debug, Clone)]
pub struct RendererConfig {
    pub clear_color: Color,
    /// `true` = Fifo (vsync), `false` = lowest-latency mode available.
    pub vsync: bool,
    pub power_preference: wgpu::PowerPreference,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            clear_color: Color::hex(0x16161d),
            vsync: true,
            power_preference: wgpu::PowerPreference::HighPerformance,
        }
    }
}

#[derive(Debug)]
pub enum RendererInitError {
    NoAdapter(String),
    Device(String),
    Surface(String),
}

impl core::fmt::Display for RendererInitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoAdapter(m) => write!(f, "no suitable GPU adapter: {m}"),
            Self::Device(m) => write!(f, "failed to create device: {m}"),
            Self::Surface(m) => write!(f, "surface error: {m}"),
        }
    }
}

impl std::error::Error for RendererInitError {}

enum Target {
    Surface {
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
    },
    Offscreen {
        texture: wgpu::Texture,
    },
}

/// Growable GPU buffer rewritten every frame.
struct DynBuffer {
    buffer: wgpu::Buffer,
    capacity: u64,
    usage: wgpu::BufferUsages,
}

impl DynBuffer {
    fn new(device: &wgpu::Device, usage: wgpu::BufferUsages, label: &str) -> Self {
        let capacity = 4096;
        Self {
            buffer: Self::alloc(device, capacity, usage, label),
            capacity,
            usage,
        }
    }

    fn alloc(
        device: &wgpu::Device,
        size: u64,
        usage: wgpu::BufferUsages,
        label: &str,
    ) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) {
        let need = (data.len() as u64).next_power_of_two().max(256);
        if need > self.capacity {
            self.buffer = Self::alloc(device, need, self.usage, "zenith dyn buffer");
            self.capacity = need;
        }
        if !data.is_empty() {
            queue.write_buffer(&self.buffer, 0, data);
        }
    }
}

struct CachedText {
    buffer: glyphon::Buffer,
    last_used: u64,
}

pub struct WgpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: Target,
    format: wgpu::TextureFormat,
    size_px: (u32, u32),
    scale: f32,
    clear: wgpu::Color,

    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    rect_pipeline: wgpu::RenderPipeline,
    curve_pipeline: wgpu::RenderPipeline,
    rect_buf: DynBuffer,
    curve_buf: DynBuffer,

    fonts: SharedFonts,
    measure: Arc<FontMeasure>,
    swash: SwashCache,
    atlas: TextAtlas,
    glyph_viewport: Viewport,
    text_renderers: Vec<TextRenderer>,
    text_cache: HashMap<(String, u32), CachedText>,
    frame_no: u64,
}

impl WgpuRenderer {
    /// Renderer presenting to a window surface. `instance` must be the one the
    /// surface was created from.
    pub fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        size_px: (u32, u32),
        scale_factor: f32,
        config: RendererConfig,
    ) -> Result<Self, RendererInitError> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: config.power_preference,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|e| RendererInitError::NoAdapter(e.to_string()))?;

        let caps = surface.get_capabilities(&adapter);
        // Prefer a non-sRGB format: colors are authored as sRGB values and blended
        // "web style", like most UI toolkits.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| RendererInitError::Surface("surface supports no formats".into()))?;
        let present_mode = if config.vsync {
            wgpu::PresentMode::Fifo
        } else if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else if caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else {
            wgpu::PresentMode::Fifo
        };
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size_px.0.max(1),
            height: size_px.1.max(1),
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            color_space: Default::default(),
        };
        let (device, queue) = Self::request_device(&adapter)?;
        surface.configure(&device, &surface_config);
        Ok(Self::build(
            device,
            queue,
            Target::Surface {
                surface,
                config: surface_config,
            },
            format,
            size_px,
            scale_factor,
            &config,
        ))
    }

    /// Renderer drawing into an offscreen RGBA8 texture (no window needed).
    pub fn new_headless(
        size_px: (u32, u32),
        scale_factor: f32,
        config: RendererConfig,
    ) -> Result<Self, RendererInitError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: config.power_preference,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .map_err(|e| RendererInitError::NoAdapter(e.to_string()))?;
        let (device, queue) = Self::request_device(&adapter)?;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = Self::offscreen_texture(&device, size_px, format);
        Ok(Self::build(
            device,
            queue,
            Target::Offscreen { texture },
            format,
            size_px,
            scale_factor,
            &config,
        ))
    }

    fn request_device(
        adapter: &wgpu::Adapter,
    ) -> Result<(wgpu::Device, wgpu::Queue), RendererInitError> {
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("zenith device"),
            ..Default::default()
        }))
        .map_err(|e| RendererInitError::Device(e.to_string()))
    }

    fn offscreen_texture(
        device: &wgpu::Device,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("zenith offscreen"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    fn build(
        device: wgpu::Device,
        queue: wgpu::Queue,
        target: Target,
        format: wgpu::TextureFormat,
        size_px: (u32, u32),
        scale: f32,
        config: &RendererConfig,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("zenith shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let globals = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("zenith globals"),
            contents: cast_slice(&[0.0f32; 4]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("zenith globals layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("zenith globals bind"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("zenith pipeline layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });

        let make_pipeline = |label: &str,
                             vs: &str,
                             fs: &str,
                             buffers: &[Option<wgpu::VertexBufferLayout>]|
         -> wgpu::RenderPipeline {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let rect_attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
        let rect_pipeline = make_pipeline(
            "zenith rects",
            "vs_rect",
            "fs_rect",
            &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<RectInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &rect_attrs,
            })],
        );
        let curve_attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
        let curve_pipeline = make_pipeline(
            "zenith curves",
            "vs_curve",
            "fs_curve",
            &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<CurveVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &curve_attrs,
            })],
        );

        let fonts: SharedFonts = Arc::new(Mutex::new(FontSystem::new()));
        let glyph_cache = Cache::new(&device);
        let color_mode = if format.is_srgb() {
            ColorMode::Accurate
        } else {
            ColorMode::Web
        };
        let atlas = TextAtlas::with_color_mode(&device, &queue, &glyph_cache, format, color_mode);
        let glyph_viewport = Viewport::new(&device, &glyph_cache);

        let rect_buf = DynBuffer::new(&device, wgpu::BufferUsages::VERTEX, "zenith rects");
        let curve_buf = DynBuffer::new(&device, wgpu::BufferUsages::VERTEX, "zenith curves");
        let c = config.clear_color;
        Self {
            measure: Arc::new(FontMeasure::new(fonts.clone())),
            fonts,
            device,
            queue,
            target,
            format,
            size_px,
            scale,
            clear: wgpu::Color {
                r: c.r as f64,
                g: c.g as f64,
                b: c.b as f64,
                a: c.a as f64,
            },
            globals,
            globals_bind,
            rect_pipeline,
            curve_pipeline,
            rect_buf,
            curve_buf,
            swash: SwashCache::new(),
            atlas,
            glyph_viewport,
            text_renderers: Vec::new(),
            text_cache: HashMap::new(),
            frame_no: 0,
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Draws one frame into `view` (any target of `self.format` and `size_px`).
    fn draw(&mut self, list: &DrawList, view: &wgpu::TextureView) -> Result<(), RenderError> {
        self.frame_no += 1;
        let frame: Frame = build_frame(list, self.scale, self.size_px);

        let globals = [
            self.size_px.0 as f32,
            self.size_px.1 as f32,
            if self.format.is_srgb() { 1.0 } else { 0.0 },
            0.0,
        ];
        self.queue
            .write_buffer(&self.globals, 0, cast_slice(&globals));
        self.rect_buf
            .write(&self.device, &self.queue, cast_slice(&frame.rects));
        self.curve_buf
            .write(&self.device, &self.queue, cast_slice(&frame.curve_vertices));
        self.glyph_viewport.update(
            &self.queue,
            Resolution {
                width: self.size_px.0,
                height: self.size_px.1,
            },
        );

        // One glyphon TextRenderer per text run, so text interleaves correctly
        // with shapes drawn before/after it.
        let text_runs: Vec<&[TextItem]> = frame
            .segments
            .iter()
            .filter_map(|s| match s {
                Segment::Text { range } => Some(&frame.texts[range.clone()]),
                _ => None,
            })
            .collect();
        while self.text_renderers.len() < text_runs.len() {
            self.text_renderers.push(TextRenderer::new(
                &mut self.atlas,
                &self.device,
                wgpu::MultisampleState::default(),
                None,
            ));
        }
        {
            let mut fonts = self.fonts.lock().unwrap();
            for item in &frame.texts {
                let key = (item.text.clone(), item.size_px.to_bits());
                let entry = self.text_cache.entry(key).or_insert_with(|| {
                    let mut buffer = glyphon::Buffer::new_empty(crate::text::metrics(item.size_px));
                    shape(&mut fonts, &mut buffer, &item.text, item.size_px);
                    CachedText {
                        buffer,
                        last_used: 0,
                    }
                });
                entry.last_used = self.frame_no;
            }
            for (renderer, run) in self.text_renderers.iter_mut().zip(&text_runs) {
                let areas = run.iter().map(|item| TextArea {
                    buffer: &self.text_cache[&(item.text.clone(), item.size_px.to_bits())].buffer,
                    left: item.pos[0],
                    top: item.pos[1],
                    scale: 1.0,
                    bounds: TextBounds {
                        left: item.bounds[0],
                        top: item.bounds[1],
                        right: item.bounds[2],
                        bottom: item.bounds[3],
                    },
                    default_color: glyphon::Color::rgba(
                        item.color[0],
                        item.color[1],
                        item.color[2],
                        item.color[3],
                    ),
                    custom_glyphs: &[],
                });
                renderer
                    .prepare(
                        &self.device,
                        &self.queue,
                        &mut fonts,
                        &mut self.atlas,
                        &self.glyph_viewport,
                        areas,
                        &mut self.swash,
                    )
                    .map_err(|e| RenderError::Other(format!("text prepare: {e}")))?;
            }
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("zenith frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("zenith pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let full = PhysRect {
                x: 0,
                y: 0,
                w: self.size_px.0,
                h: self.size_px.1,
            };
            let set_scissor = |pass: &mut wgpu::RenderPass, r: PhysRect| {
                pass.set_scissor_rect(r.x, r.y, r.w, r.h)
            };
            let mut text_i = 0;
            for seg in &frame.segments {
                match seg {
                    Segment::Rects { range, scissor } => {
                        set_scissor(&mut pass, *scissor);
                        // glyphon rebinds group 0 for text, so always rebind ours.
                        pass.set_bind_group(0, &self.globals_bind, &[]);
                        pass.set_pipeline(&self.rect_pipeline);
                        pass.set_vertex_buffer(0, self.rect_buf.buffer.slice(..));
                        pass.draw(0..6, range.clone());
                    }
                    Segment::Curves { range, scissor } => {
                        set_scissor(&mut pass, *scissor);
                        pass.set_bind_group(0, &self.globals_bind, &[]);
                        pass.set_pipeline(&self.curve_pipeline);
                        pass.set_vertex_buffer(0, self.curve_buf.buffer.slice(..));
                        pass.draw(range.clone(), 0..1);
                    }
                    Segment::Text { .. } => {
                        set_scissor(&mut pass, full);
                        self.text_renderers[text_i]
                            .render(&self.atlas, &self.glyph_viewport, &mut pass)
                            .map_err(|e| RenderError::Other(format!("text render: {e}")))?;
                        text_i += 1;
                    }
                }
            }
        }
        self.queue.submit([encoder.finish()]);
        self.atlas.trim();
        let frame_no = self.frame_no;
        self.text_cache.retain(|_, c| c.last_used + 1 >= frame_no);
        Ok(())
    }

    /// Headless only: renders `list` and returns tightly packed RGBA8 pixels
    /// (`width * height * 4`, physical size).
    pub fn render_to_rgba(&mut self, list: &DrawList) -> Result<Vec<u8>, RenderError> {
        let Target::Offscreen { texture } = &self.target else {
            return Err(RenderError::Other(
                "render_to_rgba needs a headless renderer".into(),
            ));
        };
        let texture = texture.clone();
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.draw(list, &view)?;

        let (w, h) = self.size_px;
        let unpadded = w * 4;
        let padded = unpadded.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zenith readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("zenith readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);

        let (tx, rx) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| RenderError::Other(format!("poll: {e:?}")))?;
        rx.recv()
            .map_err(|e| RenderError::Other(e.to_string()))?
            .map_err(|e| RenderError::Other(e.to_string()))?;
        let mapped = readback
            .slice(..)
            .get_mapped_range()
            .map_err(|e| RenderError::Other(format!("map range: {e:?}")))?;
        let mut out = Vec::with_capacity((unpadded * h) as usize);
        for row in mapped.chunks(padded as usize) {
            out.extend_from_slice(&row[..unpadded as usize]);
        }
        drop(mapped);
        readback.unmap();
        Ok(out)
    }
}

impl Renderer for WgpuRenderer {
    fn resize(&mut self, width_px: u32, height_px: u32, scale_factor: f32) {
        self.scale = scale_factor;
        let size = (width_px.max(1), height_px.max(1));
        if size == self.size_px {
            return;
        }
        self.size_px = size;
        match &mut self.target {
            Target::Surface { surface, config } => {
                config.width = size.0;
                config.height = size.1;
                surface.configure(&self.device, config);
            }
            Target::Offscreen { texture } => {
                *texture = Self::offscreen_texture(&self.device, size, self.format);
            }
        }
    }

    fn render(&mut self, list: &DrawList) -> Result<(), RenderError> {
        let Target::Surface { surface, config } = &self.target else {
            // Headless: draw and discard (use `render_to_rgba` to read back).
            return self.render_to_rgba(list).map(|_| ());
        };
        let surface_texture = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                surface.configure(&self.device, config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                return Err(RenderError::Lost("surface lost".into()));
            }
            other => return Err(RenderError::Other(format!("surface: {other:?}"))),
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.draw(list, &view)?;
        self.queue.present(surface_texture);
        Ok(())
    }

    fn text_measure(&self) -> Arc<dyn TextMeasure> {
        self.measure.clone()
    }
}
