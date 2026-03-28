use eframe::egui;
use eframe::wgpu;
use bytemuck::{Pod, Zeroable};
use glam::Mat4;

use crate::scene::{Scene, Shape};
use crate::texture_manager::TextureManager;

// ─── GPU Types ───────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    sky_color: [f32; 4],
    ground_color: [f32; 4],
    camera_pos: [f32; 4],
    light_vp: [[f32; 4]; 4],
}

pub const COLOR_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_MAP_SIZE: u32 = 2048;
const SHADOW_DEPTH_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

// Shadow depth-only shader (vertex only, no fragment output)
const SHADOW_SHADER: &str = r#"
struct ShadowUniforms {
    light_vp: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> su: ShadowUniforms;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

@vertex fn vs_shadow(i: VsIn) -> @builtin(position) vec4<f32> {
    return su.light_vp * vec4<f32>(i.pos, 1.0);
}
"#;

// ─── WGSL Shader ─────────────────────────────────────────────────────────────

const SHADER: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    sky_color: vec4<f32>,
    ground_color: vec4<f32>,
    camera_pos: vec4<f32>,
    light_vp: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var shadow_tex: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

@vertex fn vs_main(i: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = u.view_proj * vec4<f32>(i.pos, 1.0);
    o.world_pos = i.pos;
    o.normal = i.normal;
    o.color = i.color;
    return o;
}

// ─── Procedural texture functions (triplanar, world-space UVs) ──────────────

fn brick_pattern(pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    // Triplanar: pick dominant axis
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = pos.xz;
    } else if an.x > an.z {
        uv = pos.yz;
    } else {
        uv = pos.xy;
    }
    let scale = 0.002; // ~500mm bricks
    let p = uv * scale;
    let row = floor(p.y);
    let offset = step(1.0, fract(row * 0.5) * 2.0) * 0.5;
    let brick_x = fract(p.x + offset);
    let brick_y = fract(p.y);
    let mortar = 0.06;
    let is_mortar = step(brick_x, mortar) + step(1.0 - mortar, brick_x) +
                    step(brick_y, mortar) + step(1.0 - mortar, brick_y);
    let brick_col = vec3<f32>(0.72, 0.35, 0.22);
    let mortar_col = vec3<f32>(0.78, 0.75, 0.70);
    return mix(brick_col, mortar_col, clamp(is_mortar, 0.0, 1.0));
}

fn wood_pattern(pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = pos.xz;
    } else if an.x > an.z {
        uv = pos.yz;
    } else {
        uv = pos.xy;
    }
    let scale = 0.003;
    let p = uv * scale;
    let grain = sin(p.x * 10.0 + sin(p.y * 3.0) * 2.0) * 0.5 + 0.5;
    let light_wood = vec3<f32>(0.76, 0.60, 0.38);
    let dark_wood = vec3<f32>(0.50, 0.32, 0.15);
    return mix(dark_wood, light_wood, grain);
}

fn metal_pattern(pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = pos.xz;
    } else if an.x > an.z {
        uv = pos.yz;
    } else {
        uv = pos.xy;
    }
    let scale = 0.01;
    let p = uv * scale;
    let stripe = sin(p.y * 20.0) * 0.03;
    let brushed = sin(p.x * 80.0) * 0.008;
    return vec3<f32>(0.72 + stripe + brushed, 0.73 + stripe + brushed, 0.76 + stripe + brushed);
}

fn concrete_pattern(pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = pos.xz;
    } else if an.x > an.z {
        uv = pos.yz;
    } else {
        uv = pos.xy;
    }
    let scale = 0.005;
    let p = uv * scale;
    let n = fract(sin(dot(floor(p), vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let base = vec3<f32>(0.62, 0.60, 0.58);
    return base + vec3<f32>(n * 0.08 - 0.04);
}

fn marble_pattern(pos: vec3<f32>, normal: vec3<f32>, base_col: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = pos.xz;
    } else if an.x > an.z {
        uv = pos.yz;
    } else {
        uv = pos.xy;
    }
    let scale = 0.002;
    let p = uv * scale;
    let vein = sin(p.x * 5.0 + sin(p.y * 3.0) * 4.0) * 0.5 + 0.5;
    let dark = base_col * 0.8;
    return mix(base_col, dark, vein * 0.3);
}

fn tile_pattern(pos: vec3<f32>, normal: vec3<f32>, base_col: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = pos.xz;
    } else if an.x > an.z {
        uv = pos.yz;
    } else {
        uv = pos.xy;
    }
    let scale = 0.003;
    let p = uv * scale;
    let gx = fract(p.x);
    let gz = fract(p.y);
    let grout = 0.04;
    let is_grout = step(gx, grout) + step(1.0 - grout, gx) + step(gz, grout) + step(1.0 - grout, gz);
    let grout_col = base_col * 0.8;
    return mix(base_col, grout_col, clamp(is_grout, 0.0, 1.0));
}

fn asphalt_pattern(pos: vec3<f32>, normal: vec3<f32>, base_col: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = pos.xz;
    } else if an.x > an.z {
        uv = pos.yz;
    } else {
        uv = pos.xy;
    }
    let scale = 0.01;
    let p = uv * scale;
    let noise = fract(sin(dot(floor(p * 5.0), vec2<f32>(12.9898, 78.233))) * 43758.5453);
    return base_col + vec3<f32>(noise * 0.06 - 0.03);
}

fn grass_pattern(pos: vec3<f32>, normal: vec3<f32>, base_col: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = pos.xz;
    } else if an.x > an.z {
        uv = pos.yz;
    } else {
        uv = pos.xy;
    }
    let scale = 0.005;
    let p = uv * scale;
    let blade = sin(p.x * 30.0 + sin(p.y * 20.0) * 3.0) * 0.12;
    return base_col + vec3<f32>(blade * 0.3, blade, blade * 0.2);
}

