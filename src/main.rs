// main.rs
use raylib::prelude::*;
use std::thread;

mod framebuffer;
mod material;
mod projection;
mod ray_intersect;
mod scene;
mod sphere;

use framebuffer::Framebuffer;
use sphere::Sphere;

const BACKGROUND_COLOR: Color = Color { r: 14, g: 27, b: 61, a: 255 };

pub fn cast_ray(ray_origin: &Vector3, ray_direction: &Vector3, objects: &[Sphere]) -> Color {
    let mut zbuffer = f32::INFINITY;
    let mut pixel_color = BACKGROUND_COLOR; //default para el color de fondo

    for object in objects {
        if let Some(distance) = object.hit_distance(ray_origin, ray_direction) {
            if distance < zbuffer {
                zbuffer = distance;
                pixel_color = object.material.diffuse;
            }
        }
    }
    pixel_color
}

/// Traza un bloque de filas [y_start, y_start + filas del chunk).
/// Recibe el pedazo de framebuffer que le toca, asi varios hilos escriben
/// sobre el mismo buffer sin pisarse.
fn render_rows(chunk: &mut [u8], y_start: u32, width: u32, height: u32, objects: &[Sphere]) {
    let aspect_ratio = width as f32 / height as f32;
    let perspective_scale = (projection::FOV / 2.0).tan();
    // constantes de la proyeccion sacadas del loop de pixeles
    let sx_scale = (2.0 / width as f32) * aspect_ratio * perspective_scale;
    let sx_offset = -aspect_ratio * perspective_scale;
    let sy_scale = (2.0 / height as f32) * perspective_scale;
    let sy_offset = -perspective_scale;

    let ray_origin = Vector3::zero();
    let row_bytes = width as usize * 4;

    for (row, row_pixels) in chunk.chunks_exact_mut(row_bytes).enumerate() {
        let y = y_start + row as u32;
        if y >= height {
            break;
        }
        let screen_y = y as f32 * sy_scale + sy_offset;

        for (x, pixel) in row_pixels.chunks_exact_mut(4).enumerate() {
            let screen_x = x as f32 * sx_scale + sx_offset;

            // normalizamos aca en vez de llamar a ray_dir (evita recalcular
            // aspect ratio y tangente del fov un millon de veces por frame)
            let inv_len = 1.0 / (screen_x * screen_x + screen_y * screen_y + 1.0).sqrt();
            let ray_direction = Vector3::new(screen_x * inv_len, screen_y * inv_len, -inv_len);

            let color = cast_ray(&ray_origin, &ray_direction, objects);
            pixel[0] = color.r;
            pixel[1] = color.g;
            pixel[2] = color.b;
            pixel[3] = color.a;
        }
    }
}

pub fn render(framebuffer: &mut Framebuffer, objects: &[Sphere]) {
    let width = framebuffer.width;
    let height = framebuffer.height;
    let row_bytes = width as usize * 4;

    let threads = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(height as usize)
        .max(1);
    let rows_per_chunk = (height as usize + threads - 1) / threads;

    let buffer = framebuffer.pixels_mut();

    if threads == 1 {
        render_rows(buffer, 0, width, height, objects);
        return;
    }

    // un hilo por bloque de filas: el raytracing es independiente por pixel
    thread::scope(|scope| {
        for (i, chunk) in buffer.chunks_mut(rows_per_chunk * row_bytes).enumerate() {
            let y_start = (i * rows_per_chunk) as u32;
            scope.spawn(move || render_rows(chunk, y_start, width, height, objects));
        }
    });
}

fn main() {
    let window_width = 1000;
    let window_height = 1000;

    let (mut window, raylib_thread) = raylib::init()
        .size(window_width, window_height)
        .title("Raytracer")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    window.set_target_fps(60);

    let mut framebuffer = Framebuffer::new(window_width as u32, window_height as u32);
    framebuffer.set_background_color(BACKGROUND_COLOR);

    let objects: Vec<Sphere> = scene::preset_scene();
    let mut dirty = true;

    while !window.window_should_close() {
        if dirty {
            // render escribe TODOS los pixeles, no hace falta limpiar antes
            render(&mut framebuffer, &objects);
            dirty = false;
        }

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}
