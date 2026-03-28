use eframe::wgpu;
use eframe::egui;
use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use crate::scene::{Scene, Shape};
use crate::texture_manager::TextureManager;
use super::shaders::*;
use super::helpers::*;
use super::mesh_builder::build_scene_mesh;

// ─── Viewport Renderer ──────────────────────────────────────────────────────

pub struct ViewportRenderer {
    tri_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    color_tex: wgpu::Texture,
    color_view: wgpu::TextureView,
    msaa_tex: wgpu::Texture,
    msaa_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    pub texture_id: Option<egui::TextureId>,
    pub size: [u32; 2],
    grid_verts: Vec<Vertex>,
    cached_grid_spacing: f32,
    // ── Performance: dirty-flag mesh caching ──
    cached_scene_version: u64,
    cached_verts: Vec<Vertex>,
    cached_face_vert_count: usize,
    cached_idx: Vec<u32>,
    cached_edge_thickness: f32,
    cached_render_mode: u32,
    cached_selected_ids: Vec<String>,
    cached_editing_component_def_id: Option<String>,
    /// Per-object mesh cache for incremental rebuild
    per_object_cache: std::collections::HashMap<String, (u64, Vec<Vertex>, Vec<u32>)>,
    // ── Performance: shadow caching ──
    cached_shadow_verts: Vec<Vertex>,
    cached_shadow_idx: Vec<u32>,
    // ── Performance: pre-allocated GPU buffers ──
    scene_vb: Option<wgpu::Buffer>,
    scene_ib: Option<wgpu::Buffer>,
    scene_vb_capacity: usize,
    scene_ib_capacity: usize,
    shadow_vb: Option<wgpu::Buffer>,
    shadow_ib: Option<wgpu::Buffer>,
    shadow_vb_capacity: usize,
    shadow_ib_capacity: usize,
    // ── Shadow Map ──
    shadow_map_enabled: bool,
    shadow_depth_tex: Option<wgpu::Texture>,
    shadow_depth_view: Option<wgpu::TextureView>,
    shadow_pipeline: Option<wgpu::RenderPipeline>,
    shadow_bind_group: Option<wgpu::BindGroup>,
    shadow_uniform_buf: Option<wgpu::Buffer>,
    shadow_tex_bgl: Option<wgpu::BindGroupLayout>,
    shadow_tex_bind_group: Option<wgpu::BindGroup>,
    shadow_sampler: Option<wgpu::Sampler>,
}