// ─── Fragment shader ────────────────────────────────────────────────────────

@fragment fn fs_main(i: VsOut, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.5));
    let n = normalize(i.normal);
    let v = normalize(u.camera_pos.xyz - i.world_pos);
    let h = normalize(light_dir + v);
    let ndl = max(dot(n, light_dir), 0.0);
    let ndh = max(dot(n, h), 0.0);
    let ndv = max(dot(n, v), 0.01);

    // PBR: roughness 從 vertex color alpha 的低位元編碼（0.0-0.9 範圍）
    // 如果 alpha > 0.9 則為程序紋理 sentinel，使用預設 roughness
    let raw_alpha = i.color.a;
    let roughness = select(clamp(raw_alpha * 1.1, 0.05, 1.0), 0.5, raw_alpha > 0.9);
    let metallic = select(0.0, 0.8, raw_alpha > 0.925 && raw_alpha < 0.935); // metal pattern

    // GGX distribution
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = ndh * ndh * (a2 - 1.0) + 1.0;
    let D = a2 / (3.14159 * denom * denom + 0.0001);

    // Schlick Fresnel
    let f0 = mix(vec3<f32>(0.04), i.color.rgb, metallic);
    let F = f0 + (1.0 - f0) * pow(1.0 - max(dot(h, v), 0.0), 5.0);

    // Smith geometry (simplified)
    let k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    let G1_v = ndv / (ndv * (1.0 - k) + k);
    let G1_l = ndl / (ndl * (1.0 - k) + k);
    let G = G1_v * G1_l;

    let specular = D * F * G / (4.0 * ndv * ndl + 0.001);
    let ambient = 0.25 + 0.1 * max(dot(n, vec3<f32>(0.0, 1.0, 0.0)), 0.0);

    var base_color = i.color.rgb;

    // Back-face tinting: SketchUp-style blue-grey for reversed faces
    if !is_front {
        base_color = mix(base_color, vec3<f32>(0.5, 0.55, 0.7), 0.4);
    }

    let alpha = i.color.a;

    // Procedural textures keyed by sentinel alpha values
    if alpha > 0.905 && alpha < 0.915 {
        base_color = brick_pattern(i.world_pos, n);
    } else if alpha > 0.915 && alpha < 0.925 {
        base_color = wood_pattern(i.world_pos, n);
    } else if alpha > 0.925 && alpha < 0.935 {
        base_color = metal_pattern(i.world_pos, n);
    } else if alpha > 0.935 && alpha < 0.945 {
        base_color = concrete_pattern(i.world_pos, n);
    } else if alpha > 0.945 && alpha < 0.955 {
        base_color = marble_pattern(i.world_pos, n, base_color);
    } else if alpha > 0.955 && alpha < 0.965 {
        base_color = tile_pattern(i.world_pos, n, base_color);
    } else if alpha > 0.965 && alpha < 0.975 {
        base_color = asphalt_pattern(i.world_pos, n, base_color);
    } else if alpha > 0.975 && alpha < 0.985 {
        base_color = grass_pattern(i.world_pos, n, base_color);
    }

    // Resolve final alpha: sentinel values (>0.9) become opaque
    let final_alpha = select(alpha, 1.0, alpha > 0.9);

    // Shadow mapping
    let light_clip = u.light_vp * vec4<f32>(i.world_pos, 1.0);
    var shadow = 1.0;
    if light_clip.w > 0.0 {
        let light_ndc = light_clip.xyz / light_clip.w;
        let shadow_uv = vec2<f32>(light_ndc.x * 0.5 + 0.5, -light_ndc.y * 0.5 + 0.5);
        let shadow_depth = light_ndc.z;
        if shadow_uv.x >= 0.0 && shadow_uv.x <= 1.0 && shadow_uv.y >= 0.0 && shadow_uv.y <= 1.0 {
            shadow = textureSampleCompare(shadow_tex, shadow_sampler, shadow_uv, shadow_depth - 0.002);
            shadow = mix(0.4, 1.0, shadow); // 陰影不會全黑
        }
    }

    // PBR 合成：diffuse + specular + shadow
    let kd = (1.0 - metallic) * base_color;
    let diffuse_term = kd * (ambient + 0.6 * ndl * shadow);
    let spec_term = specular * ndl * shadow;
    var col = vec4<f32>(diffuse_term + spec_term, final_alpha);

    // Edge detection: subtle shader-based edges (explicit geometric edges handle most cases)
    let dx_n = dpdx(i.normal);
    let dy_n = dpdy(i.normal);
    let edge = length(dx_n) + length(dy_n);
    let edge_factor = smoothstep(1.5, 4.0, edge);
    col = mix(col, vec4<f32>(0.0, 0.0, 0.0, col.a), edge_factor * 0.35);

    return col;
}
"#;

// Sky gradient shader (fullscreen quad)
const SKY_SHADER: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    sky_color: vec4<f32>,
    ground_color: vec4<f32>,
    camera_pos: vec4<f32>,
    light_vp: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex fn vs_sky(@builtin(vertex_index) idx: u32) -> VsOut {
    // Fullscreen triangle
    var o: VsOut;
    let x = f32(i32(idx & 1u) * 4 - 1);
    let y = f32(i32(idx & 2u) * 2 - 1);
    o.pos = vec4<f32>(x, y, 0.999, 1.0);
    o.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return o;
}

