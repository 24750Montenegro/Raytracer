// main.rs
#![allow(unused_imports)]
#![allow(dead_code)]

use raylib::prelude::*;
use std::f32::consts::PI;
use std::ops::Range;
use std::thread;
use projection::ray_dir;

mod framebuffer;
mod ray_intersect;
mod sphere;
mod material;
mod projection;

use framebuffer::Framebuffer;
use ray_intersect::RayIntersect;
use sphere::Sphere;
use material::Material;



const BACKGROUND_COLOR: Color = Color { r: 14, g: 27, b: 61, a: 255 };
const DEF_DEPTH: f32 = 5.0;
const SPHERE_MOVE_SPEED: f32 = 1.0;
const MIN_RADIUS: f32 = 0.001;
// velocidad con la que m/n cambian el radio del conjunto
const RADIUS_SPEED: f32 = 0.4;
// el brush suelta 20 esferas por segundo
const BRUSH_RATE: f32 = 20.0;
const BRUSH_INTERVAL: f32 = 1.0 / BRUSH_RATE;
const BRUSH_RADIUS: f32 = 0.03;
// minimo que tiene que moverse el mouse (en pixeles) para soltar otra esfera
const BRUSH_MIN_STEP: f32 = 2.0;
// solo evita dividir entre cero al reescalar la profundidad
const DEPTH_EPS: f32 = 0.001;

const PALLETTE: [Color; 9] = [

    Color { r: 247, g: 243, b: 156, a: 255 }, // amarillo
    Color { r: 41, g: 171, b: 197, a: 255 }, // celeste
    Color { r: 14, g: 27, b: 61, a: 255 }, // azul oscuro
    Color { r: 1, g: 40, b: 83, a: 255 }, // azul oscuro
    Color{ r: 21, g: 28, b: 26, a: 255 }, //verde oscuro
    Color{ r: 182, g: 188, b: 121, a: 255 },// amarillo claro
    Color{ r: 183, g: 159, b: 42, a: 255 }, // amarillo intenso
    Color{ r: 81, g: 107, b: 168, a: 255 }, // azul claro
    Color{ r: 157, g: 172, b: 180, a: 255 }, // casi blanco
];

fn palette_index(color: Color) -> usize {
    PALLETTE
        .iter()
        .position(|c| c.r == color.r && c.g == color.g && c.b == color.b)
        .unwrap_or(0)
}

