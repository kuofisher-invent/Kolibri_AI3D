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
pub(crate) struct Uniforms {
    pub(crate) view_proj: [[f32; 4]; 4],
    pub(crate) sky_color: [f32; 4],
    pub(crate) ground_color: [f32; 4],
    pub(crate) camera_pos: [f32; 4],
    pub(crate) light_vp: [[f32; 4]; 4],
    pub(crate) section_plane: [f32; 4],
}

pub const COLOR_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
pub(crate) const DEPTH_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub(crate) const SHADOW_MAP_SIZE: u32 = 2048;
pub(crate) const SHADOW_DEPTH_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

// Shadow depth-only shader (vertex only, no fragment output)
pub(crate) const SHADOW_SHADER: &str = r#"
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

pub(crate) const SHADER: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    sky_color: vec4<f32>,
    ground_color: vec4<f32>,
    camera_pos: vec4<f32>,
    light_vp: mat4x4<f32>,
    section_plane: vec4<f32>,  // [axis, offset, flip, enabled]
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
    // Section plane clipping
    if u.section_plane.w > 0.5 {
        let axis = u.section_plane.x;
        let offset = u.section_plane.y;
        let flip = u.section_plane.z;
        var coord: f32;
        if axis < 0.5 {
            coord = i.world_pos.x;
        } else if axis < 1.5 {
            coord = i.world_pos.y;
        } else {
            coord = i.world_pos.z;
        }
        let beyond = select(coord - offset, offset - coord, flip > 0.5);
        if beyond > 0.0 {
            discard;
        }
    }

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
    // Clay-style ambient: bright, soft, minimal harsh shadows
    let hemisphere = 0.5 + 0.5 * dot(n, vec3<f32>(0.0, 1.0, 0.0));
    let ambient = 0.45 + 0.15 * hemisphere;

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
            shadow = mix(0.6, 1.0, shadow); // Clay: 陰影更柔和
        }
    }

    // Clay-style 合成：明亮 diffuse + 柔和 specular + 輕陰影
    let kd = (1.0 - metallic) * base_color;
    let diffuse_term = kd * (ambient + 0.45 * ndl * shadow);
    let spec_term = specular * ndl * shadow * 0.3; // 降低高光強度
    var col = vec4<f32>(diffuse_term + spec_term, final_alpha);

    return col;
}
"#;

// Sky gradient shader (fullscreen quad)
pub(crate) const SKY_SHADER: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    sky_color: vec4<f32>,
    ground_color: vec4<f32>,
    camera_pos: vec4<f32>,
    light_vp: mat4x4<f32>,
    section_plane: vec4<f32>,
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