@fragment fn fs_sky(i: VsOut) -> @location(0) vec4<f32> {
    let t = i.uv.y;
    let sky_zenith  = u.sky_color.rgb * 0.55; // 天頂深藍
    let sky_mid     = u.sky_color.rgb * 0.85;
    let sky_horizon = mix(u.sky_color.rgb, vec3<f32>(0.95, 0.92, 0.88), 0.5); // 地平線暖白
    let ground      = u.ground_color.rgb;

    var col: vec3<f32>;
    if t < 0.2 {
        // 天頂到中天
        let s = t / 0.2;
        col = mix(sky_zenith, sky_mid, s * s); // 二次曲線更自然
    } else if t < 0.45 {
        // 中天到地平線
        let s = (t - 0.2) / 0.25;
        col = mix(sky_mid, sky_horizon, s);
    } else if t < 0.52 {
        // 地平線帶（窄的過渡）
        let s = (t - 0.45) / 0.07;
        col = mix(sky_horizon, ground, smoothstep(0.0, 1.0, s));
    } else {
        // 地面漸暗
        let s = (t - 0.52) / 0.48;
        col = mix(ground, ground * 0.65, s);
    }
    return vec4<f32>(col, 1.0);
}
"#;

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
                    cull_mode: None,
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
        let scene_dirty = geometry_changed || has_preview || self.cached_verts.is_empty() || style_changed || editing_component_changed;

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

// ─── Texture helpers ─────────────────────────────────────────────────────────

fn create_textures(
    device: &wgpu::Device, w: u32, h: u32,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Texture, wgpu::TextureView, wgpu::TextureView) {
    let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };

    // Resolve target (sample_count 1) — used for screenshots and egui display
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("vp_color"), size,
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR_FMT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let color_view = color.create_view(&Default::default());

    // MSAA color texture (4x multisampled render target)
    let msaa = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("vp_msaa"), size,
        mip_level_count: 1, sample_count: 4,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR_FMT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let msaa_view = msaa.create_view(&Default::default());

    // Depth texture (4x multisampled to match MSAA color)
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("vp_depth"), size,
        mip_level_count: 1, sample_count: 4,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FMT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth.create_view(&Default::default());

    (color, color_view, msaa, msaa_view, depth_view)
}

// ─── Buffer helpers ──────────────────────────────────────────────────────────

fn make_buffer<T: Pod>(
    device: &wgpu::Device, queue: &wgpu::Queue, data: &[T], usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    let bytes = bytemuck::cast_slice(data);
    let size = (bytes.len() as u64).max(4); // wgpu requires non-zero
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None, size,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    if !bytes.is_empty() {
        queue.write_buffer(&buf, 0, bytes);
    }
    buf
}

#[allow(dead_code)]
fn make_index_buffer(device: &wgpu::Device, queue: &wgpu::Queue, data: &[u32]) -> wgpu::Buffer {
    make_buffer(device, queue, data, wgpu::BufferUsages::INDEX)
}

/// Reuse an existing GPU buffer if it has enough capacity, otherwise allocate a new one.
/// This avoids creating a new wgpu::Buffer every frame when the data size hasn't grown.
fn reuse_or_grow_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[u8],
    buf: &mut Option<wgpu::Buffer>,
    capacity: &mut usize,
    usage: wgpu::BufferUsages,
    label: &str,
) {
    let needed = data.len().max(4); // wgpu requires non-zero
    if *capacity >= needed {
        // Reuse existing buffer — just upload new data
        if let Some(b) = buf.as_ref() {
            if !data.is_empty() {
                queue.write_buffer(b, 0, data);
            }
            return;
        }
    }
    // Need a larger buffer — allocate with 50% headroom to reduce future re-allocs
    let alloc_size = (needed as f64 * 1.5) as usize;
    let new_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: alloc_size as u64,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    if !data.is_empty() {
        queue.write_buffer(&new_buf, 0, data);
    }
    *buf = Some(new_buf);
    *capacity = alloc_size;
}

// ─── Grid mesh ───────────────────────────────────────────────────────────────

fn generate_grid(size: f32, step: f32) -> Vec<Vertex> {
    let mut v = Vec::new();
    let half = size / 2.0;
    let n = [0.0, 1.0, 0.0];

    // Two-level grid: fine (1m) + coarse (5m) with distance-based fade
    let mut x = -half;
    while x <= half + 0.1 {
        let dist_ratio = x.abs() / half; // 0 at center, 1 at edge
        let fade = (1.0 - dist_ratio * dist_ratio).max(0.0); // quadratic fadeout
        let is_axis = x.abs() < 0.1;
        let is_major = (x % (step * 5.0)).abs() < 0.1; // every 5th line is major

        let c = if is_axis {
            [0.2, 0.2, 0.85, 1.0] // Z axis (blue)
        } else if is_major {
            [0.35, 0.35, 0.35, fade * 0.8] // major grid: darker, fades
        } else {
            [0.28, 0.28, 0.28, fade * 0.4] // minor grid: lighter, fades faster
        };
        v.push(Vertex { position: [x, 0.0, -half], normal: n, color: c });
        v.push(Vertex { position: [x, 0.0,  half], normal: n, color: c });
        x += step;
    }
    let mut z = -half;
    while z <= half + 0.1 {
        let dist_ratio = z.abs() / half;
        let fade = (1.0 - dist_ratio * dist_ratio).max(0.0);
        let is_axis = z.abs() < 0.1;
        let is_major = (z % (step * 5.0)).abs() < 0.1;

        let c = if is_axis {
            [0.85, 0.2, 0.2, 1.0] // X axis (red)
        } else if is_major {
            [0.35, 0.35, 0.35, fade * 0.8]
        } else {
            [0.28, 0.28, 0.28, fade * 0.4]
        };
        v.push(Vertex { position: [-half, 0.0, z], normal: n, color: c });
        v.push(Vertex { position: [ half, 0.0, z], normal: n, color: c });
        z += step;
    }
    // Y axis (green) — tall
    v.push(Vertex { position: [0.0, 0.0, 0.0],   normal: n, color: [0.2, 0.85, 0.2, 1.0] });
    v.push(Vertex { position: [0.0, half, 0.0],  normal: n, color: [0.2, 0.85, 0.2, 0.3] });
    v
}

