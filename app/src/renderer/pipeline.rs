use eframe::wgpu;
use eframe::wgpu::util::DeviceExt;
use eframe::egui;
use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use crate::scene::{Scene, Shape};
use crate::texture_manager::TextureManager;
use super::shaders::*;
use super::helpers::*;
use super::mesh_builder::{build_scene_mesh, build_scene_mesh_incremental, ObjMeshCache, SceneMeshResult};

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
    /// Separate edge line vertices (LineList topology)
    cached_edge_verts: Vec<Vertex>,
    edge_vb: Option<wgpu::Buffer>,
    edge_vb_capacity: usize,
    /// Per-object mesh cache for incremental rebuild
    per_object_cache: std::collections::HashMap<String, ObjMeshCache>,
    /// Per-object GPU buffers (SketchUp-style: upload once, draw many)
    gpu_objects: std::collections::HashMap<String, GpuObjectSlot>,
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

/// Per-object GPU buffers (SketchUp-style rendering)
struct GpuObjectSlot {
    obj_version: u64,
    face_vb: wgpu::Buffer,
    face_ib: wgpu::Buffer,
    face_idx_count: u32,
    edge_vb: wgpu::Buffer,
    edge_vert_count: u32,
    aabb_center: [f32; 3],
    aabb_extent: f32,  // bounding sphere radius
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
                    cull_mode: None, // 雙面渲染（SketchUp 面是雙面的）
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
            gpu_objects: std::collections::HashMap::new(),
            cached_edge_verts: Vec::new(),
            edge_vb: None,
            edge_vb_capacity: 0,
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

    /// 快取的頂點數（用於效能監控）
    pub fn cached_vert_count(&self) -> usize { self.cached_verts.len() }
    /// 快取的索引數（用於效能監控）
    pub fn cached_idx_count(&self) -> usize { self.cached_idx.len() }

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
                cull_mode: None, // 雙面渲染
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

        // ══════════════════════════════════════════════════════════════════════
        // SketchUp-style per-object GPU rendering（大場景 > 100 物件）
        // 每個物件有自己的 GPU VBO，只在物件變更時上傳，渲染用 frustum cull + multi draw call
        // ══════════════════════════════════════════════════════════════════════
        let big_scene = scene.objects.len() > 100;

        if big_scene {
            // ── Step 1: 更新 per-object GPU buffers（只重建變更的物件）──
            let geometry_changed = scene.version != self.cached_scene_version;
            if geometry_changed || self.gpu_objects.is_empty() {
                use super::mesh_builder::build_single_object_mesh_pub;
                // 移除已刪除物件
                self.gpu_objects.retain(|id, _| scene.objects.contains_key(id));

                for obj in scene.objects.values() {
                    if !obj.visible {
                        self.gpu_objects.remove(&obj.id);
                        continue;
                    }
                    let needs_upload = match self.gpu_objects.get(&obj.id) {
                        Some(slot) => slot.obj_version != obj.obj_version,
                        None => true,
                    };
                    if !needs_upload { continue; }

                    let mesh_data = build_single_object_mesh_pub(obj, edge_thickness, render_mode, texture_manager, view_proj, scene.objects.len());
                    if mesh_data.face_verts.is_empty() { continue; }

                    // AABB for frustum culling
                    let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
                    for v in &mesh_data.face_verts {
                        for i in 0..3 { mn[i] = mn[i].min(v.position[i]); mx[i] = mx[i].max(v.position[i]); }
                    }
                    let center = [(mn[0]+mx[0])*0.5, (mn[1]+mx[1])*0.5, (mn[2]+mx[2])*0.5];
                    let extent = ((mx[0]-mn[0]).powi(2) + (mx[1]-mn[1]).powi(2) + (mx[2]-mn[2]).powi(2)).sqrt() * 0.5;

                    // Upload face VBO + IBO
                    let face_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None, contents: bytemuck::cast_slice(&mesh_data.face_verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let face_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None, contents: bytemuck::cast_slice(&mesh_data.face_idx),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    // Upload edge VBO (LineList)
                    let edge_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: if mesh_data.edge_verts.is_empty() {
                            &[0u8; 40] // dummy
                        } else {
                            bytemuck::cast_slice(&mesh_data.edge_verts)
                        },
                        usage: wgpu::BufferUsages::VERTEX,
                    });

                    self.gpu_objects.insert(obj.id.clone(), GpuObjectSlot {
                        obj_version: obj.obj_version,
                        face_vb, face_ib,
                        face_idx_count: mesh_data.face_idx.len() as u32,
                        edge_vb,
                        edge_vert_count: mesh_data.edge_verts.len() as u32,
                        aabb_center: center,
                        aabb_extent: extent,
                    });
                }
                self.cached_scene_version = scene.version;
            }