pub fn cast_ray(
    ray_origin: &Vector3,
    ray_direction: &Vector3,
    objects: &[Sphere],
) -> Color {
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
            let inv_len =
                1.0 / (screen_x * screen_x + screen_y * screen_y + 1.0).sqrt();
            let ray_direction =
                Vector3::new(screen_x * inv_len, screen_y * inv_len, -inv_len);

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

/// Imprime la escena como codigo Rust
fn dump_objects(objects: &[Sphere]) {
    println!("let objects: Vec<Sphere> = vec![");
    for object in objects {
        let c = object.material.diffuse;
        println!(
            "    Sphere {{ center: Vector3::new({:?}, {:?}, {:?}), radius: {:?}, material: Material {{ diffuse: Color::new({}, {}, {}, {}) }} }},",
            object.center.x,
            object.center.y,
            object.center.z,
            object.radius,
            c.r,
            c.g,
            c.b,
            c.a,
        );
    }
    println!("];");
}

/// Escena precargada (dibujo). Descomentar el `return` para usarla.
fn preset_scene() -> Vec<Sphere> {
    vec![
        
    Sphere { center: Vector3::new(1.8100519, -1.9459274, -5.0), radius: 0.6675576, material: Material { diffuse: Color::new(182, 188, 121, 255) } },
    Sphere { center: Vector3::new(1.7711501, -1.9104263, -4.9027395), radius: 0.60205203, material: Material { diffuse: Color::new(183, 159, 42, 255) } },
    Sphere { center: Vector3::new(1.7029026, -1.9012673, -4.515962), radius: 0.4403378, material: Material { diffuse: Color::new(182, 188, 121, 255) } },
    Sphere { center: Vector3::new(-2.2157428, 2.218038, -4.515962), radius: 0.43833515, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.1275501, 1.642594, -4.515962), radius: 0.28494725, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.4873564, 0.64139396, -4.515962), radius: 0.12623174, material: Material { diffuse: Color::new(157, 172, 180, 255) } },
    Sphere { center: Vector3::new(-2.2892025, 0.7874023, -4.515962), radius: 0.09001779, material: Material { diffuse: Color::new(182, 188, 121, 255) } },
    Sphere { center: Vector3::new(-2.2892025, -2.143194, -4.515962), radius: 0.17760244, material: Material { diffuse: Color::new(157, 172, 180, 255) } },
    Sphere { center: Vector3::new(-1.2045689, -2.1014774, -4.515962), radius: 0.15859544, material: Material { diffuse: Color::new(157, 172, 180, 255) } },
    Sphere { center: Vector3::new(-0.6413938, 1.1054919, -4.515962), radius: 0.19173793, material: Material { diffuse: Color::new(157, 172, 180, 255) } },
    Sphere { center: Vector3::new(-0.8082605, 0.2190125, -4.515962), radius: 0.1332529, material: Material { diffuse: Color::new(157, 172, 180, 255) } },
    Sphere { center: Vector3::new(-2.3648136, 1.4145082, -5.0), radius: 0.21478654, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.2279184, 1.0648297, -5.0), radius: 0.17907165, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.218332, 0.88853544, -5.0), radius: 0.16743127, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.8186533, -2.0611403, -5.0), radius: 0.09933115, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.8879354, -1.9398968, -5.0), radius: 0.13165598, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(1.0167547, -1.8651711, -5.0), radius: 0.011546969, material: Material { diffuse: Color::new(182, 188, 121, 255) } },
    Sphere { center: Vector3::new(-1.9687645, -1.7962108, -5.0), radius: 0.09018523, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.9687645, -1.6916361, -5.0), radius: 0.06757721, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.3498156, -1.3625467, -5.0), radius: 0.06137337, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.332495, -1.2586236, -5.0), radius: 0.062182534, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.3036275, -1.1720212, -5.0), radius: 0.06027715, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.8481747, -2.2120693, -5.0), radius: 0.04966567, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.0091789, -1.091192, -5.0), radius: 0.118462384, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.263213, -1.079645, -5.0), radius: 0.06582807, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.1939309, -1.010363, -5.0), radius: 0.08812856, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.1188755, -0.8833459, -5.0), radius: 0.07505555, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.170837, -0.7621023, -5.0), radius: 0.059441715, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.6750933, -0.9905361, -5.0), radius: 0.045092486, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.9468138, -1.2546722, -5.0), radius: 0.09309501, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.9072223, -1.3827472, -5.0), radius: 0.095568515, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.9514439, -1.5889804, -5.0), radius: 0.07767459, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.916803, -1.529978, -5.0), radius: 0.07047463, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.7724652, -0.9237604, -5.0), radius: 0.075055346, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.9572173, -0.92953396, -5.0), radius: 0.06454971, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.882162, -0.81906176, -5.0), radius: 0.0879395, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.8494375, 1.3326304, -4.28099), radius: 0.08524466, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.9161341, 1.466553, -4.28099), radius: 0.08533389, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.69275886, 1.142071, -4.18078), radius: 0.033160552, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.738615, 1.1827495, -4.18078), radius: 0.041528054, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.7679046, 1.2119368, -4.18078), radius: 0.04096307, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.8252918, 1.908375, -4.18078), radius: 0.24525651, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.6964748, 2.252993, -4.18078), radius: 0.23212446, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.92603123, 1.5446608, -4.18078), radius: 0.087029845, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.83908296, 1.6342248, -4.18078), radius: 0.07858701, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.5074015, 2.0782037, -4.18078), radius: 0.3925781, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.1763349, 2.1435077, -4.18078), radius: 0.42201737, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.0900744, 0.6526319, -4.18078), radius: 0.08221024, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.0427505, 0.5608337, -4.18078), radius: 0.04152815, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.9925184, 0.5409892, -4.18078), radius: 0.025997201, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.836045, 1.2187264, -4.18078), radius: 0.069120176, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.92206186, 1.264818, -4.18078), radius: 0.061822828, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.0041301, 1.3563075, -4.18078), radius: 0.07858671, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.0451556, 1.51022, -4.18078), radius: 0.10848545, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.91723424, 1.3420585, -4.18078), radius: 0.014482617, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-0.89309657, 1.3420585, -4.18078), radius: 0.001, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.3205506, 1.883893, -5.0), radius: 0.28242975, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.4306445, -0.36950424, -5.0), radius: 0.05446709, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.355589, -0.34063655, -5.0), radius: 0.053228837, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.3036275, -0.24826068, -5.0), radius: 0.049665526, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.263213, -0.3287668, -5.0), radius: 0.04760934, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.2527418, -0.21163908, -5.0), radius: 0.06582825, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.217025, -0.13279057, -5.0), radius: 0.06245013, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.214132, -0.06350857, -5.0), radius: 0.07505549, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.196607, 0.040414687, -5.0), radius: 0.06582783, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.3567733, -0.45033324, -5.0), radius: 0.036968514, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.3629386, -0.38859642, -5.0), radius: 0.048989832, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.3786527, -0.32331613, -5.0), radius: 0.049328897, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.4219573, -0.2577191, -5.0), radius: 0.06582826, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.4601433, -0.17320508, -5.0), radius: 0.061373334, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.3764087, -0.5241076, -5.0), radius: 0.012909984, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.3318452, -0.48006088, -4.921124), radius: 0.025819834, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.7274694, 0.04895299, -4.921124), radius: 0.30495077, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.4376535, -0.0784391, -4.921124), radius: 0.046858445, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.5614505, 0.306982, -4.921124), radius: 0.095254906, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.5172074, 0.42618176, -4.921124), radius: 0.07387169, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.4831128, 0.5455128, -4.921124), radius: 0.06146493, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.4490181, 0.66668886, -4.921124), radius: 0.07894289, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.4092412, 0.8016572, -4.921124), radius: 0.11033269, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.1187137, 0.62226474, -4.921124), radius: 0.025412612, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.212756, 1.6076514, -4.921124), radius: 0.18085799, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.59433, 1.15925, -4.921124), radius: 0.32943404, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.7330636, 1.5285722, -4.921124), radius: 0.52675205, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.8862579, 0.71061075, -4.921124), radius: 0.39142796, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.9559553, 0.17615525, -4.921124), radius: 0.22277725, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.6649503, 0.38640496, -4.921124), radius: 0.12882935, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.7673738, -0.13356218, -4.921124), radius: 0.2986383, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.8524704, -0.5047104, -4.921124), radius: 0.23186803, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.994531, -0.8637286, -4.921124), radius: 0.17056762, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.9661189, -0.561158, -4.921124), radius: 0.22414975, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.9776322, -0.16812746, -4.921124), radius: 0.22871329, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-2.1057587, 0.32389808, -4.921124), radius: 0.18889245, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.8033472, -1.7104096, -4.921124), radius: 0.08984689, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.8183757, -1.5853964, -4.921124), radius: 0.08195297, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.8199918, -1.4212663, -4.921124), radius: 0.08821488, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.8267977, -1.2655157, -4.921124), radius: 0.13253564, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.8799934, -1.1936795, -4.921124), radius: 0.091802485, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.8934454, -1.0569681, -4.921124), radius: 0.10165029, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.7561041, -1.5777335, -4.921124), radius: 0.09457453, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.6867061, -1.4361801, -4.921124), radius: 0.09457436, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.7389154, -1.3120197, -4.921124), radius: 0.09660127, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(0.12501343, -1.9433892, -4.921124), radius: 0.19395293, material: Material { diffuse: Color::new(157, 172, 180, 255) } },
    Sphere { center: Vector3::new(0.88068444, -1.4262885, -4.921124), radius: 0.17047267, material: Material { diffuse: Color::new(182, 188, 121, 255) } },
    Sphere { center: Vector3::new(-0.11716671, 0.51700765, -4.921124), radius: 0.6150944, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.7231059, -0.3271149, -4.921124), radius: 0.66445506, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.6000462, 0.78844196, -4.921124), radius: 0.4752221, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.018436708, 0.5823984, -4.921124), radius: 0.5512244, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.22085842, 0.7494156, -4.921124), radius: 0.34890267, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.46766028, 0.6819968, -5.601494), radius: 0.47799963, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.7287401, 0.81840926, -5.601494), radius: 0.22941042, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.8673661, 0.7179533, -5.601494), radius: 0.23428278, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.7105768, 0.50843185, -5.601494), radius: 0.16363001, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.5886598, 0.517692, -5.601494), radius: 0.15856636, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.5257269, 0.5357844, -5.601494), radius: 0.15974924, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.4382238, 0.57659024, -5.601494), radius: 0.17355601, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.3386813, 0.5957681, -5.601494), radius: 0.1396257, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.99388444, 0.68136835, -5.601494), radius: 0.20788515, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.1054701, 0.5541963, -5.601494), radius: 0.110147044, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.1965889, 0.54331595, -5.601494), radius: 0.118030876, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.2871414, 0.48510352, -5.601494), radius: 0.06752841, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.1383765, 0.6920811, -5.601494), radius: 0.1459259, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.2677373, 0.7179533, -5.601494), radius: 0.094176225, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.495811, 0.9984199, -5.601494), radius: 0.34152308, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.4567006, 1.1906809, -5.601494), radius: 0.17795987, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.5775144, 1.2698165, -5.601494), radius: 0.17760666, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.7205006, 1.3683593, -5.601494), radius: 0.12887521, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.531851, -0.7566296, -5.601494), radius: 0.46371874, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.42259806, -0.6098099, -5.601494), radius: 0.6318856, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.040318795, -0.856564, -5.601494), radius: 0.44816598, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-0.3033356, -0.8793892, -5.601494), radius: 0.42713606, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-0.73660564, -0.8075173, -5.601494), radius: 0.37631595, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-1.2482728, -0.7173537, -5.937371), radius: 0.3975613, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-1.6482699, -0.49151218, -5.937371), radius: 0.331147, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-2.5481398, -0.121741384, -5.937371), radius: 0.33872503, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-3.2634013, -0.30851474, -5.937371), radius: 0.4360364, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-2.9202118, -0.0039002933, -5.937371), radius: 0.23798928, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-3.3113925, 0.082270786, -5.937371), radius: 0.19318509, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-2.9939177, 0.114754684, -5.937371), radius: 0.23670192, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-2.8177688, 0.19882055, -5.937371), radius: 0.18725476, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-1.7893862, 0.047991395, -5.937371), radius: 0.15036128, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-1.6031799, -0.13026176, -5.937371), radius: 0.13521908, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-3.5151713, -0.7234107, -5.937371), radius: 0.2700905, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.78770244, -1.0986587, -5.937371), radius: 0.20349386, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.12520237, -1.8756512, -4.7634187), radius: 0.06855891, material: Material { diffuse: Color::new(183, 159, 42, 255) } },
    Sphere { center: Vector3::new(0.12311193, -1.8958334, -4.825995), radius: 0.12887663, material: Material { diffuse: Color::new(182, 188, 121, 255) } },
    Sphere { center: Vector3::new(0.854968, -1.3638947, -4.7226825), radius: 0.033435345, material: Material { diffuse: Color::new(183, 159, 42, 255) } },
    Sphere { center: Vector3::new(-0.5990447, 1.0322909, -4.176982), radius: 0.0561448, material: Material { diffuse: Color::new(183, 159, 42, 255) } },
    Sphere { center: Vector3::new(-0.75707763, 0.20210952, -4.176982), radius: 0.05035522, material: Material { diffuse: Color::new(183, 159, 42, 255) } },
    Sphere { center: Vector3::new(-1.1093274, -1.9340883, -4.176982), radius: 0.04550146, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(-1.1265227, -1.9575487, -4.2043777), radius: 0.107091874, material: Material { diffuse: Color::new(182, 188, 121, 255) } },
    Sphere { center: Vector3::new(-2.124298, -1.9903841, -4.2043777), radius: 0.04475894, material: Material { diffuse: Color::new(183, 159, 42, 255) } },
    Sphere { center: Vector3::new(-2.3136282, 0.6068496, -4.2043777), radius: 0.035008483, material: Material { diffuse: Color::new(183, 159, 42, 255) } },
    Sphere { center: Vector3::new(-2.1361105, 0.7356064, -4.2043777), radius: 0.024273908, material: Material { diffuse: Color::new(183, 159, 42, 255) } },
    Sphere { center: Vector3::new(-1.0914623, -1.8863083, -4.060204), radius: 0.02614389, material: Material { diffuse: Color::new(183, 159, 42, 255) } },
    Sphere { center: Vector3::new(1.1345733, 2.0143697, -4.060204), radius: 0.6500554, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.5632037, 1.7924728, -4.060204), radius: 0.55429333, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(2.0576217, 1.5638051, -4.060204), radius: 0.5774822, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(2.345043, 1.0495957, -4.060204), radius: 0.20225109, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(2.114432, 2.1706922, -4.060204), radius: 0.28043836, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.6546578, 2.0578372, -4.060204), radius: 0.48189452, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(0.457025, 2.5332708, -4.8571773), radius: 0.47665474, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-0.43356517, 4.0581656, -6.728544), radius: 1.0720762, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(-2.526792, -0.29143515, -5.609857), radius: 0.09735103, material: Material { diffuse: Color::new(21, 28, 26, 255) } },
    Sphere { center: Vector3::new(0.9021208, 1.5854923, -10.688196), radius: 0.77421457, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.2374609, 1.711669, -10.688196), radius: 0.52997434, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.6057894, 1.6167579, -10.688196), radius: 0.37596536, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(1.9623246, 1.5797335, -10.688196), radius: 0.21553819, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(2.055606, 1.5034363, -10.688196), radius: 0.19513892, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(2.6770272, 0.9470688, -10.688196), radius: 0.3806577, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(2.922123, 0.7396474, -10.688196), radius: 0.26180685, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(3.3075664, 0.77056533, -10.688196), radius: 0.30481666, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(3.723731, 0.7452275, -10.688196), radius: 0.24497505, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    Sphere { center: Vector3::new(4.0727496, 0.85157496, -10.688196), radius: 0.19278325, material: Material { diffuse: Color::new(81, 107, 168, 255) } },
    ]
}

