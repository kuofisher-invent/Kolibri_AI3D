use eframe::wgpu;
use bytemuck::Pod;
use super::shaders::*;

// ─── Texture helpers ─────────────────────────────────────────────────────────

pub(crate) fn create_textures(
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

    // MSAA=1: 不需要獨立 MSAA texture，用 color texture 直接渲染
    let msaa = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("vp_msaa"), size,
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR_FMT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let msaa_view = msaa.create_view(&Default::default());

    // Depth texture（sample_count 1 匹配 MSAA）
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("vp_depth"), size,
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FMT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth.create_view(&Default::default());

    (color, color_view, msaa, msaa_view, depth_view)
}

// ─── Buffer helpers ──────────────────────────────────────────────────────────

pub(crate) fn make_buffer<T: Pod>(
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
pub(crate) fn make_index_buffer(device: &wgpu::Device, queue: &wgpu::Queue, data: &[u32]) -> wgpu::Buffer {
    make_buffer(device, queue, data, wgpu::BufferUsages::INDEX)
}

/// Reuse an existing GPU buffer if it has enough capacity, otherwise allocate a new one.
/// This avoids creating a new wgpu::Buffer every frame when the data size hasn't grown.
pub(crate) fn reuse_or_grow_buffer(
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
    // Cap at 256MB (wgpu default maxBufferSize)
    let alloc_size = ((needed as f64 * 1.5) as usize).min(256 * 1024 * 1024);
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

pub(crate) fn generate_grid(size: f32, step: f32) -> Vec<Vertex> {
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

