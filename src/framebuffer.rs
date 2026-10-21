// framebuffer.rs
// La imagen en RAM y como se sube a la ventana.
use raylib::prelude::*;

pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    /// Pixeles en RAM, RGBA8 plano (4 bytes por pixel). Escribir aca es solo
    /// memoria: no cruza el FFI de raylib como lo hacia Image::draw_pixel.
    pixels: Vec<u8>,
    /// Profundidad de cada pixel, en unidades del mundo.
    depth: Vec<f32>,
    background_color: Vector3,
    /// Textura en GPU reutilizada frame a frame; se crea una sola vez.
    texture: Option<Texture2D>,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let size = width as usize * height as usize;
        Framebuffer {
            width,
            height,
            pixels: vec![0; size * 4],
            depth: vec![f32::INFINITY; size],
            background_color: Vector3::new(0.0, 0.0, 0.0),
            texture: None,
        }
    }

    pub fn set_background_color(&mut self, color: Vector3) {
        self.background_color = color;
    }

    /// Pintar todo el fondo
    pub fn clear(&mut self) {
        let background = [
            to_byte(self.background_color.x),
            to_byte(self.background_color.y),
            to_byte(self.background_color.z),
            255,
        ];
        for pixel in self.pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&background);

        }
        self.depth.fill(f32::INFINITY);
    }

    
    
    fn index(&self, x: i32, y: i32) -> Option<usize> {
    
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    /// Pintar un pixel en el framebuffer. 
    pub fn point(&mut self, x: i32, y: i32, color: Vector3) {
        let Some(index) = self.index(x, y) else { return };

        let pixel = &mut self.pixels[index * 4..index* 4 + 4];
        pixel[0] = to_byte(color.x);
        pixel[1] = to_byte(color.y);
        pixel[2] = to_byte(color.z);
        pixel[3] = 255;
    }

    /// prueba de profundidad
    
    pub fn depth_test(&mut self, x: i32, y: i32, depth: f32) -> bool {
        let Some(index) = self.index(x, y) else { return false };

        if depth < self.depth[index] {
            self.depth[index] = depth;
            true
        } else {
            false
        }
    }

    /// crear la textura
    pub fn init_texture(&mut self, window: &mut RaylibHandle, thread: &RaylibThread) {
        let image = Image::gen_image_color(self.width as i32, self.height as i32, Color::BLACK);
        self.texture = window.load_texture_from_image(thread, &image).ok();
    }

    /// Manda el buffer a la ventana actualizando la textura existente.
    pub fn swap_buffers(&mut self, window: &mut RaylibHandle, raylib_thread: &RaylibThread) {
        if self.texture.is_none() {
            self.init_texture(window, raylib_thread);
        }

        // destructuramos para poder leer pixels mientras mutamos la textura
        let Framebuffer { pixels, texture, .. } = self;
        let Some(texture) = texture.as_mut() else { return };
        let _ = texture.update_texture(pixels);

        let mut renderer = window.begin_drawing(raylib_thread);
        renderer.clear_background(Color::BLACK);
        renderer.draw_texture(texture, 0, 0, Color::WHITE);
    }

    /// Guarda el cuadro como PNG.
    pub fn save(&self, path: &str) -> Result<(), String> {
        let mut image =
            Image::gen_image_color(self.width as i32, self.height as i32, Color::BLACK);

        for (index, pixel) in self.pixels.chunks_exact(4).enumerate() {
            let x = (index % self.width as usize) as i32;
            let y = (index / self.width as usize) as i32;
            image.draw_pixel(x, y, Color::new(pixel[0], pixel[1], pixel[2], pixel[3]));
        }

        image.export_image(path);
        if std::path::Path::new(path).exists() {
            return Ok(());
        }

        Err(format!("no se pudo escribir {path}"))
    }
}

/// Convierte un float en [0,1] a un byte en [0,255].
fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0) as u8
}