/// Estado del brush. Se prende y apaga con C; mientras este prendido, mantener
/// el click izquierdo va dejando un rastro de esferas a 20 por segundo.
struct Brush {
    active: bool,
    radius: f32,
    color: Color,
    depth: f32,
    timer: f32,
    /// indice donde arranco el trazo que se esta pintando
    stroke_start: Option<usize>,
    /// ultima posicion de mouse donde se pinto, en pixeles
    last_point: Option<Vector2>,
}

impl Brush {
    fn new() -> Self {
        Brush {
            active: false,
            radius: BRUSH_RADIUS,
            color: PALLETTE[0],
            depth: DEF_DEPTH,
            timer: 0.0,
            stroke_start: None,
            last_point: None,
        }
    }

    /// Arranca un trazo nuevo: todo lo que se pinte se agrupa desde aca.
    fn begin_stroke(&mut self, objects_len: usize, depth: f32, color: Color) {
        self.stroke_start = Some(objects_len);
        self.depth = depth;
        self.color = color;
        self.timer = BRUSH_INTERVAL; // la primera esfera sale de una
        self.last_point = None;
    }

    /// Se llama cada frame mientras el click siga presionado.
    /// Devuelve true si agrego una esfera (para volver a renderizar).
    fn paint(
        &mut self,
        objects: &mut Vec<Sphere>,
        mouse: Vector2,
        width: f32,
        height: f32,
        dt: f32,
    ) -> bool {
        if self.stroke_start.is_none() {
            return false;
        }

        self.timer += dt;

        // si el mouse no se movio no apilamos esferas encima de la anterior
        if let Some(prev) = self.last_point {
            if (mouse - prev).length() < BRUSH_MIN_STEP {
                return false;
            }
        }

        if self.timer < BRUSH_INTERVAL {
            return false;
        }
        self.timer = 0.0;

        let center = projection::screen_to_world(mouse.x, mouse.y, width, height, self.depth);
        objects.push(Sphere {
            center,
            radius: self.radius,
            material: Material { diffuse: self.color },
        });
        self.last_point = Some(mouse);
        true
    }

