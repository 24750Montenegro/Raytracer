use raylib::prelude::*;

/// Buffer de píxeles RGBA en RAM que se sube cada frame a una única textura de GPU.
pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pixels: Vec<u8>,
    texture: Texture2D,
}

impl Framebuffer {
    pub fn new(
        window: &mut RaylibHandle,
        raylib_thread: &RaylibThread,
        width: u32,
        height: u32,
    ) -> Self {
        let blank = Image::gen_image_color(width as i32, height as i32, Color::BLACK);
        let texture = window
            .load_texture_from_image(raylib_thread, &blank)
            .expect("no se pudo crear la textura del framebuffer");

        Framebuffer {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
            texture,
        }
    }

    #[inline]
    fn offset(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some(((y as u32 * self.width + x as u32) * 4) as usize)
    }

    #[inline]
    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        if let Some(i) = self.offset(x, y) {
            self.pixels[i..i + 4].copy_from_slice(&[color.r, color.g, color.b, 255]);
        }
    }

    /// Mezcla `color` sobre lo ya dibujado usando su canal alfa como peso.
    #[inline]
    pub fn blend_pixel(&mut self, x: i32, y: i32, color: Color) {
        if color.a == 255 {
            return self.set_pixel(x, y, color);
        }
        if color.a == 0 {
            return;
        }
        if let Some(i) = self.offset(x, y) {
            let src = [color.r as u32, color.g as u32, color.b as u32];
            let alpha = color.a as u32;
            for (channel, value) in src.iter().enumerate() {
                let dst = self.pixels[i + channel] as u32;
                self.pixels[i + channel] = ((value * alpha + dst * (255 - alpha)) / 255) as u8;
            }
        }
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        let left = x.max(0);
        let right = (x + width).min(self.width as i32);
        if right <= left {
            return;
        }
        let rgba = [color.r, color.g, color.b, 255];

        for py in y.max(0)..(y + height).min(self.height as i32) {
            let start = ((py as u32 * self.width + left as u32) * 4) as usize;
            let end = start + (right - left) as usize * 4;

            for pixel in self.pixels[start..end].chunks_exact_mut(4) {
                pixel.copy_from_slice(&rgba);
            }
        }
    }

    pub fn blend_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        for py in y.max(0)..(y + height).min(self.height as i32) {
            for px in x.max(0)..(x + width).min(self.width as i32) {
                self.blend_pixel(px, py, color);
            }
        }
    }

    pub fn draw_line(&mut self, from: Vector2, to: Vector2, color: Color) {
        let delta = to - from;
        let steps = delta.x.abs().max(delta.y.abs()).ceil().max(1.0);
        let advance = delta / steps;

        let mut point = from;
        for _ in 0..=steps as i32 {
            self.blend_pixel(point.x as i32, point.y as i32, color);
            point += advance;
        }
    }

    pub fn fill_circle(&mut self, center: Vector2, radius: f32, color: Color) {
        let r = radius.ceil() as i32;
        let (cx, cy) = (center.x as i32, center.y as i32);

        for dy in -r..=r {
            for dx in -r..=r {
                if (dx * dx + dy * dy) as f32 <= radius * radius {
                    self.blend_pixel(cx + dx, cy + dy, color);
                }
            }
        }
    }

    pub fn present(&mut self, window: &mut RaylibHandle, raylib_thread: &RaylibThread) {
        let _ = self.texture.update_texture(&self.pixels);

        let mut renderer = window.begin_drawing(raylib_thread);
        renderer.clear_background(Color::BLACK);
        renderer.draw_texture(&self.texture, 0, 0, Color::WHITE);
    }
}
