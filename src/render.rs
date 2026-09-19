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

/// Filas por tile. Tiles chicos = mejor reparto (una franja con muchos
/// bloques cuesta bastante mas que uno de puro fondo), pero demasiado
/// chicos y se paga el lock mas seguido. 8 filas es un punto sano.
const TILE_ROWS: usize = 8;

/// Que tan fino se traza un frame.
///
/// Con reflejos y refracciones un frame completo cuesta varias veces mas que
/// antes, y mientras se arrastra la camara la imagen cambia entera en cada
/// cuadro: no tiene sentido pagarla toda para que dure 16 ms. Mientras algo
/// se mueve se traza en grueso, y al soltar se hace un unico pase fino.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quality {
    /// Cada cuantos pixeles se tira un rayo. Los del medio copian al vecino,
    /// asi que stride 4 son 16 veces menos rayos primarios.
    pub stride: usize,
    /// Rebotes permitidos por rayo.
    pub depth: u32,
}

impl Quality {
    /// Frame definitivo: un rayo por pixel y la cadena entera de rebotes.
    pub const FULL: Quality = Quality { stride: 1, depth: shading::MAX_DEPTH };

    /// Mientras la escena se mueve. El stride divide a TILE_ROWS para que el
    /// patron no se corte en el borde de cada tile.
    pub const MOVING: Quality = Quality { stride: 4, depth: 1 };
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
    quality: Quality,
) {
    let aspect_ratio = width as f32 / height as f32;
    let perspective_scale = (projection::FOV / 2.0).tan();
    // constantes de la proyeccion sacadas del loop de pixeles
    let sx_scale = (2.0 / width as f32) * aspect_ratio * perspective_scale;
    let sx_offset = -aspect_ratio * perspective_scale;
    let sy_scale = (2.0 / height as f32) * perspective_scale;
    let sy_offset = -perspective_scale;

    // avanzar un pixel en X es sumar siempre el mismo vector; con stride se
    // avanza de a varios de una
    let step_x = camera.right * (sx_scale * quality.stride as f32);
    let row_bytes = width as usize * 4;
    let block_bytes = 4 * quality.stride;

    // cada bloque son las filas que comparte un mismo trazado
    for (block, rows) in tile.chunks_mut(row_bytes * quality.stride).enumerate() {
        let y = y_start + (block * quality.stride) as u32;
        if y >= height {
            break;
        }

        let screen_y = y as f32 * sy_scale + sy_offset;
        // el rayo del primer pixel de la fila; los demas salen sumando step_x
        let mut dir = camera.forward + camera.down * screen_y + camera.right * sx_offset;

        let (traced_row, copies) = rows.split_at_mut(row_bytes);

        for pixel_block in traced_row.chunks_mut(block_bytes) {
            // normalizamos aca en vez de armar el rayo con helpers: evita
            // recalcular la base de la camara un millon de veces por frame
            let inv_len = 1.0 / (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z).sqrt();
            let ray_direction = Vector3::new(dir.x * inv_len, dir.y * inv_len, dir.z * inv_len);

            let color =
                shading::trace(scene, camera.origin, ray_direction, quality.depth).to_color();

            // el rayo pinta su pixel y, si hay stride, los que lo acompanan
            for pixel in pixel_block.chunks_exact_mut(4) {
                pixel[0] = color.r;
                pixel[1] = color.g;
                pixel[2] = color.b;
                pixel[3] = color.a;
            }

            dir = dir + step_x;
        }

        // las filas del medio repiten la que se acaba de trazar
        for row in copies.chunks_exact_mut(row_bytes) {
            row.copy_from_slice(traced_row);
        }
    }
}

