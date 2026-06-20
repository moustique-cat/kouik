use std::collections::HashMap;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

const FONT_BYTES: &[u8] = include_bytes!("/System/Library/Fonts/SFNSMono.ttf");
const FONT_SIZE: f32 = 20.0;
const PADDING_X: f32 = 40.0;
const PADDING_Y: f32 = 40.0;
const CURSOR_WIDTH_PX: f32 = 2.0;
const MAX_CHARS: usize = 4096;
const ASCII_FIRST: u8 = 32;
const ASCII_LAST: u8 = 126;
const GLYPH_COUNT: usize = (ASCII_LAST - ASCII_FIRST + 1) as usize;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
}

struct GlyphInfo {
    advance_width: f32,
    bearing_x: f32,
    width: f32,
    height: f32,
    above_baseline: f32,
    u0: f32,
    u1: f32,
}

fn push_quad(
    verts: &mut Vec<Vertex>,
    sw: f32,
    sh: f32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
) {
    let p = |px: f32, py: f32| -> [f32; 2] { [px / sw * 2.0 - 1.0, 1.0 - py / sh * 2.0] };
    let tl = Vertex { position: p(x0, y0), uv: [u0, v0] };
    let tr = Vertex { position: p(x1, y0), uv: [u1, v0] };
    let bl = Vertex { position: p(x0, y1), uv: [u0, v1] };
    let br = Vertex { position: p(x1, y1), uv: [u1, v1] };
    verts.extend_from_slice(&[tl, tr, bl, bl, tr, br]);
}

struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
    bind_group: wgpu::BindGroup,
    glyphs: HashMap<char, GlyphInfo>,
    atlas_height: f32,
    max_above: f32,
    ascent: f32,
    descent: f32,
}