// ─── Scene mesh generation ───────────────────────────────────────────────────

fn build_scene_mesh(
    scene: &Scene, selected_ids: &[String], hovered: Option<&str>,
    editing_group_id: Option<&str>,
    editing_component_def_id: Option<&str>,
    hovered_face: Option<(&str, u8)>,
    selected_face: Option<(&str, u8)>,
    edge_thickness_param: f32,
    render_mode: u32,
    texture_manager: &TextureManager,
    view_proj: glam::Mat4,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idx = Vec::new();

    for obj in scene.objects.values() {
        if !obj.visible { continue; }

        // ── Frustum culling: 跳過完全在視錐外的物件 ──
        {
            let p = glam::Vec3::from(obj.position);
            let extent = match &obj.shape {
                Shape::Box { width, height, depth } => glam::Vec3::new(*width, *height, *depth),
                Shape::Cylinder { radius, height, .. } => glam::Vec3::new(*radius * 2.0, *height, *radius * 2.0),
                Shape::Sphere { radius, .. } => glam::Vec3::splat(*radius * 2.0),
                _ => glam::Vec3::splat(1000.0), // Line/Mesh 保守估計
            };
            let center = p + extent * 0.5;
            let radius = extent.length() * 0.5;
            // 球體 vs frustum 測試：投影到 clip space
            let clip = view_proj * glam::Vec4::new(center.x, center.y, center.z, 1.0);
            if clip.w > 0.0 {
                let ndc_x = clip.x / clip.w;
                let ndc_y = clip.y / clip.w;
                let ndc_r = radius / clip.w * 1.5; // 投影半徑（保守放大）
                // 如果球心 + 半徑完全在 NDC 範圍外，跳過
                if ndc_x - ndc_r > 1.5 || ndc_x + ndc_r < -1.5
                    || ndc_y - ndc_r > 1.5 || ndc_y + ndc_r < -1.5
                {
                    continue;
                }
            }
            // clip.w <= 0 表示在相機後方但可能很大（不 cull，安全起見）
        }

        // Use texture average color if a texture is loaded, otherwise material color
        let mut color = if let Some(ref tex_path) = obj.texture_path {
            if texture_manager.is_loaded(tex_path) {
                texture_manager.average_color(tex_path)
            } else {
                obj.material.color()
            }
        } else {
            obj.material.color()
        };
        if selected_ids.iter().any(|s| s == &obj.id) {
            // 選取高亮：材質色調 + 藍色淡化，保留材質可辨識性
            let sel = [0.2_f32, 0.6, 1.0];
            color[0] = color[0] * 0.45 + sel[0] * 0.55;
            color[1] = color[1] * 0.45 + sel[1] * 0.55;
            color[2] = color[2] * 0.45 + sel[2] * 0.55;
        } else if Some(obj.id.as_str()) == hovered {
            // lighten
            color[0] = (color[0] + 0.15).min(1.0);
            color[1] = (color[1] + 0.15).min(1.0);
            color[2] = (color[2] + 0.15).min(1.0);
        }

        // Group isolation: dim non-group objects
        if let Some(gid) = editing_group_id {
            if obj.id != gid {
                color[0] *= 0.3;
                color[1] *= 0.3;
                color[2] *= 0.3;
                color[3] *= 0.3;
            }
        }

        if let Some(def_id) = editing_component_def_id {
            if obj.component_def_id.as_deref() != Some(def_id) {
                color[0] *= 0.3;
                color[1] *= 0.3;
                color[2] *= 0.3;
                color[3] *= 0.3;
            }
        }

        // PBR: 編碼 roughness 到 alpha（非程序紋理材質時）
        if color[3] >= 0.99 || color[3] <= 0.0 {
            // 非 sentinel alpha → 用 roughness 值（0.0-0.89 範圍）
            color[3] = obj.roughness.clamp(0.05, 0.89);
        }

        let p = obj.position;
        let start_idx = verts.len();

        // LOD: 根據螢幕投影大小降低 segment 數
        let lod_segments = |base_segs: u32| -> u32 {
            let center = glam::Vec3::from(p);
            let clip = view_proj * glam::Vec4::new(center.x, center.y, center.z, 1.0);
            if clip.w > 0.0 {
                let screen_size = 500.0 / clip.w; // 粗估螢幕投影大小
                if screen_size < 20.0 { return (base_segs / 4).max(6); }
                if screen_size < 80.0 { return (base_segs / 2).max(8); }
            }
            base_segs
        };

        match &obj.shape {
            Shape::Box { width, height, depth } =>
                push_box(&mut verts, &mut idx, p, *width, *height, *depth, color),
            Shape::Cylinder { radius, height, segments } =>
                push_cylinder(&mut verts, &mut idx, p, *radius, *height, lod_segments(*segments), color),
            Shape::Sphere { radius, segments } =>
                push_sphere(&mut verts, &mut idx, p, *radius, lod_segments(*segments), color),
            Shape::Line { points, thickness, .. } =>
                push_line_segments(&mut verts, &mut idx, points, *thickness, color),
            Shape::Mesh(ref mesh) => {
                for (&fid, face) in &mesh.faces {
                    let face_verts = mesh.face_vertices(fid);
                    if face_verts.len() >= 3 {
                        let base = verts.len() as u32;
                        for fv in &face_verts {
                            verts.push(Vertex {
                                position: *fv, normal: face.normal, color,
                            });
                        }
                        for i in 1..face_verts.len()-1 {
                            idx.push(base);
                            idx.push(base + i as u32);
                            idx.push(base + (i+1) as u32);
                        }
                    }
                }
                for (p1, p2) in mesh.all_edge_segments() {
                    let mesh_edge_color = if render_mode == 5 { [0.0, 0.0, 0.0, 1.0] } else { [0.15, 0.15, 0.15, 1.0] };
                    let mesh_edge_thick = if render_mode == 5 { edge_thickness_param * 1.5 } else { edge_thickness_param.max(3.0) };
                    push_line_segments(&mut verts, &mut idx, &[p1, p2], mesh_edge_thick, mesh_edge_color);
                }
            }
        }

        // ── Per-vertex triplanar texture sampling for textured objects ──
        if let Some(ref tex_path) = obj.texture_path {
            if texture_manager.is_loaded(tex_path) && !selected_ids.iter().any(|s| s == &obj.id) {
                // Recolor face vertices with triplanar-sampled texture color
                // Use a scale of 0.001 (1 texture repeat per 1000mm = 1m)
                let scale = 0.001;
                for vert in &mut verts[start_idx..] {
                    // Skip edge line vertices (very thin quads have small normals — skip if color is dark edge)
                    if vert.color[0] < 0.2 && vert.color[1] < 0.2 && vert.color[2] < 0.2 {
                        continue;
                    }
                    let tc = texture_manager.triplanar_sample(tex_path, vert.position, vert.normal, scale);
                    vert.color = tc;
                }
            }
        }

        // ── Explicit geometric edge lines for ALL objects (SketchUp-style) ──
        {
            let edge_color = if render_mode == 5 {
                [0.0, 0.0, 0.0, 1.0]  // pure black for sketch
            } else {
                [0.15, 0.15, 0.15, 1.0] // dark edges
            };
            let edge_thickness = if render_mode == 5 {
                edge_thickness_param * 1.5  // thicker in sketch mode
            } else {
                edge_thickness_param
            };
            match &obj.shape {
                Shape::Box { width, height, depth } => {
                    let (w, h, d) = (*width, *height, *depth);
                    let edges: [([f32; 3], [f32; 3]); 12] = [
                        // Bottom
                        ([p[0],p[1],p[2]], [p[0]+w,p[1],p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1],p[2]+d]),
                        ([p[0]+w,p[1],p[2]+d], [p[0],p[1],p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1],p[2]]),
                        // Top
                        ([p[0],p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d]),
                        ([p[0],p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]]),
                        // Verticals
                        ([p[0],p[1],p[2]], [p[0],p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1]+h,p[2]+d]),
                    ];
                    for (a, b) in &edges {
                        push_line_segments(&mut verts, &mut idx, &[*a, *b], edge_thickness, edge_color);
                    }
                }
                Shape::Cylinder { radius, height, segments } => {
                    let seg = (*segments).max(6);
                    let cx = p[0];
                    let cz = p[2];
                    let r = *radius;
                    let h = *height;
                    // Top and bottom circles
                    for y_off in [0.0, h] {
                        let mut circle_pts: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                        for i in 0..=seg {
                            let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                            circle_pts.push([cx + r * a.cos(), p[1] + y_off, cz + r * a.sin()]);
                        }
                        push_line_segments(&mut verts, &mut idx, &circle_pts, edge_thickness, edge_color);
                    }
                    // 4 vertical lines
                    for i in [0, seg / 4, seg / 2, 3 * seg / 4] {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        let px = cx + r * a.cos();
                        let pz = cz + r * a.sin();
                        push_line_segments(&mut verts, &mut idx,
                            &[[px, p[1], pz], [px, p[1] + h, pz]], edge_thickness, edge_color);
                    }
                }
                Shape::Sphere { radius, segments } => {
                    let seg = (*segments).max(6);
                    let r = *radius;
                    let cx = p[0];
                    let cy = p[1] + r;
                    let cz = p[2];
                    // Equator
                    let mut equator: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        equator.push([cx + r * a.cos(), cy, cz + r * a.sin()]);
                    }
                    push_line_segments(&mut verts, &mut idx, &equator, edge_thickness, edge_color);
                    // Meridian XY
                    let mut meridian: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        meridian.push([cx + r * a.cos(), cy + r * a.sin(), cz]);
                    }
                    push_line_segments(&mut verts, &mut idx, &meridian, edge_thickness, edge_color);
                    // Meridian YZ
                    let mut meridian2: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        meridian2.push([cx, cy + r * a.sin(), cz + r * a.cos()]);
                    }
                    push_line_segments(&mut verts, &mut idx, &meridian2, edge_thickness, edge_color);
                }
                _ => {} // Line and Mesh shapes handle their own edges
            }
        }

        // ── Selection outline (bright blue edges) ──────────────────────────
        let is_selected = selected_ids.iter().any(|s| s == &obj.id);
        if is_selected {
            let sel_color = [0.2, 0.5, 1.0, 1.0]; // bright blue
            let edge_thickness = 6.0;
            match &obj.shape {
                Shape::Box { width, height, depth } => {
                    let (w, h, d) = (*width, *height, *depth);
                    let edges: Vec<([f32; 3], [f32; 3])> = vec![
                        // Bottom
                        ([p[0],p[1],p[2]], [p[0]+w,p[1],p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1],p[2]+d]),
                        ([p[0]+w,p[1],p[2]+d], [p[0],p[1],p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1],p[2]]),
                        // Top
                        ([p[0],p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d]),
                        ([p[0],p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]]),
                        // Verticals
                        ([p[0],p[1],p[2]], [p[0],p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1]+h,p[2]+d]),
                    ];
                    for (a, b) in &edges {
                        push_line_segments(&mut verts, &mut idx, &[*a, *b], edge_thickness, sel_color);
                    }
                }
                Shape::Cylinder { radius, height, segments } => {
                    let seg = (*segments).max(6);
                    let cx = p[0];
                    let cz = p[2];
                    let r = *radius;
                    let h = *height;
                    // Top and bottom circles
                    for y_off in [0.0, h] {
                        let mut circle_pts: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                        for i in 0..=seg {
                            let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                            circle_pts.push([cx + r * a.cos(), p[1] + y_off, cz + r * a.sin()]);
                        }
                        push_line_segments(&mut verts, &mut idx, &circle_pts, edge_thickness, sel_color);
                    }
                    // 4 vertical lines
                    for i in [0, seg / 4, seg / 2, 3 * seg / 4] {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        let px = cx + r * a.cos();
                        let pz = cz + r * a.sin();
                        push_line_segments(&mut verts, &mut idx,
                            &[[px, p[1], pz], [px, p[1] + h, pz]], edge_thickness, sel_color);
                    }
                }
                Shape::Sphere { radius, segments } => {
                    let seg = (*segments).max(6);
                    let r = *radius;
                    let cx = p[0];
                    let cy = p[1] + r; // sphere center is offset by radius
                    let cz = p[2];
                    // Equator (XZ circle at center Y)
                    let mut equator: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        equator.push([cx + r * a.cos(), cy, cz + r * a.sin()]);
                    }
                    push_line_segments(&mut verts, &mut idx, &equator, edge_thickness, sel_color);
                    // Meridian (XY circle)
                    let mut meridian: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        meridian.push([cx + r * a.cos(), cy + r * a.sin(), cz]);
                    }
                    push_line_segments(&mut verts, &mut idx, &meridian, edge_thickness, sel_color);
                    // Second meridian (YZ circle)
                    let mut meridian2: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        meridian2.push([cx, cy + r * a.sin(), cz + r * a.cos()]);
                    }
                    push_line_segments(&mut verts, &mut idx, &meridian2, edge_thickness, sel_color);
                }
                _ => {}
            }
        }

        // ── Face & edge hover highlighting ──────────────────────────────────
        // Use axis-aligned colors: X=Red, Y=Green, Z=Blue (matches SketchUp)
        let face_active = selected_face.or(hovered_face);
        if let Some((hf_id, hf_idx)) = face_active {
            if obj.id == hf_id {
                // Axis color: Front/Back(Z)=Blue, Top/Bottom(Y)=Green, Left/Right(X)=Red
                let (axis_tint, edge_color): ([f32; 3], [f32; 4]) = match hf_idx {
                    0 | 1 => ([0.3, 0.4, 0.95], [0.3, 0.5, 1.0, 1.0]),   // Front/Back → Z = Blue
                    2 | 3 => ([0.3, 0.85, 0.3], [0.2, 0.9, 0.2, 1.0]),   // Top/Bottom → Y = Green
                    4 | 5 => ([0.95, 0.3, 0.3], [1.0, 0.3, 0.3, 1.0]),   // Left/Right → X = Red
                    _     => ([0.5, 0.5, 0.5],  [0.8, 0.8, 0.8, 1.0]),
                };

                match &obj.shape {
                    Shape::Box { width, height, depth } => {
                        // Tint the face vertices with axis color
                        let face_start = start_idx + (hf_idx as usize) * 4;
                        if face_start + 4 <= verts.len() {
                            for i in face_start..face_start + 4 {
                                let c = &mut verts[i].color;
                                c[0] = c[0] * 0.25 + axis_tint[0] * 0.75;
                                c[1] = c[1] * 0.25 + axis_tint[1] * 0.75;
                                c[2] = c[2] * 0.25 + axis_tint[2] * 0.75;
                            }
                        }

                        // Draw edge outline in axis color
                        let px = obj.position;
                        let (w, h, d) = (*width, *height, *depth);
                        let corners: [[f32; 3]; 4] = match hf_idx {
                            0 => [ // Front (Z-)
                                [px[0],px[1],px[2]], [px[0]+w,px[1],px[2]],
                                [px[0]+w,px[1]+h,px[2]], [px[0],px[1]+h,px[2]],
                            ],
                            1 => [ // Back (Z+)
                                [px[0]+w,px[1],px[2]+d], [px[0],px[1],px[2]+d],
                                [px[0],px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]+d],
                            ],
                            2 => [ // Top (Y+)
                                [px[0],px[1]+h,px[2]], [px[0]+w,px[1]+h,px[2]],
                                [px[0]+w,px[1]+h,px[2]+d], [px[0],px[1]+h,px[2]+d],
                            ],
                            3 => [ // Bottom (Y-)
                                [px[0],px[1],px[2]+d], [px[0]+w,px[1],px[2]+d],
                                [px[0]+w,px[1],px[2]], [px[0],px[1],px[2]],
                            ],
                            4 => [ // Left (X-)
                                [px[0],px[1],px[2]+d], [px[0],px[1],px[2]],
                                [px[0],px[1]+h,px[2]], [px[0],px[1]+h,px[2]+d],
                            ],
                            5 => [ // Right (X+)
                                [px[0]+w,px[1],px[2]], [px[0]+w,px[1],px[2]+d],
                                [px[0]+w,px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]],
                            ],
                            _ => [[0.0;3];4],
                        };
                        // Draw closed edge loop (5 points = 4 segments forming a rectangle)
                        let edge_pts = [corners[0], corners[1], corners[2], corners[3], corners[0]];
                        push_line_segments(&mut verts, &mut idx, &edge_pts, 6.0, edge_color);
                    }
                    Shape::Cylinder { radius, height, .. } => {
                        // For cylinders, only top/bottom faces are pick-able
                        // hf_idx 2 = Top, 3 = Bottom (mapped from PullFace::Top/Bottom)
                        let is_top = hf_idx == 2;
                        let face_y = if is_top { obj.position[1] + *height } else { obj.position[1] };
                        // Highlight the cap by drawing a circle outline
                        let seg = 24u32;
                        let cx = obj.position[0] + *radius;
                        let cz = obj.position[2] + *radius;
                        let edge_color = [1.0, 0.9, 0.3, 1.0];
                        let mut circle_pts: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                        for i in 0..=seg {
                            let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                            circle_pts.push([cx + *radius * a.cos(), face_y, cz + *radius * a.sin()]);
                        }
                        push_line_segments(&mut verts, &mut idx, &circle_pts, 6.0, edge_color);
                    }
                    _ => {}
                }
            }
        }

        // ── Click-locked face highlight (stronger than hover) ──────────────
        if let Some((sf_id, sf_idx)) = selected_face {
            if obj.id == sf_id {
                if let Shape::Box { width, height, depth } = &obj.shape {
                    let face_start = start_idx + (sf_idx as usize) * 4;
                    if face_start + 4 <= verts.len() {
                        for i in face_start..face_start + 4 {
                            let c = &mut verts[i].color;
                            c[0] = c[0] * 0.2 + 0.2;
                            c[1] = c[1] * 0.2 + 0.7;
                            c[2] = c[2] * 0.2 + 1.0;
                        }
                    }
                    // Bright cyan edge outline
                    let px = obj.position;
                    let (w, h, d) = (*width, *height, *depth);
                    let edge_color = [0.2, 1.0, 1.0, 1.0]; // cyan
                    let corners: [[f32; 3]; 4] = match sf_idx {
                        0 => [[px[0],px[1],px[2]], [px[0]+w,px[1],px[2]], [px[0]+w,px[1]+h,px[2]], [px[0],px[1]+h,px[2]]],
                        1 => [[px[0]+w,px[1],px[2]+d], [px[0],px[1],px[2]+d], [px[0],px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]+d]],
                        2 => [[px[0],px[1]+h,px[2]], [px[0]+w,px[1]+h,px[2]], [px[0]+w,px[1]+h,px[2]+d], [px[0],px[1]+h,px[2]+d]],
                        3 => [[px[0],px[1],px[2]+d], [px[0]+w,px[1],px[2]+d], [px[0]+w,px[1],px[2]], [px[0],px[1],px[2]]],
                        4 => [[px[0],px[1],px[2]+d], [px[0],px[1],px[2]], [px[0],px[1]+h,px[2]], [px[0],px[1]+h,px[2]+d]],
                        5 => [[px[0]+w,px[1],px[2]], [px[0]+w,px[1],px[2]+d], [px[0]+w,px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]]],
                        _ => [[0.0;3];4],
                    };
                    let edge_pts = [corners[0], corners[1], corners[2], corners[3], corners[0]];
                    push_line_segments(&mut verts, &mut idx, &edge_pts, 8.0, edge_color);
                }
            }
        }

        // Apply Y-axis rotation around object center
        if obj.rotation_y.abs() > 0.001 {
            let (sin, cos) = obj.rotation_y.sin_cos();
            let (center_offset_x, center_offset_z) = match &obj.shape {
                Shape::Box { width, depth, .. } => (*width / 2.0, *depth / 2.0),
                Shape::Cylinder { radius, .. } => (*radius, *radius),
                Shape::Sphere { radius, .. } => (*radius, *radius),
                Shape::Line { .. } => (0.0, 0.0),
                Shape::Mesh(ref mesh) => {
                    let (min, max) = mesh.aabb();
                    ((max[0] - min[0]) / 2.0, (max[2] - min[2]) / 2.0)
                }
            };
            let cx = obj.position[0] + center_offset_x;
            let cz = obj.position[2] + center_offset_z;

            for v in &mut verts[start_idx..] {
                let dx = v.position[0] - cx;
                let dz = v.position[2] - cz;
                v.position[0] = cx + dx * cos - dz * sin;
                v.position[2] = cz + dx * sin + dz * cos;
                // Also rotate normals
                let nx = v.normal[0];
                let nz = v.normal[2];
                v.normal[0] = nx * cos - nz * sin;
                v.normal[2] = nx * sin + nz * cos;
            }
        }
    }

    // ── Render the shared free mesh ──────────────────────────────────────────
    {
        let mesh = &scene.free_mesh;
        let mat_color = scene.free_mesh_material.color();
        let face_color = if editing_group_id.is_some() || editing_component_def_id.is_some() {
            [mat_color[0] * 0.3, mat_color[1] * 0.3, mat_color[2] * 0.3, mat_color[3]]
        } else {
            mat_color
        };

        // Render faces
        for (&fid, face) in &mesh.faces {
            let face_verts = mesh.face_vertices(fid);
            if face_verts.len() >= 3 {
                let base = verts.len() as u32;
                for fv in &face_verts {
                    verts.push(Vertex {
                        position: *fv,
                        normal: face.normal,
                        color: face_color,
                    });
                }
                for i in 1..face_verts.len() - 1 {
                    idx.push(base);
                    idx.push(base + i as u32);
                    idx.push(base + (i + 1) as u32);
                }
            }
        }

        // Render edges as thin lines
        for (p1, p2) in mesh.all_edge_segments() {
            push_line_segments(&mut verts, &mut idx, &[p1, p2], 5.0, [0.1, 0.1, 0.1, 1.0]);
        }
    }

    (verts, idx)
}