    /// Cierra el trazo y devuelve el rango de esferas que quedo pintado.
    fn end_stroke(&mut self, objects_len: usize) -> Option<Range<usize>> {
        let start = self.stroke_start.take()?;
        self.last_point = None;
        if start < objects_len {
            Some(start..objects_len)
        } else {
            None
        }
    }
}

/// Mueve en bloque todo el conjunto seleccionado.
/// El grupo se mueve rigido: si a alguna esfera le tocaria cruzar la camara,
/// no se mueve ninguna, asi el trazo nunca se deforma.
fn move_selection(
    objects: &mut [Sphere],
    range: &Range<usize>,
    delta_x: f32,
    delta_y: f32,
    delta_depth: f32,
) -> bool {
    if delta_x == 0.0 && delta_y == 0.0 && delta_depth == 0.0 {
        return false;
    }

    let group = &mut objects[range.clone()];
    if group.iter().any(|s| -s.center.z + delta_depth < DEPTH_EPS) {
        return false;
    }

    for sphere in group {
        // center.z guarda -depth, entonces sumar profundidad es restar en z
        sphere.center = Vector3::new(
            sphere.center.x + delta_x,
            sphere.center.y + delta_y,
            sphere.center.z - delta_depth,
        );
    }
    true
}

/// Pasa todo el conjunto al siguiente (o anterior) color de la paleta,
/// tomando como referencia el color de la primera esfera del grupo.
fn shift_selection_color(objects: &mut [Sphere], range: &Range<usize>, step: i32) -> bool {
    if step == 0 || range.is_empty() {
        return false;
    }

    let len = PALLETTE.len() as i32;
    let current = palette_index(objects[range.start].material.diffuse) as i32;
    let next = (current + step).rem_euclid(len) as usize;

    for sphere in &mut objects[range.clone()] {
        sphere.material.diffuse = PALLETTE[next];
    }
    true
}