impl Gpu {
    async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(Arc::clone(&window)).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // --- Build glyph atlas for printable ASCII ---
        let font =
            fontdue::Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default()).unwrap();

        let rasterized: Vec<(char, fontdue::Metrics, Vec<u8>)> = (ASCII_FIRST..=ASCII_LAST)
            .map(|b| {
                let c = b as char;
                let (m, bitmap) = font.rasterize(c, FONT_SIZE);
                (c, m, bitmap)
            })
            .collect();

        // Determine atlas dimensions from the tallest glyphs above and below baseline.
        // fontdue: ymin = bottom of glyph in pixels above baseline; ymin+height = top above baseline.
        let max_above = rasterized
            .iter()
            .map(|(_, m, _)| m.ymin + m.height as i32)
            .max()
            .unwrap_or(1);
        let max_below = rasterized
            .iter()
            .map(|(_, m, _)| (-m.ymin).max(0))
            .max()
            .unwrap_or(0);
        let atlas_h = ((max_above + max_below) as u32).max(1);

        // Fixed-width cells: max advance_width across all glyphs (monospace → all equal).
        let cell_w = rasterized
            .iter()
            .map(|(_, m, _)| m.advance_width.ceil() as u32)
            .max()
            .unwrap_or(16);
        let atlas_w = (cell_w * GLYPH_COUNT as u32).max(1);

        let mut atlas_pixels = vec![0u8; (atlas_w * atlas_h) as usize];
        let mut glyphs: HashMap<char, GlyphInfo> = HashMap::with_capacity(GLYPH_COUNT);

        for (i, (ch, metrics, bitmap)) in rasterized.iter().enumerate() {
            let above_this = metrics.ymin + metrics.height as i32;
            let y_top = (max_above - above_this).max(0) as u32;
            let x_off = i as u32 * cell_w;
            let bearing_x = metrics.xmin.max(0) as u32;

            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let ax = x_off + bearing_x + col as u32;
                    let ay = y_top + row as u32;
                    if ax < atlas_w && ay < atlas_h {
                        atlas_pixels[(ay * atlas_w + ax) as usize] =
                            bitmap[row * metrics.width + col];
                    }
                }
            }

            let bx = bearing_x as f32;
            glyphs.insert(*ch, GlyphInfo {
                advance_width: metrics.advance_width,
                bearing_x: bx,
                width: metrics.width as f32,
                height: metrics.height as f32,
                above_baseline: above_this as f32,
                u0: (x_off as f32 + bx) / atlas_w as f32,
                u1: (x_off as f32 + bx + metrics.width as f32) / atlas_w as f32,
            });
        }

        // Upload atlas texture (R8Unorm: one byte per pixel = alpha coverage).
        let atlas_size =
            wgpu::Extent3d { width: atlas_w, height: atlas_h, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: atlas_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas_w),
                rows_per_image: Some(atlas_h),
            },
            atlas_size,
        );
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Pre-allocated dynamic vertex buffer: (MAX_CHARS glyphs + 1 cursor) × 6 vertices.
        let max_verts = (MAX_CHARS + 1) * 6;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (max_verts * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            vertex_buffer,
            vertex_count: 0,
            bind_group,
            glyphs,
            atlas_height: atlas_h as f32,
            max_above: max_above as f32,
            ascent: max_above as f32,
            descent: max_below as f32,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    // Rebuild vertex data from text + cursor and upload to GPU.
    fn update(&mut self, text: &str, cursor: usize) {
        let verts = self.build_vertices(text, cursor);
        self.vertex_count = verts.len() as u32;
        self.queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&verts));
    }

    fn build_vertices(&self, text: &str, cursor: usize) -> Vec<Vertex> {
        let sw = self.config.width as f32;
        let sh = self.config.height as f32;
        let mut verts: Vec<Vertex> = Vec::with_capacity((text.len() + 1) * 6);

        let mut pen_x = PADDING_X;
        let baseline_y = PADDING_Y + self.ascent;
        let mut cursor_x = pen_x;
        let mut byte_idx = 0usize;

        for ch in text.chars() {
            if byte_idx == cursor {
                cursor_x = pen_x;
            }
            if let Some(g) = self.glyphs.get(&ch) {
                if g.width > 0.0 && g.height > 0.0 {
                    let x0 = pen_x + g.bearing_x;
                    let y0 = baseline_y - g.above_baseline;
                    let v0 = (self.max_above - g.above_baseline) / self.atlas_height;
                    let v1 = v0 + g.height / self.atlas_height;
                    push_quad(
                        &mut verts,
                        sw,
                        sh,
                        x0,
                        y0,
                        x0 + g.width,
                        y0 + g.height,
                        g.u0,
                        v0,
                        g.u1,
                        v1,
                    );
                }
                pen_x += g.advance_width;
            }
            byte_idx += ch.len_utf8();
        }

        if byte_idx == cursor {
            cursor_x = pen_x;
        }

        // Cursor bar. UV x = -1 signals solid fill in the fragment shader.
        push_quad(
            &mut verts,
            sw,
            sh,
            cursor_x,
            baseline_y - self.ascent,
            cursor_x + CURSOR_WIDTH_PX,
            baseline_y + self.descent,
            -1.0,
            -1.0,
            -1.0,
            -1.0,
        );

        verts
    }

    fn draw(&self) {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => return,
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            if self.vertex_count > 0 {
                pass.draw(0..self.vertex_count, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        frame.present();
    }
}

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    text: String,
    cursor: usize,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("kouik"))
                .unwrap(),
        );
        let mut gpu = pollster::block_on(Gpu::new(Arc::clone(&window)));
        gpu.update(&self.text, self.cursor);
        self.window = Some(window);
        self.gpu = Some(gpu);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(new_size);
                    gpu.update(&self.text, self.cursor);
                }
            }

            WindowEvent::KeyboardInput { event: key_event, .. } => {
                // Handle both press and repeat (key held down).
                if key_event.state == ElementState::Released {
                    return;
                }
                let changed = match &key_event.logical_key {
                    Key::Named(NamedKey::Backspace) => {
                        if self.cursor > 0 {
                            let prev = prev_char_boundary(&self.text, self.cursor);
                            self.text.remove(prev);
                            self.cursor = prev;
                            true
                        } else {
                            false
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if self.cursor > 0 {
                            self.cursor = prev_char_boundary(&self.text, self.cursor);
                            true
                        } else {
                            false
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if self.cursor < self.text.len() {
                            self.cursor = next_char_boundary(&self.text, self.cursor);
                            true
                        } else {
                            false
                        }
                    }
                    Key::Named(NamedKey::Escape) => {
                        event_loop.exit();
                        false
                    }
                    _ => {
                        if let Some(text) = &key_event.text {
                            let before = self.text.len();
                            for ch in text.chars() {
                                if !ch.is_control()
                                    && self.text.chars().count() < MAX_CHARS
                                {
                                    self.text.insert(self.cursor, ch);
                                    self.cursor += ch.len_utf8();
                                }
                            }
                            self.text.len() != before
                        } else {
                            false
                        }
                    }
                };

                if changed {
                    if let Some(gpu) = &mut self.gpu {
                        gpu.update(&self.text, self.cursor);
                    }
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &self.gpu {
                    gpu.draw();
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn prev_char_boundary(s: &str, from: usize) -> usize {
    let mut i = from - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_char_boundary(s: &str, from: usize) -> usize {
    let mut i = from + 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn main() {
    let mut app = App { window: None, gpu: None, text: String::new(), cursor: 0 };
    let event_loop = EventLoop::new().unwrap();
    event_loop.run_app(&mut app).unwrap();
}