pub fn push_line_pub(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    points: &[[f32; 3]], thickness: f32, c: [f32; 4],
) { push_line_segments(v, idx, points, thickness, c); }

pub fn push_box_pub(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], w: f32, h: f32, d: f32, c: [f32; 4],
) { push_box(v, idx, p, w, h, d, c); }

pub fn push_cylinder_pub(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], r: f32, h: f32, seg: u32, c: [f32; 4],
) { push_cylinder(v, idx, p, r, h, seg, c); }

pub fn push_sphere_pub(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], r: f32, seg: u32, c: [f32; 4],
) { push_sphere(v, idx, p, r, seg, c); }

fn push_box(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], w: f32, h: f32, d: f32, c: [f32; 4],
) {
    let [x, y, z] = p;
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([0.0,0.0,-1.0], [[x,y,z],[x+w,y,z],[x+w,y+h,z],[x,y+h,z]]),
        ([0.0,0.0, 1.0], [[x+w,y,z+d],[x,y,z+d],[x,y+h,z+d],[x+w,y+h,z+d]]),
        ([0.0, 1.0,0.0], [[x,y+h,z],[x+w,y+h,z],[x+w,y+h,z+d],[x,y+h,z+d]]),
        ([0.0,-1.0,0.0], [[x,y,z+d],[x+w,y,z+d],[x+w,y,z],[x,y,z]]),
        ([-1.0,0.0,0.0], [[x,y,z+d],[x,y,z],[x,y+h,z],[x,y+h,z+d]]),
        ([ 1.0,0.0,0.0], [[x+w,y,z],[x+w,y,z+d],[x+w,y+h,z+d],[x+w,y+h,z]]),
    ];
    for (n, vs) in &faces {
        let base = v.len() as u32;
        for p in vs {
            v.push(Vertex { position: *p, normal: *n, color: c });
        }
        idx.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
    }
}

