//! Texture Manager — loads and caches image textures for materials

use std::collections::HashMap;

/// Cached texture data (CPU-side)
#[derive(Debug, Clone)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 4]>,  // RGBA
    pub path: String,
}

/// Manages loaded textures
#[derive(Default)]
pub struct TextureManager {
    cache: HashMap<String, TextureData>,
}

impl TextureManager {
    pub fn new() -> Self {
        Self { cache: HashMap::new() }
    }

    /// Load a texture from file (PNG/JPG)
    pub fn load(&mut self, path: &str) -> Result<&TextureData, String> {
        if self.cache.contains_key(path) {
            return Ok(&self.cache[path]);
        }

        let img = image::open(path).map_err(|e| format!("無法載入圖片: {}", e))?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();

        let pixels: Vec<[u8; 4]> = rgba.pixels()
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect();

        let tex = TextureData {
            width: w,
            height: h,
            pixels,
            path: path.to_string(),
        };

        self.cache.insert(path.to_string(), tex);
        Ok(&self.cache[path])
    }

    /// Sample a texture at UV coordinates (0-1 range, wrapping)
    pub fn sample(&self, path: &str, u: f32, v: f32) -> [f32; 4] {
        let tex = match self.cache.get(path) {
            Some(t) => t,
            None => return [0.8, 0.8, 0.8, 1.0], // default grey
        };

        // Wrap UV coordinates
        let u = ((u % 1.0) + 1.0) % 1.0;
        let v = ((v % 1.0) + 1.0) % 1.0;

        let x = (u * tex.width as f32) as u32 % tex.width;
        let y = (v * tex.height as f32) as u32 % tex.height;
        let idx = (y * tex.width + x) as usize;

        if idx < tex.pixels.len() {
            let p = tex.pixels[idx];
            [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0]
        } else {
            [0.8, 0.8, 0.8, 1.0]
        }
    }

    /// Sample using triplanar UV projection from world-space position and normal
    pub fn triplanar_sample(&self, path: &str, pos: [f32; 3], normal: [f32; 3], scale: f32) -> [f32; 4] {
        let abs_n = [normal[0].abs(), normal[1].abs(), normal[2].abs()];

        // Choose dominant axis for UV projection
        if abs_n[1] > abs_n[0] && abs_n[1] > abs_n[2] {
            // Y-dominant: use XZ as UV
            self.sample(path, pos[0] * scale, pos[2] * scale)
        } else if abs_n[0] > abs_n[2] {
            // X-dominant: use YZ as UV
            self.sample(path, pos[1] * scale, pos[2] * scale)
        } else {
            // Z-dominant: use XY as UV
            self.sample(path, pos[0] * scale, pos[1] * scale)
        }
    }

    /// Get average color of a texture (for material preview / per-face tinting)
    pub fn average_color(&self, path: &str) -> [f32; 4] {
        let tex = match self.cache.get(path) {
            Some(t) => t,
            None => return [0.8, 0.8, 0.8, 1.0],
        };

        if tex.pixels.is_empty() { return [0.8, 0.8, 0.8, 1.0]; }

        let mut r = 0.0f32;
        let mut g = 0.0f32;
        let mut b = 0.0f32;

        // Sample every Nth pixel for speed
        let step = (tex.pixels.len() / 100).max(1);
        let mut count = 0.0f32;
        for (i, p) in tex.pixels.iter().enumerate() {
            if i % step != 0 { continue; }
            r += p[0] as f32;
            g += p[1] as f32;
            b += p[2] as f32;
            count += 1.0;
        }

        [r / count / 255.0, g / count / 255.0, b / count / 255.0, 1.0]
    }

    /// Check if a texture is loaded
    pub fn is_loaded(&self, path: &str) -> bool {
        self.cache.contains_key(path)
    }

    /// Get texture dimensions
    pub fn info(&self, path: &str) -> Option<(u32, u32)> {
        self.cache.get(path).map(|t| (t.width, t.height))
    }
}
