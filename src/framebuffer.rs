use raylib::prelude::*;

pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    /// Pixeles en RAM, RGBA8 plano (4 bytes por pixel). Escribir aca es solo
    /// memoria: no cruza el FFI de raylib como lo hacia Image::draw_pixel.
    pixels: Vec<u8>,
    background_color: Color,
    current_color: Color,
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
            current_color: Color::WHITE,
            texture: None,
        }
    }

    /// Borra todo el buffer al color de fondo (un memset, sin reallocar).
    pub fn clear(&mut self) {
        let bg = [
            self.background_color.r,
            self.background_color.g,
            self.background_color.b,
            self.background_color.a,
        ];
        for px in self.pixels.chunks_exact_mut(4) {
            px.copy_from_slice(&bg);
        }
    }

    /// Cambia el color con el que van a pintar los siguientes set_pixel.
    pub fn set_current_color(&mut self, color: Color) {
        self.current_color = color;
    }

    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
    }

    /// Pinta UN pixel con el color actual. Ignora coordenadas fuera de pantalla.
    pub fn set_pixel(&mut self, x: u32, y: u32) {
        let color = self.current_color;
        self.set_pixel_color(x, y, color);
    }

    /// Igual que set_pixel pero con color explicito (evita andar cambiando el pincel).
    pub fn set_pixel_color(&mut self, x: u32, y: u32, color: Color) {
        if x < self.width && y < self.height {
            let i = ((y as usize) * (self.width as usize) + x as usize) * 4;
            self.pixels[i] = color.r;
            self.pixels[i + 1] = color.g;
            self.pixels[i + 2] = color.b;
            self.pixels[i + 3] = color.a;
        }
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

    /// Para entregar capturas de pantalla de la tarea.
    pub fn render_to_file(&self, file_path: &str) {
        let mut image =
            Image::gen_image_color(self.width as i32, self.height as i32, self.background_color);
        for y in 0..self.height {
            for x in 0..self.width {
                let i = ((y as usize) * (self.width as usize) + x as usize) * 4;
                let color = Color::new(
                    self.pixels[i],
                    self.pixels[i + 1],
                    self.pixels[i + 2],
                    self.pixels[i + 3],
                );
                image.draw_pixel(x as i32, y as i32, color);
            }
        }
        image.export_image(file_path);
    }
}