fn push_cylinder(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], r: f32, h: f32, seg: u32, c: [f32; 4],
) {
    let [cx, cy, cz] = p;
    let seg = seg.max(6);

    // Side faces
    for i in 0..seg {
        let a0 = (i as f32 / seg as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / seg as f32) * std::f32::consts::TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        // Smooth per-vertex normals: each vertex gets its own radial normal
        let n0 = [c0, 0.0, s0]; // normal for vertices at angle0
        let n1 = [c1, 0.0, s1]; // normal for vertices at angle1
        let base = v.len() as u32;
        v.push(Vertex { position: [cx + r*c0, cy,     cz + r*s0], normal: n0, color: c });
        v.push(Vertex { position: [cx + r*c1, cy,     cz + r*s1], normal: n1, color: c });
        v.push(Vertex { position: [cx + r*c1, cy + h, cz + r*s1], normal: n1, color: c });
        v.push(Vertex { position: [cx + r*c0, cy + h, cz + r*s0], normal: n0, color: c });
        idx.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
    }

    // Top & bottom caps
    let top_n = [0.0, 1.0, 0.0];
    let bot_n = [0.0, -1.0, 0.0];
    let top_center = v.len() as u32;
    v.push(Vertex { position: [cx, cy + h, cz], normal: top_n, color: c });
    let bot_center = v.len() as u32;
    v.push(Vertex { position: [cx, cy, cz], normal: bot_n, color: c });

    for i in 0..seg {
        let a0 = (i as f32 / seg as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / seg as f32) * std::f32::consts::TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();

        // top
        let b = v.len() as u32;
        v.push(Vertex { position: [cx + r*c0, cy+h, cz + r*s0], normal: top_n, color: c });
        v.push(Vertex { position: [cx + r*c1, cy+h, cz + r*s1], normal: top_n, color: c });
        idx.extend_from_slice(&[top_center, b, b+1]);

        // bottom
        let b = v.len() as u32;
        v.push(Vertex { position: [cx + r*c1, cy, cz + r*s1], normal: bot_n, color: c });
        v.push(Vertex { position: [cx + r*c0, cy, cz + r*s0], normal: bot_n, color: c });
        idx.extend_from_slice(&[bot_center, b, b+1]);
    }
}

