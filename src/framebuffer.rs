use raylib::prelude::*;

pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    /// Pixeles en RAM, RGBA8 plano (4 bytes por pixel). Escribir aca es solo
    /// memoria: no cruza el FFI de raylib como lo hacia Image::draw_pixel.
    pixels: Vec<u8>,
    background_color: Color,
    /// Textura en GPU reutilizada frame a frame; se crea una sola vez.
    texture: Option<Texture2D>,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Framebuffer {
            width,
            height,
            pixels: vec![0; (width as usize) * (height as usize) * 4],
            background_color: Color::BLACK,
            texture: None,
        }
    }

    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
    }

    /// Acceso directo al buffer para que el render escriba en paralelo.
    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    /// Manda el buffer a la ventana actualizando la textura existente.
    pub fn swap_buffers(&mut self, window: &mut RaylibHandle, raylib_thread: &RaylibThread) {
        if self.texture.is_none() {
            let image = Image::gen_image_color(
                self.width as i32,
                self.height as i32,
                self.background_color,
            );
            match window.load_texture_from_image(raylib_thread, &image) {
                Ok(texture) => self.texture = Some(texture),
                Err(_) => return,
            }
        }

        // destructuramos para poder leer pixels mientras mutamos la textura
        let Framebuffer { pixels, texture, .. } = self;
        let Some(texture) = texture.as_mut() else { return };
        let _ = texture.update_texture(pixels);

        let mut renderer = window.begin_drawing(raylib_thread);
        renderer.clear_background(Color::BLACK);
        renderer.draw_texture(texture, 0, 0, Color::WHITE);
    }

    /// Guarda el cuadro como PNG. Es para sacar capturas sin depender de que
    /// alguien tenga la ventana abierta en el momento justo.
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