impl ViewportRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniform_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform_bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        // Shadow texture bind group layout (@group(1))
        let shadow_tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_tex_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&bgl, &shadow_tex_bgl],
            push_constant_ranges: &[],
        });
        let sky_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky_pipeline_layout"),
            bind_group_layouts: &[&bgl], // sky only needs group 0
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0,  shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x3 },
                wgpu::VertexAttribute { offset: 24, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
            ],
        };

        let make_pipeline = |topo: wgpu::PrimitiveTopology, label: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[vertex_layout.clone()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FMT,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: topo,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FMT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 4,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            })
        };

        let tri_pipeline = make_pipeline(wgpu::PrimitiveTopology::TriangleList, "tri_pipe");
        let line_pipeline = make_pipeline(wgpu::PrimitiveTopology::LineList, "line_pipe");

        // Sky gradient pipeline (no vertex buffers, no depth, no bind group)
        let sky_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky_shader"),
            source: wgpu::ShaderSource::Wgsl(SKY_SHADER.into()),
        });
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky_pipe"),
            layout: Some(&sky_layout),
            vertex: wgpu::VertexState {
                module: &sky_shader, entry_point: "vs_sky",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sky_shader, entry_point: "fs_sky",
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR_FMT, blend: None, write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FMT,
                depth_write_enabled: false, // sky doesn't write depth
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 4,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let (color_tex, color_view, msaa_tex, msaa_view, depth_view) = create_textures(device, 2, 2);
        let grid_verts = generate_grid(100_000.0, 1_000.0);

        Self {
            tri_pipeline, line_pipeline, sky_pipeline,
            uniform_buf, bind_group,
            color_tex, color_view, msaa_tex, msaa_view, depth_view,
            texture_id: None,
            size: [2, 2],
            grid_verts,
            cached_grid_spacing: 1000.0,
            // Dirty-flag caching (version 0 forces first rebuild)
            cached_scene_version: u64::MAX,
            cached_verts: Vec::new(),
            cached_face_vert_count: 0,
            cached_idx: Vec::new(),
            cached_edge_thickness: -1.0,
            cached_render_mode: u32::MAX,
            cached_selected_ids: Vec::new(),
            cached_editing_component_def_id: None,
            per_object_cache: std::collections::HashMap::new(),
            // Pre-allocated GPU buffers (None = not yet created)
            cached_shadow_verts: Vec::new(),
            cached_shadow_idx: Vec::new(),
            scene_vb: None,
            scene_ib: None,
            scene_vb_capacity: 0,
            scene_ib_capacity: 0,
            shadow_vb: None,
            shadow_ib: None,
            shadow_vb_capacity: 0,
            shadow_ib_capacity: 0,
            // Shadow map (initialized lazily on first render)
            shadow_map_enabled: true,
            shadow_depth_tex: None,
            shadow_depth_view: None,
            shadow_pipeline: None,
            shadow_bind_group: None,
            shadow_uniform_buf: None,
            shadow_tex_bgl: Some(shadow_tex_bgl),
            shadow_tex_bind_group: None,
            shadow_sampler: None,
        }
    }

    pub fn ensure_size(
        &mut self,
        device: &wgpu::Device,
        egui_renderer: &mut eframe::egui_wgpu::Renderer,
        w: u32, h: u32,
    ) {
        let w = w.max(1);
        let h = h.max(1);
        if self.size == [w, h] { return; }

        if let Some(id) = self.texture_id.take() {
            egui_renderer.free_texture(&id);
        }

        let (ct, cv, mt, mv, dv) = create_textures(device, w, h);
        self.color_tex = ct;
        self.color_view = cv;
        self.msaa_tex = mt;
        self.msaa_view = mv;
        self.depth_view = dv;
        self.size = [w, h];

        self.texture_id = Some(egui_renderer.register_native_texture(
            device,
            &self.color_view,
            wgpu::FilterMode::Linear,
        ));
    }

    /// Save the current viewport to a PNG file
    pub fn save_screenshot(&self, device: &wgpu::Device, queue: &wgpu::Queue, path: &str) {
        let [w, h] = self.size;
        if w < 2 || h < 2 { return; }

        let bpp = 4u32;
        let unpadded_row = w * bpp;
        let padded_row = (unpadded_row + 255) & !255;

        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot_staging"),
            size: (padded_row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("screenshot_enc"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.color_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &staging,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(encoder.finish()));

        let slice = staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::Maintain::Wait);

        let data = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((w * h * bpp) as usize);
        for row in 0..h {
            let start = (row * padded_row) as usize;
            let end = start + (w * bpp) as usize;
            pixels.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging.unmap();

        if let Some(img) = image::RgbaImage::from_raw(w, h, pixels) {
            match img.save(path) {
                Ok(_) => tracing::info!("Screenshot saved: {}", path),
                Err(e) => tracing::error!("Screenshot save failed: {}", e),
            }
        }
    }

    /// Capture viewport pixels as RGB (no alpha). Returns (width, height, rgb_bytes).
    pub fn capture_rgb(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Option<(u32, u32, Vec<u8>)> {
        let [w, h] = self.size;
        if w < 2 || h < 2 { return None; }

        let bpp = 4u32;
        let unpadded_row = w * bpp;
        let padded_row = (unpadded_row + 255) & !255;

        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture_staging"),
            size: (padded_row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("capture_enc"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.color_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &staging,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(encoder.finish()));

        let slice = staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::Maintain::Wait);

        let data = slice.get_mapped_range();
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for row in 0..h {
            let start = (row * padded_row) as usize;
            for col in 0..w {
                let px = start + (col * bpp) as usize;
                rgb.push(data[px]);
                rgb.push(data[px + 1]);
                rgb.push(data[px + 2]);
            }
        }
        drop(data);
        staging.unmap();

        Some((w, h, rgb))
    }

    /// Initialize shadow map resources (called lazily on first render)
    fn init_shadow_map(&mut self, device: &wgpu::Device) {
        if self.shadow_pipeline.is_some() { return; } // already initialized

        // Shadow depth texture
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_depth_tex"),
            size: wgpu::Extent3d { width: SHADOW_MAP_SIZE, height: SHADOW_MAP_SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_DEPTH_FMT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());

        // Shadow uniform buffer (light VP matrix)
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow_uniform_buf"),
            size: 64, // mat4x4<f32>
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Shadow bind group layout + bind group
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0, visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None,
                },
                count: None,
            }],
        });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_bind_group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() }],
        });

        // Shadow pipeline (depth-only, no fragment output)
        let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADOW_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shadow_shader,
                entry_point: "vs_shadow",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4],
                }],
                compilation_options: Default::default(),
            },
            fragment: None, // depth-only
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: SHADOW_DEPTH_FMT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Shadow comparison sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow_sampler"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Shadow texture bind group for main shader (@group(1))
        if let Some(ref bgl) = self.shadow_tex_bgl {
            let tex_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow_tex_bg"),
                layout: bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
                ],
            });
            self.shadow_tex_bind_group = Some(tex_bg);
        }

        self.shadow_sampler = Some(sampler);
        self.shadow_depth_tex = Some(tex);
        self.shadow_depth_view = Some(view);
        self.shadow_uniform_buf = Some(buf);
        self.shadow_bind_group = Some(bg);
        self.shadow_pipeline = Some(pipeline);
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        scene: &Scene,
        selected_ids: &[String],
        hovered_id: Option<&str>,
        editing_group_id: Option<&str>,
        editing_component_def_id: Option<&str>,
        preview: &(Vec<Vertex>, Vec<u32>),
        render_mode: u32,
        sky_color: [f32; 3],
        ground_color: [f32; 3],
        hovered_face: Option<(&str, u8)>,
        selected_face: Option<(&str, u8)>,
        edge_thickness: f32,
        show_colors: bool,
        texture_manager: &TextureManager,
        show_grid: bool,
        grid_spacing: f32,
        section_plane: [f32; 4],
    ) {
        // Initialize shadow map on first render (before any borrows)
        if self.shadow_map_enabled {
            self.init_shadow_map(device);
        }

        // Upload uniforms
        // 從 view_proj 的逆矩陣提取相機位置
        let inv_vp = view_proj.inverse();
        let cam_pos = inv_vp.col(3).truncate();
        // Light VP for shadow mapping
        let light_dir = glam::Vec3::new(0.3, -1.0, 0.5).normalize();
        let light_pos = -light_dir * 20000.0;
        let light_view = glam::Mat4::look_at_rh(light_pos, glam::Vec3::ZERO, glam::Vec3::Y);
        let light_proj = glam::Mat4::orthographic_rh(-15000.0, 15000.0, -15000.0, 15000.0, 0.1, 50000.0);
        let light_vp = light_proj * light_view;

        let uniforms = Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
            sky_color: [sky_color[0], sky_color[1], sky_color[2], 0.0],
            ground_color: [ground_color[0], ground_color[1], ground_color[2], 0.0],
            camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z, 0.0],
            light_vp: light_vp.to_cols_array_2d(),
            section_plane,
        };
        queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniforms));

        // Regenerate grid if spacing changed
        if (grid_spacing - self.cached_grid_spacing).abs() > 0.1 {
            self.grid_verts = generate_grid(100_000.0, grid_spacing);
            self.cached_grid_spacing = grid_spacing;
        }

        // ── Dirty-flag caching: only rebuild scene mesh when scene changes ──
        let geometry_changed = scene.version != self.cached_scene_version;
        let has_preview = !preview.0.is_empty();
        let style_changed = (self.cached_edge_thickness - edge_thickness).abs() > 0.01
            || self.cached_render_mode != render_mode;
        let editing_component_changed = self.cached_editing_component_def_id.as_deref() != editing_component_def_id;
        let selection_changed = selected_ids != self.cached_selected_ids.as_slice();
        let scene_dirty = geometry_changed || has_preview || self.cached_verts.is_empty() || style_changed || editing_component_changed || selection_changed;

        if scene_dirty {
            let (verts, idx) = build_scene_mesh(
                scene, selected_ids, hovered_id, editing_group_id, editing_component_def_id,
                hovered_face, selected_face,
                edge_thickness, render_mode,
                texture_manager,
                view_proj,
            );
            self.cached_verts = verts;
            self.cached_idx = idx;
            self.cached_scene_version = scene.version;
            self.cached_edge_thickness = edge_thickness;
            self.cached_render_mode = render_mode;
            self.cached_editing_component_def_id = editing_component_def_id.map(|s| s.to_string());
            self.cached_selected_ids = selected_ids.to_vec();
        }

        // Combine cached scene mesh + preview geometry
        let mut tri_verts = self.cached_verts.clone();
        let mut tri_idx = self.cached_idx.clone();
        let offset = tri_verts.len() as u32;
        tri_verts.extend_from_slice(&preview.0);
        tri_idx.extend(preview.1.iter().map(|i| i + offset));

        // Apply render mode to vertex colors
        // Edge line vertices have dark colors (rgb < 0.3) — preserve them in wireframe/sketch modes
        match render_mode {
            1 => { // Wireframe: make faces invisible, keep edge lines visible
                for v in &mut tri_verts {
                    let is_edge = v.color[0] < 0.3 && v.color[1] < 0.3 && v.color[2] < 0.3;
                    let is_selected_edge = v.color[2] > 0.8 && v.color[0] < 0.4; // blue selection edge
                    if !is_edge && !is_selected_edge {
                        v.color[3] = 0.0; // hide face completely
                    }
                }
            }
            2 => { // X-Ray: make faces transparent, keep edges
                for v in &mut tri_verts {
                    let is_edge = v.color[0] < 0.3 && v.color[1] < 0.3 && v.color[2] < 0.3;
                    if !is_edge {
                        v.color[3] = 0.15;
                    }
                }
            }
            3 => { // Hidden Line: white surfaces, keep edges
                for v in &mut tri_verts {
                    let is_edge = v.color[0] < 0.3 && v.color[1] < 0.3 && v.color[2] < 0.3;
                    if !is_edge {
                        v.color = [0.95, 0.95, 0.95, 1.0];
                    }
                }
            }
            4 => { // Monochrome: uniform grey faces, keep edges
                for v in &mut tri_verts {
                    let is_edge = v.color[0] < 0.3 && v.color[1] < 0.3 && v.color[2] < 0.3;
                    if !is_edge {
                        v.color = [0.75, 0.75, 0.75, 1.0];
                    }
                }
            }
            5 => { // Sketch: invisible faces, black edges only
                for v in &mut tri_verts {
                    let is_edge = v.color[0] < 0.3 && v.color[1] < 0.3 && v.color[2] < 0.3;
                    let is_selected_edge = v.color[2] > 0.8 && v.color[0] < 0.4;
                    if is_edge || is_selected_edge {
                        v.color = [0.0, 0.0, 0.0, 1.0]; // pure black edges
                    } else {
                        v.color[3] = 0.0; // hide faces
                    }
                }
            }
            _ => {} // Shaded: default
        }

        // Color removal: replace all vertex colors with light grey (keep alpha)
        if !show_colors && render_mode != 5 {
            for v in &mut tri_verts {
                v.color = [0.85, 0.85, 0.85, v.color[3]];
            }
        }

        // GPU buffer 大小保護（wgpu 上限 256MB，reuse_or_grow_buffer 會加 1.5x 餘量）
        // 必須在 shadow 計算之前截斷
        {
            const MAX_BUF: usize = 160 * 1024 * 1024;
            let max_v = MAX_BUF / std::mem::size_of::<Vertex>();
            let max_i = MAX_BUF / 4;
            if tri_verts.len() > max_v {
                tracing::error!("[Renderer] Scene too large: {} verts (max {}), truncating", tri_verts.len(), max_v);
                tri_verts.truncate(max_v);
            }
            if tri_idx.len() > max_i {
                tracing::error!("[Renderer] Scene too large: {} idx (max {}), truncating", tri_idx.len(), max_i);
                tri_idx.truncate(max_i);
            }
        }

        // Generate ground shadow vertices (cached — improved directional projection)
        if scene_dirty {
            let light_dir = glam::Vec3::new(-0.3, -1.0, -0.5).normalize();
            self.cached_shadow_verts = tri_verts.iter().map(|v| {
                let pos = glam::Vec3::from(v.position);
                let (proj_x, proj_z) = if light_dir.y.abs() > 0.001 {
                    let t = -pos.y / light_dir.y;
                    let proj = pos + light_dir * t;
                    (proj.x, proj.z)
                } else {
                    (pos.x, pos.z)
                };
                // 高度衰減：物件越高，陰影越淡
                let height_fade = (1.0 - (pos.y / 10000.0).min(0.8)).max(0.02);
                let alpha = 0.12 * height_fade;
                Vertex {
                    position: [proj_x, 0.5, proj_z],
                    normal: [0.0, 1.0, 0.0],
                    color: [0.0, 0.0, 0.0, alpha],
                }
            }).collect();
            self.cached_shadow_idx = tri_idx.clone();
        }
        let shadow_verts = &self.cached_shadow_verts;
        let shadow_idx = &self.cached_shadow_idx;

        // ── Pre-allocated GPU buffer reuse ──
        let grid_buf = make_buffer(device, queue, &self.grid_verts, wgpu::BufferUsages::VERTEX);

        let vert_bytes = bytemuck::cast_slice::<Vertex, u8>(&tri_verts);
        let idx_bytes = bytemuck::cast_slice::<u32, u8>(&tri_idx);
        reuse_or_grow_buffer(
            device, queue, vert_bytes,
            &mut self.scene_vb, &mut self.scene_vb_capacity,
            wgpu::BufferUsages::VERTEX, "scene_vb",
        );
        reuse_or_grow_buffer(
            device, queue, idx_bytes,
            &mut self.scene_ib, &mut self.scene_ib_capacity,
            wgpu::BufferUsages::INDEX, "scene_ib",
        );

        let sv_bytes = bytemuck::cast_slice::<Vertex, u8>(&shadow_verts);
        let si_bytes = bytemuck::cast_slice::<u32, u8>(&shadow_idx);
        reuse_or_grow_buffer(
            device, queue, sv_bytes,
            &mut self.shadow_vb, &mut self.shadow_vb_capacity,
            wgpu::BufferUsages::VERTEX, "shadow_vb",
        );
        reuse_or_grow_buffer(
            device, queue, si_bytes,
            &mut self.shadow_ib, &mut self.shadow_ib_capacity,
            wgpu::BufferUsages::INDEX, "shadow_ib",
        );

        // ── Shadow pass (depth-only from light's perspective) ──
        if self.shadow_map_enabled {
            if let (Some(ref shadow_view), Some(ref shadow_pipeline), Some(ref shadow_bg), Some(ref shadow_ubuf)) =
                (&self.shadow_depth_view, &self.shadow_pipeline, &self.shadow_bind_group, &self.shadow_uniform_buf)
            {
                // Light view-projection (directional, orthographic)
                let light_dir = glam::Vec3::new(0.3, -1.0, 0.5).normalize();
                let light_pos = -light_dir * 20000.0;
                let light_view = glam::Mat4::look_at_rh(light_pos, glam::Vec3::ZERO, glam::Vec3::Y);
                let light_proj = glam::Mat4::orthographic_rh(-15000.0, 15000.0, -15000.0, 15000.0, 0.1, 50000.0);
                let light_vp = light_proj * light_view;
                queue.write_buffer(shadow_ubuf, 0, bytemuck::bytes_of(&light_vp.to_cols_array_2d()));

                let mut shadow_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("shadow_enc"),
                });
                {
                    let mut pass = shadow_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("shadow_pass"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: shadow_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        ..Default::default()
                    });
                    pass.set_pipeline(shadow_pipeline);
                    pass.set_bind_group(0, shadow_bg, &[]);
                    // 用跟主 pass 相同的 scene vertex/index buffer
                    if let (Some(ref vb), Some(ref ib)) = (&self.scene_vb, &self.scene_ib) {
                        pass.set_vertex_buffer(0, vb.slice(..));
                        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                        let face_idx_count = (self.cached_face_vert_count as f32 * 1.5) as u32; // approximate
                        pass.draw_indexed(0..face_idx_count.min(self.cached_idx.len() as u32), 0, 0..1);
                    }
                }
                queue.submit(std::iter::once(shadow_encoder.finish()));
            }
        }

        // Encode
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("viewport_enc"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,
                    resolve_target: Some(&self.color_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(if render_mode == 5 {
                            wgpu::Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }
                        } else {
                            wgpu::Color { r: 0.12, g: 0.12, b: 0.16, a: 1.0 }
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Set bind groups
            pass.set_bind_group(0, &self.bind_group, &[]);
            if let Some(ref shadow_tex_bg) = self.shadow_tex_bind_group {
                pass.set_bind_group(1, shadow_tex_bg, &[]);
            }

            // Sky gradient (fullscreen triangle, no vertex buffer) — skip in Sketch mode
            if render_mode != 5 {
                pass.set_pipeline(&self.sky_pipeline);
                pass.draw(0..3, 0..1);
            }

            // Grid lines — skip in Sketch mode or when hidden
            if render_mode != 5 && show_grid {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_vertex_buffer(0, grid_buf.slice(..));
                pass.draw(0..self.grid_verts.len() as u32, 0..1);
            }

            // Ground shadows (drawn before scene so objects render on top) — skip in Sketch mode
            if render_mode != 5 && !shadow_idx.is_empty() {
                if let (Some(svb), Some(sib)) = (&self.shadow_vb, &self.shadow_ib) {
                    pass.set_pipeline(&self.tri_pipeline);
                    pass.set_vertex_buffer(0, svb.slice(..));
                    pass.set_index_buffer(sib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..shadow_idx.len() as u32, 0, 0..1);
                }
            }

            // Scene triangles
            if !tri_idx.is_empty() {
                if let (Some(vb), Some(ib)) = (&self.scene_vb, &self.scene_ib) {
                    pass.set_pipeline(&self.tri_pipeline);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..tri_idx.len() as u32, 0, 0..1);
                }
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}