fn push_sphere(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], r: f32, seg: u32, c: [f32; 4],
) {
    let [cx, cy, cz] = p;
    let rings = seg.max(4);
    let slices = seg.max(6);

    let base = v.len() as u32;

    for ring in 0..=rings {
        let phi = std::f32::consts::PI * ring as f32 / rings as f32;
        let (sp, cp) = phi.sin_cos();
        for slice in 0..=slices {
            let theta = std::f32::consts::TAU * slice as f32 / slices as f32;
            let (st, ct) = theta.sin_cos();
            let nx = sp * ct;
            let ny = cp;
            let nz = sp * st;
            v.push(Vertex {
                position: [cx + r*nx, cy + r + r*ny, cz + r*nz],
                normal: [nx, ny, nz],
                color: c,
            });
        }
    }

    for ring in 0..rings {
        for slice in 0..slices {
            let a = base + ring * (slices + 1) + slice;
            let b = a + slices + 1;
            idx.extend_from_slice(&[a, a+1, b,  b, a+1, b+1]);
        }
    }
}

fn push_line_segments(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    points: &[[f32; 3]], thickness: f32, c: [f32; 4],
) {
    let half = thickness * 0.5;
    for pair in points.windows(2) {
        let a = glam::Vec3::from(pair[0]);
        let b = glam::Vec3::from(pair[1]);
        let dir = b - a;
        if dir.length_squared() < 0.01 { continue; }

        // Build a thin box along the segment
        let fwd = dir.normalize();
        let up = if fwd.y.abs() > 0.99 { glam::Vec3::Z } else { glam::Vec3::Y };
        let right = fwd.cross(up).normalize() * half;
        let up2 = right.cross(fwd).normalize() * half;

        let corners = [
            a - right - up2, a + right - up2, a + right + up2, a - right + up2,
            b - right - up2, b + right - up2, b + right + up2, b - right + up2,
        ];

        let base = v.len() as u32;
        let faces: [([f32; 3], [usize; 4]); 6] = [
            ((-fwd).into(), [0,3,2,1]),   // front
            (fwd.into(),    [4,5,6,7]),    // back
            (up2.normalize().into(),  [3,7,6,2]),   // top
            ((-up2).normalize().into(), [0,1,5,4]), // bottom
            ((-right).normalize().into(), [0,4,7,3]), // left
            (right.normalize().into(), [1,2,6,5]),    // right
        ];

        for (n, fi) in &faces {
            let i = v.len() as u32;
            for &ci in fi {
                v.push(Vertex { position: corners[ci].into(), normal: *n, color: c });
            }
            idx.extend_from_slice(&[i, i+1, i+2, i, i+2, i+3]);
        }
        let _ = base;
    }
}
