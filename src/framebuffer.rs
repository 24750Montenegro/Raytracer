use raylib::prelude::*;

pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    color_buffer: Image,        // la imagen en RAM donde realmente pintamos
    background_color: Color,
    current_color: Color,       // el color "activo" del pincel
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        // gen_image_color pide i32, nosotros guardamos u32 -> hay que castear
        let color_buffer = Image::gen_image_color(
            width as i32,
            height as i32,
            Color::BLACK,
        );

        Framebuffer {
            width,
            height,
            color_buffer,
            background_color: Color::BLACK,
            current_color: Color::WHITE,
        }
    }

    /// Borra todo el buffer al color de fondo. Se llama al inicio de cada frame.
    pub fn clear(&mut self) {
        self.color_buffer = Image::gen_image_color(
            self.width as i32,
            self.height as i32,
            self.background_color,
        );
    }

    /// Cambia el color con el que van a pintar los siguientes set_pixel.
    pub fn set_current_color(&mut self, color: Color) {
        self.current_color = color;
    }

    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
    }

    /// Pinta UN píxel con el color actual. Ignora coordenadas fuera de pantalla.
    pub fn set_pixel(&mut self, x: u32, y: u32) {
        if x < self.width && y < self.height {
            self.color_buffer.draw_pixel(x as i32, y as i32, self.current_color);
        }
    }

    /// Igual que set_pixel pero con color explícito (evita andar cambiando el pincel).
    pub fn set_pixel_color(&mut self, x: u32, y: u32, color: Color) {
        if x < self.width && y < self.height {
            self.color_buffer.draw_pixel(x as i32, y as i32, color);
        }
    }

    /// Manda el buffer a la ventana.
    pub fn swap_buffers(&self, window: &mut RaylibHandle, raylib_thread: &RaylibThread) {
        if let Ok(texture) = window.load_texture_from_image(raylib_thread, &self.color_buffer) {
            let mut renderer = window.begin_drawing(raylib_thread);
            renderer.clear_background(Color::BLACK);
            renderer.draw_texture(&texture, 0, 0, Color::WHITE);
        }
    }

    /// Para entregar capturas de pantalla de la tarea.
    pub fn render_to_file(&self, file_path: &str) {
        self.color_buffer.export_image(file_path);
    }
}