/// Crece o encoge el radio de todo el conjunto (m/n).
fn resize_selection(objects: &mut [Sphere], range: &Range<usize>, delta: f32) -> bool {
    if delta == 0.0 {
        return false;
    }

    let mut changed = false;
    for sphere in &mut objects[range.clone()] {
        let radius = (sphere.radius + delta).max(MIN_RADIUS);
        if radius != sphere.radius {
            sphere.radius = radius;
            changed = true;
        }
    }
    changed
}

fn main() {
    let window_width = 1000;
    let window_height = 1000;

    let (mut window, raylib_thread) = raylib::init()
        .size(window_width, window_height)
        .title("Raytracer Example")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    window.set_target_fps(60);

    let mut framebuffer = Framebuffer::new(window_width as u32, window_height as u32);
    framebuffer.set_background_color(Color::new(80, 80, 200, 255));

    let width = window_width as f32;
    let height = window_height as f32;

    // dibujo precargado (usar Vec::new() para empezar en blanco)
    let mut objects: Vec<Sphere> = preset_scene();
    let mut dragging: Option<usize> = None;
    // la seleccion es un rango: una sola esfera (i..i+1) o el ultimo trazo del brush
    let mut selected: Option<Range<usize>> = None;
    let mut brush = Brush::new();
    let mut dirty = true;

    println!("C = brush | click = pintar/crear | QE profundidad | WASD mover | JK color | MN radio");

    while !window.window_should_close() {
        let mouse = window.get_mouse_position();
        let dt = window.get_frame_time();

        // por si la seleccion quedo apuntando fuera del vector
        if selected.as_ref().is_some_and(|r| r.end > objects.len()) {
            selected = None;
        }

        // C prende/apaga el brush
        if window.is_key_pressed(KeyboardKey::KEY_C) {
            brush.active = !brush.active;
            dragging = None; // no dejar un drag a medias al cambiar de modo
            if let Some(range) = brush.end_stroke(objects.len()) {
                selected = Some(range);
            }
            println!("brush: {}", if brush.active { "ON" } else { "OFF" });
        }

        if brush.active {
            // el rastro del mouse pinta esferas mientras el click siga apretado
            if window.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
                let depth = match &selected {
                    Some(range) => -objects[range.start].center.z,
                    None => DEF_DEPTH,
                };
                let color = PALLETTE[objects.len() % PALLETTE.len()];
                brush.begin_stroke(objects.len(), depth, color);
            }

            if window.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT)
                && brush.paint(&mut objects, mouse, width, height, dt)
            {
                dirty = true;
            }

            if window.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT) {
                // al soltar, todo el trazo queda seleccionado como un conjunto
                if let Some(range) = brush.end_stroke(objects.len()) {
                    selected = Some(range);
                }
            }
        } else {
            //crear esferas con click izquierdo
            if window.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
                let depth = match &selected {
                    Some(range) => -objects[range.start].center.z,
                    None => DEF_DEPTH,
                };

                let center = projection::screen_to_world(mouse.x, mouse.y, width, height, depth);
                let diffuse = PALLETTE[objects.len() % PALLETTE.len()];

                objects.push(Sphere {
                    center,
                    radius: MIN_RADIUS,
                    material: Material { diffuse },
                });

                let index = objects.len() - 1;
                dragging = Some(index);
                selected = Some(index..index + 1);
                dirty = true;
            }

            if let Some(index) = dragging {
                let depth = -objects[index].center.z;
                let edge = projection::screen_to_world(mouse.x, mouse.y, width, height, depth);
                let radius = (edge - objects[index].center).length().max(MIN_RADIUS);

                if (radius - objects[index].radius).abs() > 0.01 {
                    objects[index].radius = radius;
                    dirty = true;
                }
            }

            if window.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT) {
                if let Some(i) = dragging.take() {
                    if objects[i].radius < MIN_RADIUS {
                        objects.pop();
                        selected = None;
                        dirty = true;
                    }
                }
            }
        }

        // controles del conjunto seleccionado: una esfera o el ultimo trazo
        if let Some(range) = selected.clone() {
            let mut delta_depth = 0.0;
            let mut delta_x = 0.0;
            let mut delta_y = 0.0;

            //depth q/e
            if window.is_key_down(KeyboardKey::KEY_Q) {
                delta_depth += SPHERE_MOVE_SPEED * dt; // más profundo
            }
            if window.is_key_down(KeyboardKey::KEY_E) {
                delta_depth -= SPHERE_MOVE_SPEED * dt; // más cerca
            }

            // mover x con a/d
            if window.is_key_down(KeyboardKey::KEY_A) {
                delta_x -= SPHERE_MOVE_SPEED * dt; // izquierda
            }
            if window.is_key_down(KeyboardKey::KEY_D) {
                delta_x += SPHERE_MOVE_SPEED * dt; // derecha
            }

            //mover y con w/s
            if window.is_key_down(KeyboardKey::KEY_W) {
                delta_y -= SPHERE_MOVE_SPEED * dt; // arriba
            }
            if window.is_key_down(KeyboardKey::KEY_S) {
                delta_y += SPHERE_MOVE_SPEED * dt; // abajo
            }

            if move_selection(&mut objects, &range, delta_x, delta_y, delta_depth) {
                dirty = true;
            }

            // cambiar color con j/k siguiendo el orden del array
            let mut step = 0i32;
            if window.is_key_pressed(KeyboardKey::KEY_J) {
                step -= 1;
            }
            if window.is_key_pressed(KeyboardKey::KEY_K) {
                step += 1;
            }

            if shift_selection_color(&mut objects, &range, step) {
                dirty = true;
            }

            // radio del conjunto con m/n
            let mut delta_radius = 0.0;
            if window.is_key_down(KeyboardKey::KEY_M) {
                delta_radius += RADIUS_SPEED * dt;
            }
            if window.is_key_down(KeyboardKey::KEY_N) {
                delta_radius -= RADIUS_SPEED * dt;
            }

            if resize_selection(&mut objects, &range, delta_radius) {
                // el brush hereda el grosor para que el siguiente trazo siga igual
                if brush.active {
                    brush.radius = (brush.radius + delta_radius).max(MIN_RADIUS);
                }
                dirty = true;
            }
        }

        if dirty {
            // render escribe TODOS los pixeles, no hace falta limpiar antes
            render(&mut framebuffer, &objects);
            dirty = false;
        }

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }

    dump_objects(&objects);
}
