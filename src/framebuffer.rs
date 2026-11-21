// framebuffer.rs
// La imagen en RAM y como se sube a la ventana.
use raylib::prelude::*;
use std::sync::Mutex;

use crate::pool::Pool;

pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    /// Pixeles en RAM, RGBA8 plano (4 bytes por pixel). Escribir aca es solo
    /// memoria: no cruza el FFI de raylib como lo hacia Image::draw_pixel.
    pixels: Vec<u8>,
    /// Índices y colores de las estrellas de fondo, calculados una sola vez.
    starfield: Vec<(usize, [u8; 3])>,
    /// Profundidad de cada pixel, en unidades del mundo.
    depth: Vec<f32>,
    background_color: Vector3,
    /// Textura en GPU reutilizada frame a frame; se crea una sola vez.
    texture: Option<Texture2D>,
    /// Hilos que reparten el fragment shader; tambien se crean una sola vez.
    pool: Pool,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let size = width as usize * height as usize;
        let starfield = (0..size)
            .filter_map(|index| {
                let hash = star_hash(index);
                if hash % 900 != 0 { return None; }
                let brightness = 85 + ((hash >> 24) as u8 % 150);
                Some((index, [
                    brightness.saturating_sub(8),
                    brightness.saturating_sub(3),
                    brightness,
                ]))
            })
            .collect();
        Framebuffer {
            width,
            height,
            pixels: vec![0; size * 4],
            starfield,
            depth: vec![f32::INFINITY; size],
            background_color: Vector3::new(0.0, 0.0, 0.0),
            texture: None,
            pool: Pool::new(std::thread::available_parallelism().map_or(1, |count| count.get())),
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

    /// Limpia el framebuffer y dibuja un campo estelar determinista en CPU.
    /// Usar el mismo patrón en cada cuadro evita que las estrellas titilen.
    pub fn clear_with_starfield(&mut self) {
        self.clear();
        for (index, [red, green, blue]) in &self.starfield {
            let offset = *index * 4;
            self.pixels[offset..offset + 4].copy_from_slice(&[*red, *green, *blue, 255]);
        }
    }

    /// Añade una aureola radial aditiva alrededor de una coordenada de
    /// pantalla. Se usa para el brillo estelar sin recurrir a shaders GPU.
    pub fn add_glow(&mut self, center_x: i32, center_y: i32, radius: f32, color: Color, strength: f32) {
        let radius = radius.max(1.0);
        let extent = radius.ceil() as i32;
        let min_x = (center_x - extent).max(0);
        let max_x = (center_x + extent).min(self.width as i32 - 1);
        let min_y = (center_y - extent).max(0);
        let max_y = (center_y + extent).min(self.height as i32 - 1);

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = (x - center_x) as f32 / radius;
                let dy = (y - center_y) as f32 / radius;
                let distance_squared = dx * dx + dy * dy;
                if distance_squared > 1.0 { continue; }
                let falloff = (-4.5 * distance_squared).exp() * strength;
                let index = (y as usize * self.width as usize + x as usize) * 4;
                self.pixels[index] = (self.pixels[index] as f32 + color.r as f32 * falloff).min(255.0) as u8;
                self.pixels[index + 1] = (self.pixels[index + 1] as f32 + color.g as f32 * falloff).min(255.0) as u8;
                self.pixels[index + 2] = (self.pixels[index + 2] as f32 + color.b as f32 * falloff).min(255.0) as u8;
            }
        }
    }

    /// Rectangulo relleno, recortado a la pantalla. El HUD tambien se dibuja
    /// aca, en CPU, como todo lo demas.
    pub fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        let (x0, x1) = (x.max(0), (x + width).min(self.width as i32));
        let (y0, y1) = (y.max(0), (y + height).min(self.height as i32));
        for row in y0..y1 {
            for column in x0..x1 {
                let index = (row as usize * self.width as usize + column as usize) * 4;
                self.pixels[index..index + 4].copy_from_slice(&[color.r, color.g, color.b, 255]);
            }
        }
    }

    /// Un pixel suelto, si cae dentro de la pantalla.
    pub fn plot(&mut self, x: i32, y: i32, color: Color) {
        if let Some(index) = self.index(x, y) {
            self.pixels[index * 4..index * 4 + 4].copy_from_slice(&[color.r, color.g, color.b, 255]);
        }
    }

    /// Linea entre dos puntos de pantalla (x e y en pixeles, z la
    /// profundidad). Respeta lo que ya se dibujo mas cerca pero no escribe la
    /// profundidad: las orbitas quedan detras de los planetas y no se tapan
    /// entre ellas.
    pub fn line(&mut self, from: Vector3, to: Vector3, color: Color) {
        let Some((from, to)) = clip_to_screen(from, to, self.width as f32, self.height as f32) else {
            return;
        };
        let steps = (to.x - from.x).abs().max((to.y - from.y).abs()).ceil().max(1.0);
        for step in 0..=steps as i32 {
            let point = from.lerp(to, step as f32 / steps);
            let Some(index) = self.index(point.x as i32, point.y as i32) else { continue };
            if point.z < self.depth[index] {
                self.pixels[index * 4..index * 4 + 4].copy_from_slice(&[color.r, color.g, color.b, 255]);
            }
        }
    }

    /// Anillo de `radius` pixeles de radio y `thickness` de grosor, con el
    /// borde suavizado y mezclado con lo que ya hay segun `alpha`. Con radio
    /// 0 es un disco. `visible(angulo)` elige que partes se dibujan, para
    /// hacer arcos y marcas; el angulo sale de atan2, 0 a la derecha.
    pub fn ring(
        &mut self,
        center_x: f32,
        center_y: f32,
        radius: f32,
        thickness: f32,
        color: Color,
        alpha: f32,
        visible: impl Fn(f32) -> bool,
    ) {
        let reach = radius + thickness;
        let min_x = (center_x - reach).floor().max(0.0) as i32;
        let max_x = (center_x + reach).ceil().min(self.width as f32 - 1.0) as i32;
        let min_y = (center_y - reach).floor().max(0.0) as i32;
        let max_y = (center_y + reach).ceil().min(self.height as f32 - 1.0) as i32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 + 0.5 - center_x;
                let dy = y as f32 + 0.5 - center_y;
                // cuanto del pixel cubre el trazo: 1 adentro, 0 a un pixel del borde
                let distance = (dx * dx + dy * dy).sqrt();
                let coverage = (thickness / 2.0 + 0.5 - (distance - radius).abs()).clamp(0.0, 1.0);
                if coverage > 0.0 && visible(dy.atan2(dx)) {
                    self.blend(x, y, color, coverage * alpha);
                }
            }
        }
    }

    /// Mezcla `color` sobre el pixel: 0 lo deja igual, 1 lo reemplaza.
    fn blend(&mut self, x: i32, y: i32, color: Color, amount: f32) {
        let Some(index) = self.index(x, y) else { return };
        let pixel = &mut self.pixels[index * 4..index * 4 + 3];
        for (channel, target) in pixel.iter_mut().zip([color.r, color.g, color.b]) {
            *channel = (*channel as f32 + (target as f32 - *channel as f32) * amount) as u8;
        }
    }

    /// Borde de un rectangulo, `thickness` pixeles hacia adentro.
    pub fn outline(&mut self, x: i32, y: i32, width: i32, height: i32, thickness: i32, color: Color) {
        self.fill_rect(x, y, width, thickness, color);
        self.fill_rect(x, y + height - thickness, width, thickness, color);
        self.fill_rect(x, y, thickness, height, color);
        self.fill_rect(x + width - thickness, y, thickness, height, color);
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
    
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    /// Prueba de profundidad. Si el fragmento queda delante devuelve el
    /// indice de su pixel.
    pub fn depth_test(&mut self, x: i32, y: i32, depth: f32) -> Option<usize> {
        let index = self.index(x, y)?;

        if depth < self.depth[index] {
            self.depth[index] = depth;
            Some(index)
        } else {
            None
        }
    }

    /// Pinta cada pixel con `color_at(indice)`; los que devuelven None quedan
    /// con el fondo. Las filas se reparten entre los hilos intercaladas para
    /// que todos reciban parte de la esfera.
    pub fn shade(&mut self, color_at: impl Fn(usize) -> Option<Color> + Sync) {
        let width = self.width as usize;
        let Framebuffer { pixels, pool, .. } = self;
        let threads = pool.threads();

        let mut work: Vec<Vec<(usize, &mut [u8])>> = (0..threads).map(|_| Vec::new()).collect();
        for (y, row) in pixels.chunks_exact_mut(width * 4).enumerate() {
            work[y % threads].push((y, row));
        }
        // cada hilo toma solo su propio lote, asi que los candados nunca se
        // disputan: estan para que el compilador acepte prestar filas
        // mutables a una funcion compartida
        let work: Vec<Mutex<_>> = work.into_iter().map(Mutex::new).collect();

        pool.run(&|thread| {
            for (y, row) in work[thread].lock().unwrap().iter_mut() {
                for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
                    if let Some(color) = color_at(*y * width + x) {
                        pixel.copy_from_slice(&[color.r, color.g, color.b, 255]);
                    }
                }
            }
        });
    }

    /// crear la textura
    pub fn init_texture(&mut self, window: &mut RaylibHandle, thread: &RaylibThread) {
        let image = Image::gen_image_color(self.width as i32, self.height as i32, Color::BLACK);
        self.texture = window.load_texture_from_image(thread, &image).ok();
    }

    /// Manda el buffer a la ventana actualizando la textura existente.
    /// `overlay` dibuja encima (texto); no queda en el buffer ni en las capturas.
    pub fn swap_buffers(
        &mut self,
        window: &mut RaylibHandle,
        raylib_thread: &RaylibThread,
        overlay: impl FnOnce(&mut RaylibDrawHandle),
    ) {
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
        overlay(&mut renderer);
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

/// Recorta el segmento al rectangulo de la pantalla (Liang-Barsky): una
/// orbita que pasa al lado de la camara puede proyectarse a miles de pixeles
/// y no hace falta recorrer los que quedan afuera.
fn clip_to_screen(from: Vector3, to: Vector3, width: f32, height: f32) -> Option<(Vector3, Vector3)> {
    let delta = to - from;
    let (mut enter, mut exit) = (0.0_f32, 1.0_f32);
    let edges = [
        (-delta.x, from.x),
        (delta.x, width - 1.0 - from.x),
        (-delta.y, from.y),
        (delta.y, height - 1.0 - from.y),
    ];
    for (direction, room) in edges {
        if direction == 0.0 {
            if room < 0.0 {
                return None;
            }
            continue;
        }
        let t = room / direction;
        if direction < 0.0 {
            enter = enter.max(t);
        } else {
            exit = exit.min(t);
        }
    }
    (enter <= exit).then(|| (from + delta * enter, from + delta * exit))
}

pub fn star_hash(index: usize) -> u32 {
    let mut hash = (index as u32).wrapping_add(0x9E37_79B9);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7FEB_352D);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846C_A68B);
    hash ^ (hash >> 16)
}