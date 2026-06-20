mod mascot;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, Modifiers, MouseButton, WindowEvent};
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

use mascot::{COLS as MASCOT_COLS, EYE_COLS, EYE_ROWS, GRID as MASCOT_GRID, PIXEL_SIZE, ROWS as MASCOT_ROWS};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
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
    color: [f32; 4],
) {
    let p = |px: f32, py: f32| -> [f32; 2] { [px / sw * 2.0 - 1.0, 1.0 - py / sh * 2.0] };
    let mk = |px: f32, py: f32, u: f32, v: f32| Vertex { position: p(px, py), uv: [u, v], color };
    verts.extend_from_slice(&[
        mk(x0, y0, u0, v0),
        mk(x1, y0, u1, v0),
        mk(x0, y1, u0, v1),
        mk(x0, y1, u0, v1),
        mk(x1, y0, u1, v0),
        mk(x1, y1, u1, v1),
    ]);
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
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
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
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x4
                    ],
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

        // Buffer sized for editor text + mascot overhead (pixels + labels)
        let max_verts = (MAX_CHARS + 400) * 6;
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

    fn update_text(&mut self, text: &str, cursor: usize, sel: Option<(usize, usize)>) {
        let verts = self.build_text_vertices(text, cursor, sel);
        self.vertex_count = verts.len() as u32;
        self.queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&verts));
    }

    fn update_mascot(&mut self, t_ms: f64, hop_elapsed_ms: Option<f64>) {
        let verts = self.build_mascot_vertices(t_ms, hop_elapsed_ms);
        self.vertex_count = verts.len() as u32;
        self.queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&verts));
    }

    fn build_text_vertices(&self, text: &str, cursor: usize, sel: Option<(usize, usize)>) -> Vec<Vertex> {
        let sw = self.config.width as f32;
        let sh = self.config.height as f32;
        let white = [1.0f32, 1.0, 1.0, 1.0];
        let sel_color = [0.27f32, 0.45, 0.85, 0.35];
        let mut verts: Vec<Vertex> = Vec::with_capacity((text.len() + 1) * 6);

        let line_height = self.ascent + self.descent;
        let mut pen_x = PADDING_X;
        let mut line = 0u32;
        let mut cursor_x = pen_x;
        let mut cursor_line = 0u32;
        let mut byte_idx = 0usize;

        for ch in text.chars() {
            if byte_idx == cursor {
                cursor_x = pen_x;
                cursor_line = line;
            }
            let baseline_y = PADDING_Y + line as f32 * line_height + self.ascent;
            let top_y = baseline_y - self.ascent;
            let bot_y = baseline_y + self.descent;
            if ch == '\n' {
                if let Some((s, e)) = sel {
                    if byte_idx >= s && byte_idx < e {
                        push_quad(&mut verts, sw, sh, pen_x, top_y, pen_x + 8.0, bot_y,
                            -1.0, -1.0, -1.0, -1.0, sel_color);
                    }
                }
                pen_x = PADDING_X;
                line += 1;
            } else if let Some(g) = self.glyphs.get(&ch) {
                if let Some((s, e)) = sel {
                    if byte_idx >= s && byte_idx < e {
                        push_quad(&mut verts, sw, sh, pen_x, top_y, pen_x + g.advance_width, bot_y,
                            -1.0, -1.0, -1.0, -1.0, sel_color);
                    }
                }
                if g.width > 0.0 && g.height > 0.0 {
                    let x0 = pen_x + g.bearing_x;
                    let y0 = baseline_y - g.above_baseline;
                    let v0 = (self.max_above - g.above_baseline) / self.atlas_height;
                    let v1 = v0 + g.height / self.atlas_height;
                    push_quad(&mut verts, sw, sh, x0, y0, x0 + g.width, y0 + g.height,
                        g.u0, v0, g.u1, v1, white);
                }
                pen_x += g.advance_width;
            }
            byte_idx += ch.len_utf8();
        }

        if byte_idx == cursor {
            cursor_x = pen_x;
            cursor_line = line;
        }

        let cursor_baseline_y = PADDING_Y + cursor_line as f32 * line_height + self.ascent;
        push_quad(&mut verts, sw, sh,
            cursor_x, cursor_baseline_y - self.ascent,
            cursor_x + CURSOR_WIDTH_PX, cursor_baseline_y + self.descent,
            -1.0, -1.0, -1.0, -1.0, white);

        verts
    }

    fn build_mascot_vertices(&self, t_ms: f64, hop_elapsed_ms: Option<f64>) -> Vec<Vertex> {
        let sw = self.config.width as f32;
        let sh = self.config.height as f32;
        let white = [1.0f32, 1.0, 1.0, 1.0];
        let dim = [0.40f32, 0.40, 0.50, 1.0];

        let s = PIXEL_SIZE;
        let sprite_w = MASCOT_COLS as f32 * s;
        let sprite_h = MASCOT_ROWS as f32 * s;

        // Float: gentle sine bob
        let float_y = (t_ms / 1400.0 * std::f64::consts::PI).sin() as f32 * s;

        // Hop: -sin arc over 580ms
        let hop_y = hop_elapsed_ms
            .map(|ms| -(ms / 580.0 * std::f64::consts::PI).sin() as f32 * s * 3.2)
            .unwrap_or(0.0);

        let dy = (float_y + hop_y).round();

        // Blink cycle: 4200ms
        let bp = (t_ms % 4200.0) / 4200.0;
        let squinting = (bp > 0.87 && bp < 0.90) || (bp > 0.94 && bp < 0.97);
        let blinking = bp > 0.90 && bp < 0.94;

        // Layout
        let line_height = self.ascent + self.descent;
        let label1 = "// src/mascot.rs";
        let label2 = "kouik";
        let label1_w = self.text_width(label1);
        let label2_w = self.text_width(label2);
        let gap_sprite_label = 40.0f32;
        let gap_labels = 8.0f32;
        let total_h = sprite_h + gap_sprite_label + line_height + gap_labels + line_height;
        let ox = ((sw - sprite_w) / 2.0).round();
        let oy = ((sh - total_h) / 2.0).round();

        let mut verts = Vec::with_capacity(300 * 6);

        // Shadow (fixed ground position, shrinks as sprite hops)
        let shadow_cy = oy + sprite_h + s * 0.75;
        let s_scale = (1.0 - dy.abs() / (s * 7.0)).max(0.3);
        let shadow_hw = sprite_w * 0.42 * s_scale;
        let shadow_hh = s * 0.55 * s_scale;
        push_quad(&mut verts, sw, sh,
            sw / 2.0 - shadow_hw, shadow_cy - shadow_hh,
            sw / 2.0 + shadow_hw, shadow_cy + shadow_hh,
            -1.0, -1.0, -1.0, -1.0,
            [0.0, 0.0, 0.0, s_scale * 0.42]);

        // Sprite pixels
        for (r, row) in MASCOT_GRID.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                let Some(base_color) = cell else { continue };
                let is_eye = EYE_ROWS.contains(&r) && EYE_COLS.contains(&c);
                let color = if is_eye && (blinking || (squinting && r == 7)) {
                    [1.0f32, 1.0, 1.0, 1.0]
                } else {
                    *base_color
                };
                let x0 = ox + c as f32 * s;
                let y0 = oy + r as f32 * s + dy;
                push_quad(&mut verts, sw, sh, x0, y0, x0 + s, y0 + s,
                    -1.0, -1.0, -1.0, -1.0, color);
            }
        }

        // Label 1: "// src/mascot.rs" in dim
        let label1_x = ((sw - label1_w) / 2.0).round();
        let label1_baseline = oy + sprite_h + gap_sprite_label + self.ascent;
        self.append_text(&mut verts, sw, sh, label1, label1_x, label1_baseline, dim);

        // Label 2: "kouik" + blinking cursor in white
        let label2_x = ((sw - label2_w) / 2.0).round();
        let label2_baseline = label1_baseline + self.descent + gap_labels + self.ascent;
        self.append_text(&mut verts, sw, sh, label2, label2_x, label2_baseline, white);

        let cursor_on = (t_ms / 500.0) as u64 % 2 == 0;
        if cursor_on {
            let cur_x = label2_x + label2_w;
            push_quad(&mut verts, sw, sh,
                cur_x, label2_baseline - self.ascent,
                cur_x + CURSOR_WIDTH_PX, label2_baseline + self.descent,
                -1.0, -1.0, -1.0, -1.0, white);
        }

        verts
    }

    fn append_text(
        &self,
        verts: &mut Vec<Vertex>,
        sw: f32,
        sh: f32,
        text: &str,
        x: f32,
        baseline_y: f32,
        color: [f32; 4],
    ) {
        let mut pen_x = x;
        for ch in text.chars() {
            if let Some(g) = self.glyphs.get(&ch) {
                if g.width > 0.0 && g.height > 0.0 {
                    let x0 = pen_x + g.bearing_x;
                    let y0 = baseline_y - g.above_baseline;
                    let v0 = (self.max_above - g.above_baseline) / self.atlas_height;
                    let v1 = v0 + g.height / self.atlas_height;
                    push_quad(verts, sw, sh, x0, y0, x0 + g.width, y0 + g.height,
                        g.u0, v0, g.u1, v1, color);
                }
                pen_x += g.advance_width;
            }
        }
    }

    fn text_width(&self, text: &str) -> f32 {
        text.chars().filter_map(|c| self.glyphs.get(&c)).map(|g| g.advance_width).sum()
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
    anchor: Option<usize>,
    modifiers: Modifiers,
    clipboard: Option<arboard::Clipboard>,
    show_mascot: bool,
    mascot_t0: Instant,
    mascot_hop_t: Option<Instant>,
    file_path: Option<std::path::PathBuf>,
    docs_folder: std::path::PathBuf,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let title = if cfg!(debug_assertions) { "kouik [dev]" } else { "kouik" };
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title(title))
                .unwrap(),
        );
        let mut gpu = pollster::block_on(Gpu::new(Arc::clone(&window)));
        if self.show_mascot {
            gpu.update_mascot(self.mascot_t0.elapsed().as_secs_f64() * 1000.0, None);
        } else {
            let sel = self.anchor.map(|a| (a.min(self.cursor), a.max(self.cursor)));
            gpu.update_text(&self.text, self.cursor, sel);
        }
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
                    if self.show_mascot {
                        let t_ms = self.mascot_t0.elapsed().as_secs_f64() * 1000.0;
                        let hop_ms = self.mascot_hop_t
                            .map(|ht| ht.elapsed().as_secs_f64() * 1000.0)
                            .filter(|&ms| ms < 580.0);
                        gpu.update_mascot(t_ms, hop_ms);
                    } else {
                        let sel = self.anchor.map(|a| (a.min(self.cursor), a.max(self.cursor)));
                        gpu.update_text(&self.text, self.cursor, sel);
                    }
                }
            }

            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods;
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if state == ElementState::Pressed
                    && button == MouseButton::Left
                    && self.show_mascot
                {
                    self.mascot_hop_t = Some(Instant::now());
                }
            }

            WindowEvent::KeyboardInput { event: key_event, .. } => {
                if key_event.state == ElementState::Released {
                    return;
                }

                let was_mascot = self.show_mascot;
                self.show_mascot = false;

                let cmd = self.modifiers.state().super_key() || self.modifiers.state().control_key();
                let shift = self.modifiers.state().shift_key();

                let changed = match &key_event.logical_key {
                    Key::Named(NamedKey::Backspace) => {
                        if let Some(anchor) = self.anchor.take() {
                            let (lo, hi) = (self.cursor.min(anchor), self.cursor.max(anchor));
                            self.text.replace_range(lo..hi, "");
                            self.cursor = lo;
                            true
                        } else if self.cursor > 0 {
                            let prev = prev_char_boundary(&self.text, self.cursor);
                            self.text.remove(prev);
                            self.cursor = prev;
                            true
                        } else {
                            false
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if cmd {
                            let start = line_start(&self.text, self.cursor);
                            if shift { if self.anchor.is_none() { self.anchor = Some(self.cursor); } }
                            else { self.anchor = None; }
                            let changed = self.cursor != start;
                            self.cursor = start;
                            changed
                        } else if shift {
                            if self.anchor.is_none() { self.anchor = Some(self.cursor); }
                            if self.cursor > 0 { self.cursor = prev_char_boundary(&self.text, self.cursor); true }
                            else { false }
                        } else if let Some(anchor) = self.anchor.take() {
                            let target = self.cursor.min(anchor);
                            let changed = self.cursor != target;
                            self.cursor = target;
                            changed
                        } else if self.cursor > 0 {
                            self.cursor = prev_char_boundary(&self.text, self.cursor);
                            true
                        } else {
                            false
                        }
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if cmd {
                            let end = line_end(&self.text, self.cursor);
                            if shift { if self.anchor.is_none() { self.anchor = Some(self.cursor); } }
                            else { self.anchor = None; }
                            let changed = self.cursor != end;
                            self.cursor = end;
                            changed
                        } else if shift {
                            if self.anchor.is_none() { self.anchor = Some(self.cursor); }
                            if self.cursor < self.text.len() { self.cursor = next_char_boundary(&self.text, self.cursor); true }
                            else { false }
                        } else if let Some(anchor) = self.anchor.take() {
                            let target = self.cursor.max(anchor);
                            let changed = self.cursor != target;
                            self.cursor = target;
                            changed
                        } else if self.cursor < self.text.len() {
                            self.cursor = next_char_boundary(&self.text, self.cursor);
                            true
                        } else {
                            false
                        }
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if shift { if self.anchor.is_none() { self.anchor = Some(self.cursor); } }
                        else { self.anchor = None; }
                        if cmd {
                            let changed = self.cursor != 0;
                            self.cursor = 0;
                            changed
                        } else {
                            let cur_start = line_start(&self.text, self.cursor);
                            if cur_start == 0 {
                                if self.cursor != 0 { self.cursor = 0; true } else { false }
                            } else {
                                let col = char_col(&self.text, self.cursor);
                                let prev_start = line_start(&self.text, cur_start - 1);
                                let new_pos = col_to_byte(&self.text, prev_start, col);
                                let changed = self.cursor != new_pos;
                                self.cursor = new_pos;
                                changed
                            }
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if shift { if self.anchor.is_none() { self.anchor = Some(self.cursor); } }
                        else { self.anchor = None; }
                        if cmd {
                            let changed = self.cursor != self.text.len();
                            self.cursor = self.text.len();
                            changed
                        } else if let Some(offset) = self.text[self.cursor..].find('\n') {
                            let col = char_col(&self.text, self.cursor);
                            let next_start = self.cursor + offset + 1;
                            let new_pos = col_to_byte(&self.text, next_start, col);
                            let changed = self.cursor != new_pos;
                            self.cursor = new_pos;
                            changed
                        } else if self.cursor != self.text.len() {
                            self.cursor = self.text.len();
                            true
                        } else {
                            false
                        }
                    }
                    Key::Named(NamedKey::Home) => {
                        let start = line_start(&self.text, self.cursor);
                        if shift { if self.anchor.is_none() { self.anchor = Some(self.cursor); } }
                        else { self.anchor = None; }
                        let changed = self.cursor != start;
                        self.cursor = start;
                        changed
                    }
                    Key::Named(NamedKey::End) => {
                        let end = line_end(&self.text, self.cursor);
                        if shift { if self.anchor.is_none() { self.anchor = Some(self.cursor); } }
                        else { self.anchor = None; }
                        let changed = self.cursor != end;
                        self.cursor = end;
                        changed
                    }
                    Key::Named(NamedKey::Enter) => {
                        if let Some(anchor) = self.anchor.take() {
                            let (lo, hi) = (self.cursor.min(anchor), self.cursor.max(anchor));
                            self.text.replace_range(lo..hi, "");
                            self.cursor = lo;
                        }
                        if self.text.chars().count() < MAX_CHARS {
                            self.text.insert(self.cursor, '\n');
                            self.cursor += 1;
                            true
                        } else {
                            false
                        }
                    }
                    Key::Named(NamedKey::Escape) => {
                        event_loop.exit();
                        false
                    }
                    Key::Character(s) if cmd => match s.as_str() {
                        "n" | "N" => {
                            let picked = rfd::FileDialog::new()
                                .set_directory(&self.docs_folder)
                                .add_filter("Text", &["txt"])
                                .save_file();
                            if let Some(mut path) = picked {
                                if path.extension().is_none() {
                                    path.set_extension("txt");
                                }
                                let _ = std::fs::write(&path, "");
                                self.text.clear();
                                self.cursor = 0;
                                self.anchor = None;
                                self.show_mascot = false;
                                if let Some(w) = &self.window {
                                    let name = path.file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("kouik");
                                    let title = if cfg!(debug_assertions) {
                                        format!("{} [dev]", name)
                                    } else {
                                        name.to_string()
                                    };
                                    w.set_title(&title);
                                }
                                self.file_path = Some(path);
                                true
                            } else {
                                false
                            }
                        }
                        "s" | "S" => {
                            if let Some(path) = &self.file_path {
                                let _ = std::fs::write(path, &self.text);
                            }
                            false
                        }
                        "a" | "A" => {
                            self.anchor = Some(0);
                            self.cursor = self.text.len();
                            true
                        }
                        "c" | "C" => {
                            if let Some(anchor) = self.anchor {
                                let (lo, hi) = (self.cursor.min(anchor), self.cursor.max(anchor));
                                let selected = self.text[lo..hi].to_string();
                                if let Some(cb) = &mut self.clipboard {
                                    let _ = cb.set_text(selected);
                                }
                            }
                            false
                        }
                        "x" | "X" => {
                            if let Some(anchor) = self.anchor.take() {
                                let (lo, hi) = (self.cursor.min(anchor), self.cursor.max(anchor));
                                let selected = self.text[lo..hi].to_string();
                                if let Some(cb) = &mut self.clipboard {
                                    let _ = cb.set_text(selected);
                                }
                                self.text.replace_range(lo..hi, "");
                                self.cursor = lo;
                                true
                            } else {
                                false
                            }
                        }
                        "v" | "V" => {
                            if let Some(anchor) = self.anchor.take() {
                                let (lo, hi) = (self.cursor.min(anchor), self.cursor.max(anchor));
                                self.text.replace_range(lo..hi, "");
                                self.cursor = lo;
                            }
                            let pasted = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok());
                            if let Some(pasted) = pasted {
                                for ch in pasted.chars() {
                                    if (!ch.is_control() || ch == '\n') && self.text.chars().count() < MAX_CHARS {
                                        self.text.insert(self.cursor, ch);
                                        self.cursor += ch.len_utf8();
                                    }
                                }
                                true
                            } else {
                                false
                            }
                        }
                        _ => false,
                    },
                    _ => {
                        if let Some(text) = &key_event.text {
                            if let Some(anchor) = self.anchor.take() {
                                let (lo, hi) = (self.cursor.min(anchor), self.cursor.max(anchor));
                                self.text.replace_range(lo..hi, "");
                                self.cursor = lo;
                            }
                            let before = self.text.len();
                            for ch in text.chars() {
                                if !ch.is_control() && self.text.chars().count() < MAX_CHARS {
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

                if changed || was_mascot {
                    let sel = self.anchor.map(|a| (a.min(self.cursor), a.max(self.cursor)));
                    if let Some(gpu) = &mut self.gpu {
                        gpu.update_text(&self.text, self.cursor, sel);
                    }
                    if let Some(gpu) = &self.gpu {
                        gpu.draw();
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
        if self.show_mascot {
            if let Some(gpu) = &mut self.gpu {
                let t_ms = self.mascot_t0.elapsed().as_secs_f64() * 1000.0;
                let hop_ms = self.mascot_hop_t
                    .map(|ht| ht.elapsed().as_secs_f64() * 1000.0);
                if hop_ms.map(|ms| ms >= 580.0).unwrap_or(false) {
                    self.mascot_hop_t = None;
                }
                gpu.update_mascot(t_ms, hop_ms.filter(|&ms| ms < 580.0));
            }
        }
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

fn line_end(s: &str, from: usize) -> usize {
    s[from..].find('\n').map(|i| from + i).unwrap_or(s.len())
}

fn line_start(s: &str, from: usize) -> usize {
    s[..from].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

fn char_col(s: &str, pos: usize) -> usize {
    let start = line_start(s, pos);
    s[start..pos].chars().count()
}

fn col_to_byte(s: &str, start: usize, col: usize) -> usize {
    let mut idx = start;
    for _ in 0..col {
        match s[idx..].chars().next() {
            Some('\n') | None => break,
            Some(ch) => idx += ch.len_utf8(),
        }
    }
    idx
}

fn docs_folder_config_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home).join(".config").join("kouik").join("folder")
}

fn load_docs_folder() -> Option<std::path::PathBuf> {
    std::fs::read_to_string(docs_folder_config_path())
        .ok()
        .map(|s| std::path::PathBuf::from(s.trim()))
        .filter(|p| p.is_dir())
}

fn persist_docs_folder(path: &std::path::Path) {
    let cfg = docs_folder_config_path();
    let _ = std::fs::create_dir_all(cfg.parent().unwrap());
    let _ = std::fs::write(cfg, path.to_string_lossy().as_bytes());
}

fn main() {
    let docs_folder = load_docs_folder().unwrap_or_else(|| {
        let picked = rfd::FileDialog::new()
            .set_title("Choose your notes folder")
            .pick_folder();
        let path = picked.unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            std::path::PathBuf::from(home).join("Documents")
        });
        persist_docs_folder(&path);
        path
    });

    let mut app = App {
        window: None,
        gpu: None,
        text: String::new(),
        cursor: 0,
        anchor: None,
        modifiers: Modifiers::default(),
        clipboard: arboard::Clipboard::new().ok(),
        show_mascot: true,
        mascot_t0: Instant::now(),
        mascot_hop_t: None,
        file_path: None,
        docs_folder,
    };
    let event_loop = EventLoop::new().unwrap();
    event_loop.run_app(&mut app).unwrap();
}