pub fn render(
    framebuffer: &mut Framebuffer,
    scene: &Scene,
    camera: &CameraBasis,
    quality: Quality,
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
        render_tile(buffer, 0, width, height, scene, camera, quality);
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
                render_tile(tile, y_start, width, height, scene, camera, quality);
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bvh::Bvh;
    use crate::camera::Camera;
    use crate::color::Rgb;
    use crate::light::Light;
    use crate::material::Material;
    use crate::object::Object;

    /// Un bloque en el medio, para que el frame tenga fondo y objeto.
    fn test_scene() -> Scene {
        let mut scene = Scene::for_test(
            vec![Light::new(
                Vector3::new(0.0, -6.0, 0.0),
                Rgb::new(1.0, 1.0, 1.0),
                1.0,
            )],
            Rgb::new(0.1, 0.1, 0.1),
        );
        scene.objects = vec![Object::cube(
            Vector3::new(0.0, 0.0, -10.0),
            6.0,
            Material::plastic(Color::new(200, 120, 60, 255)),
        )];
        scene.bvh = Bvh::build(&mut scene.objects);
        scene
    }

    fn render_to_vec(width: u32, height: u32, quality: Quality) -> Vec<u8> {
        let scene = test_scene();
        let camera = Camera::new(Vector3::new(0.0, 0.0, -10.0), 12.0);
        let mut framebuffer = Framebuffer::new(width, height);

        render(&mut framebuffer, &scene, &camera.basis(), quality);
        framebuffer.pixels_mut().to_vec()
    }

    /// Ningun pixel puede quedar sin escribir, ni con medidas que no son
    /// multiplos del stride ni de las filas por tile.
    #[test]
    fn every_pixel_gets_written_at_any_quality() {
        for quality in [Quality::FULL, Quality::MOVING] {
            let pixels = render_to_vec(37, 21, quality);

            assert_eq!(pixels.len(), 37 * 21 * 4);
            for (i, pixel) in pixels.chunks_exact(4).enumerate() {
                assert_eq!(pixel[3], 255, "pixel {i} quedo sin trazar con {quality:?}");
            }
        }
    }

    /// Con stride, el rayo pinta su bloque entero: los vecinos copian.
    #[test]
    fn stride_copies_the_traced_pixel_across_its_block() {
        let width = 64usize;
        let height = 32usize;
        let stride = Quality::MOVING.stride;
        let pixels = render_to_vec(width as u32, height as u32, Quality::MOVING);

        let pixel_at = |x: usize, y: usize| {
            let i = (y * width + x) * 4;
            [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
        };

        for y in (0..height).step_by(stride) {
            for x in (0..width).step_by(stride) {
                let traced = pixel_at(x, y);
                for dy in 0..stride.min(height - y) {
                    for dx in 0..stride.min(width - x) {
                        assert_eq!(pixel_at(x + dx, y + dy), traced, "bloque en ({x}, {y})");
                    }
                }
            }
        }
    }

    /// Es lo unico que ata la camara inicial con el tamano de la escena que
    /// sortea el ruido: sin esto podria salir un cielo vacio.
    #[test]
    fn the_opening_view_frames_the_terrain() {
        let scene = Scene::from_seed(42);
        let camera = Camera::framing(scene.center, scene.radius);
        let mut framebuffer = Framebuffer::new(96, 96);

        render(&mut framebuffer, &scene, &camera.basis(), Quality::FULL);

        let sky = scene.background.to_color();
        let pixels = framebuffer.pixels_mut();
        let terrain = pixels
            .chunks_exact(4)
            .filter(|pixel| pixel[0] != sky.r || pixel[1] != sky.g || pixel[2] != sky.b)
            .count();

        let share = terrain as f32 / (96.0 * 96.0);
        assert!(
            (0.25..0.95).contains(&share),
            "el terreno ocupa el {:.0}% del cuadro",
            share * 100.0
        );
    }

    /// El frame fino no puede quedar en bloques: con un cuerpo en el medio
    /// tiene que haber filas y columnas vecinas distintas.
    #[test]
    fn full_quality_traces_every_pixel_on_its_own() {
        let width = 64usize;
        let pixels = render_to_vec(width as u32, 64, Quality::FULL);

        let row = 32usize;
        let neighbours_differ = (0..width - 1).any(|x| {
            let a = (row * width + x) * 4;
            let b = (row * width + x + 1) * 4;
            pixels[a..a + 3] != pixels[b..b + 3]
        });

        assert!(neighbours_differ, "el borde del cuerpo tendria que variar pixel a pixel");
    }
}
