// render.rs
// Trazado de la escena repartido entre todos los nucleos disponibles.
use raylib::prelude::*;
use std::sync::Mutex;
use std::thread;

use crate::camera::CameraBasis;
use crate::framebuffer::Framebuffer;
use crate::projection;
use crate::scene::Scene;
use crate::shading;

pub const BACKGROUND_COLOR: Color = Color { r: 14, g: 27, b: 61, a: 255 };

/// Filas por tile. Tiles chicos = mejor reparto (una franja con muchas
/// esferas cuesta bastante mas que uno de puro fondo), pero demasiado
/// chicos y se paga el lock mas seguido. 8 filas es un punto sano.
const TILE_ROWS: usize = 8;

pub fn cast_ray(ray_origin: &Vector3, ray_direction: &Vector3, scene: &Scene) -> Color {
    let Some((index, distance)) = scene.bvh.nearest_hit(ray_origin, ray_direction, &scene.objects)
    else {
        return BACKGROUND_COLOR; //default para el color de fondo
    };

    // punto y normal solo para la esfera que gano, no para las descartadas
    let sphere = &scene.objects[index];
    let point = *ray_origin + *ray_direction * distance;
    let normal = sphere.normal_at(point);

    // del punto hacia la camara: el especular necesita saber desde donde se mira
    let view_dir = -*ray_direction;

    shading::shade(scene, &sphere.material, point, normal, view_dir)
}

/// Traza un tile: las filas [y_start, y_start + filas del tile).
/// Recibe solo el pedazo de framebuffer que le toca, asi varios hilos
/// escriben sobre el mismo buffer sin pisarse y sin locks por pixel.
fn render_tile(
    tile: &mut [u8],
    y_start: u32,
    width: u32,
    height: u32,
    scene: &Scene,
    camera: &CameraBasis,
) {
    let aspect_ratio = width as f32 / height as f32;
    let perspective_scale = (projection::FOV / 2.0).tan();
    // constantes de la proyeccion sacadas del loop de pixeles
    let sx_scale = (2.0 / width as f32) * aspect_ratio * perspective_scale;
    let sx_offset = -aspect_ratio * perspective_scale;
    let sy_scale = (2.0 / height as f32) * perspective_scale;
    let sy_offset = -perspective_scale;

    // avanzar un pixel en X es sumar siempre el mismo vector
    let step_x = camera.right * sx_scale;
    let row_bytes = width as usize * 4;

    for (row, row_pixels) in tile.chunks_exact_mut(row_bytes).enumerate() {
        let y = y_start + row as u32;
        if y >= height {
            break;
        }

        let screen_y = y as f32 * sy_scale + sy_offset;
        // el rayo del primer pixel de la fila; los demas salen sumando step_x
        let mut dir = camera.forward + camera.down * screen_y + camera.right * sx_offset;

        for pixel in row_pixels.chunks_exact_mut(4) {
            // normalizamos aca en vez de armar el rayo con helpers: evita
            // recalcular la base de la camara un millon de veces por frame
            let inv_len = 1.0 / (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z).sqrt();
            let ray_direction = Vector3::new(dir.x * inv_len, dir.y * inv_len, dir.z * inv_len);

            let color = cast_ray(&camera.origin, &ray_direction, scene);
            pixel[0] = color.r;
            pixel[1] = color.g;
            pixel[2] = color.b;
            pixel[3] = color.a;

            dir = dir + step_x;
        }
    }
}

pub fn render(
    framebuffer: &mut Framebuffer,
    scene: &Scene,
    camera: &CameraBasis,
) {
    let width = framebuffer.width;
    let height = framebuffer.height;
    let row_bytes = width as usize * 4;
    let tile_bytes = TILE_ROWS * row_bytes;

    let threads = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1);

    let buffer = framebuffer.pixels_mut();

    if threads == 1 {
        render_tile(buffer, 0, width, height, scene, camera);
        return;
    }

    // Cola de tiles: cada hilo agarra el siguiente apenas termina el suyo.
    // Repartir los tiles fijos de entrada dejaba a un hilo con la parte
    // cargada de la imagen y a los demas esperandolo.
    let mut tiles: Vec<(u32, &mut [u8])> = buffer
        .chunks_mut(tile_bytes)
        .enumerate()
        .map(|(i, tile)| ((i * TILE_ROWS) as u32, tile))
        .collect();
    tiles.reverse(); // pop() saca el final del vector: asi se traza de arriba a abajo

    let queue = Mutex::new(tiles);
    let threads = threads.min(queue.lock().unwrap().len().max(1));

    thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                // el lock solo protege sacar el siguiente tile, no el trazado
                let next = queue.lock().unwrap_or_else(|e| e.into_inner()).pop();
                let Some((y_start, tile)) = next else { break };
                render_tile(tile, y_start, width, height, scene, camera);
            });
        }
    });
}