            // ── Step 2: Preview geometry（drawing tools 預覽）──
            let has_preview = !preview.0.is_empty();
            let mut preview_vb: Option<wgpu::Buffer> = None;
            let mut preview_ib: Option<wgpu::Buffer> = None;
            let mut preview_idx_count = 0u32;
            if has_preview {
                preview_vb = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None, contents: bytemuck::cast_slice(&preview.0), usage: wgpu::BufferUsages::VERTEX,
                }));
                preview_ib = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None, contents: bytemuck::cast_slice(&preview.1), usage: wgpu::BufferUsages::INDEX,
                }));
                preview_idx_count = preview.1.len() as u32;
            }

            // ── Step 3: Render pass ──
            let grid_buf = make_buffer(device, queue, &self.grid_verts, wgpu::BufferUsages::VERTEX);

            // ── Pre-build selection outline buffer（在 render pass 之前）──
            let sel_outline_buf = if !selected_ids.is_empty() {
                let sel_set: std::collections::HashSet<&str> = selected_ids.iter().map(|s| s.as_str()).collect();
                let mut sel_lines: Vec<Vertex> = Vec::new();
                let sel_color = [0.2_f32, 0.6, 1.0, 1.0];
                let n0 = [0.0_f32, 1.0, 0.0];
                for sel_id in &sel_set {
                    if let Some(slot) = self.gpu_objects.get(*sel_id) {
                        let c = slot.aabb_center;
                        let r = slot.aabb_extent;
                        let mn = [c[0]-r, c[1]-r, c[2]-r];
                        let mx = [c[0]+r, c[1]+r, c[2]+r];
                        let edges: [([f32;3],[f32;3]);12] = [
                            (mn, [mx[0],mn[1],mn[2]]), ([mx[0],mn[1],mn[2]], [mx[0],mn[1],mx[2]]),
                            ([mx[0],mn[1],mx[2]], [mn[0],mn[1],mx[2]]), ([mn[0],mn[1],mx[2]], mn),
                            ([mn[0],mx[1],mn[2]], [mx[0],mx[1],mn[2]]), ([mx[0],mx[1],mn[2]], [mx[0],mx[1],mx[2]]),
                            ([mx[0],mx[1],mx[2]], [mn[0],mx[1],mx[2]]), ([mn[0],mx[1],mx[2]], [mn[0],mx[1],mn[2]]),
                            (mn, [mn[0],mx[1],mn[2]]), ([mx[0],mn[1],mn[2]], [mx[0],mx[1],mn[2]]),
                            ([mx[0],mn[1],mx[2]], [mx[0],mx[1],mx[2]]), ([mn[0],mn[1],mx[2]], [mn[0],mx[1],mx[2]]),
                        ];
                        for (a, b) in &edges {
                            sel_lines.push(Vertex { position: *a, normal: n0, color: sel_color });
                            sel_lines.push(Vertex { position: *b, normal: n0, color: sel_color });
                        }
                    }
                }
                if !sel_lines.is_empty() {
                    Some((device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None, contents: bytemuck::cast_slice(&sel_lines), usage: wgpu::BufferUsages::VERTEX,
                    }), sel_lines.len() as u32))
                } else { None }
            } else { None };

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
                            load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.12, g: 0.12, b: 0.16, a: 1.0 }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                pass.set_bind_group(0, &self.bind_group, &[]);
                if let Some(ref shadow_tex_bg) = self.shadow_tex_bind_group {
                    pass.set_bind_group(1, shadow_tex_bg, &[]);
                }

                // Sky
                if render_mode != 5 {
                    pass.set_pipeline(&self.sky_pipeline);
                    pass.draw(0..3, 0..1);
                }

                // Grid
                if render_mode != 5 && show_grid {
                    pass.set_pipeline(&self.line_pipeline);
                    pass.set_vertex_buffer(0, grid_buf.slice(..));
                    pass.draw(0..self.grid_verts.len() as u32, 0..1);
                }

                // ── Per-object draw calls with frustum culling ──
                let mut drawn_objects = 0u32;
                let mut drawn_face_verts = 0u32;
                let mut drawn_edge_verts = 0u32;

                // Face pass (triangles)
                pass.set_pipeline(&self.tri_pipeline);
                for slot in self.gpu_objects.values() {
                    // Frustum cull
                    let c = glam::Vec3::from(slot.aabb_center);
                    let clip = view_proj * glam::Vec4::new(c.x, c.y, c.z, 1.0);
                    if clip.w > 0.0 {
                        let ndc_x = (clip.x / clip.w).abs();
                        let ndc_y = (clip.y / clip.w).abs();
                        let r_ndc = slot.aabb_extent / clip.w;
                        if ndc_x - r_ndc > 1.5 || ndc_y - r_ndc > 1.5 { continue; }
                    }
                    if slot.face_idx_count == 0 { continue; }
                    pass.set_vertex_buffer(0, slot.face_vb.slice(..));
                    pass.set_index_buffer(slot.face_ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..slot.face_idx_count, 0, 0..1);
                    drawn_objects += 1;
                    drawn_face_verts += slot.face_idx_count;
                }

                // Edge pass (lines)
                pass.set_pipeline(&self.line_pipeline);
                for slot in self.gpu_objects.values() {
                    if slot.edge_vert_count == 0 { continue; }
                    let c = glam::Vec3::from(slot.aabb_center);
                    let clip = view_proj * glam::Vec4::new(c.x, c.y, c.z, 1.0);
                    if clip.w > 0.0 {
                        let ndc_x = (clip.x / clip.w).abs();
                        let ndc_y = (clip.y / clip.w).abs();
                        let r_ndc = slot.aabb_extent / clip.w;
                        if ndc_x - r_ndc > 1.5 || ndc_y - r_ndc > 1.5 { continue; }
                    }
                    pass.set_vertex_buffer(0, slot.edge_vb.slice(..));
                    pass.draw(0..slot.edge_vert_count, 0..1);
                    drawn_edge_verts += slot.edge_vert_count;
                }

                // ── Selection highlight（AABB outline）──
                if let Some((ref sel_buf, sel_count)) = sel_outline_buf {
                    pass.set_pipeline(&self.line_pipeline);
                    pass.set_vertex_buffer(0, sel_buf.slice(..));
                    pass.draw(0..sel_count, 0..1);
                }

                // Preview geometry
                if let (Some(ref pvb), Some(ref pib)) = (&preview_vb, &preview_ib) {
                    pass.set_pipeline(&self.tri_pipeline);
                    pass.set_vertex_buffer(0, pvb.slice(..));
                    pass.set_index_buffer(pib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..preview_idx_count, 0, 0..1);
                }

                self.cached_face_vert_count = drawn_face_verts as usize;
            }
            queue.submit(std::iter::once(encoder.finish()));
            return; // 大場景路徑完成，跳過舊路徑
        }

        // ══════════════════════════════════════════════════════════════════════
        // 小場景舊路徑（< 100 物件，用 mega-buffer）
        // ══════════════════════════════════════════════════════════════════════
        let geometry_changed = scene.version != self.cached_scene_version;
        let has_preview = !preview.0.is_empty();
        let style_changed = (self.cached_edge_thickness - edge_thickness).abs() > 0.01
            || self.cached_render_mode != render_mode;
        let scene_dirty = geometry_changed || has_preview || self.cached_verts.is_empty() || style_changed;

        if scene_dirty {
            let result = build_scene_mesh_incremental(
                scene, &mut self.per_object_cache,
                selected_ids, hovered_id, editing_group_id, editing_component_def_id,
                hovered_face, selected_face,
                edge_thickness, render_mode,
                texture_manager,
                view_proj,
            );
            self.cached_verts = result.face_verts;
            self.cached_idx = result.face_idx;
            self.cached_edge_verts = result.edge_verts;
            self.cached_scene_version = scene.version;
            self.cached_edge_thickness = edge_thickness;
            self.cached_render_mode = render_mode;
        }

        let mut tri_verts = Vec::with_capacity(self.cached_verts.len() + preview.0.len());
        tri_verts.extend_from_slice(&self.cached_verts);
        let offset = tri_verts.len() as u32;
        tri_verts.extend_from_slice(&preview.0);
        let mut tri_idx = Vec::with_capacity(self.cached_idx.len() + preview.1.len());
        tri_idx.extend_from_slice(&self.cached_idx);
        tri_idx.extend(preview.1.iter().map(|i| i + offset));

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

        // Upload edge VB (LineList)
        if !self.cached_edge_verts.is_empty() {
            let edge_bytes = bytemuck::cast_slice::<Vertex, u8>(&self.cached_edge_verts);
            reuse_or_grow_buffer(
                device, queue, edge_bytes,
                &mut self.edge_vb, &mut self.edge_vb_capacity,
                wgpu::BufferUsages::VERTEX, "edge_vb",
            );
        }

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
        // 大場景（> 100 萬頂點）自動關閉 shadow pass（GPU 瓶頸）
        let shadow_enabled = self.shadow_map_enabled && tri_verts.len() < 1_000_000;
        if shadow_enabled {
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

            // Ground shadows (drawn before scene so objects render on top) — skip in Sketch mode and large scenes
            if render_mode != 5 && !shadow_idx.is_empty() && shadow_enabled {
                if let (Some(svb), Some(sib)) = (&self.shadow_vb, &self.shadow_ib) {
                    pass.set_pipeline(&self.tri_pipeline);
                    pass.set_vertex_buffer(0, svb.slice(..));
                    pass.set_index_buffer(sib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..shadow_idx.len() as u32, 0, 0..1);
                }
            }

            // Scene triangles (faces)
            if !tri_idx.is_empty() {
                if let (Some(vb), Some(ib)) = (&self.scene_vb, &self.scene_ib) {
                    pass.set_pipeline(&self.tri_pipeline);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..tri_idx.len() as u32, 0, 0..1);
                }
            }

            // Scene edges (LineList — SketchUp style, 2 verts per edge)
            if !self.cached_edge_verts.is_empty() {
                if let Some(ref evb) = self.edge_vb {
                    pass.set_pipeline(&self.line_pipeline);
                    pass.set_vertex_buffer(0, evb.slice(..));
                    pass.draw(0..self.cached_edge_verts.len() as u32, 0..1);
                }
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}

