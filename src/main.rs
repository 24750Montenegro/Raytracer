
use raylib::prelude::*;
use std::f32::consts::PI;
use std::time::{Duration, Instant};

mod benchmark;
mod fragment;
mod framebuffer;
mod line;
mod matrix;
mod obj;
mod primitive;
mod rasterizer;
mod shaders;
mod triangle;
mod vertex;

// Luces y sombras que vienen del raytracer.
mod color;
mod light;
mod material;
mod shading;

use color::Rgb;
use framebuffer::Framebuffer;
use light::Light;
use material::Material;
use matrix::create_model_matrix;
use obj::Obj;
use primitive::primitive_assembly;
use rasterizer::{rasterize, TriangleMode};
use shaders::{fragment_shader, vertex_shader};
use vertex::Vertex;

const MODEL_PATH: &str = "assets/models/nave-juanmontenegro-24750.obj";

/// tamaño inicial del modelo (radio / lado corto de la ventana)
const FIT: f32 = 0.42;

/// velocidades de los controles, por segundo
const MOVE_SPEED: f32 = 300.0;
const ROTATE_SPEED: f32 = 1.5;
const SCALE_SPEED: f32 = 1.8;

pub struct Uniforms {
    pub model_matrix: Matrix,
    /// si el vertex shader calcula el color con las luces. En el contorno
    /// no: las lineas quedan del color del vertice
    pub lighting: bool,
    pub lights: Vec<Light>,
    pub ambient: Rgb,
    pub material: Material,
}

#[derive(Clone, Copy, PartialEq)]
struct Transform {
    translation: Vector3,
    scale: f32,
    /// angulos en radianes alrededor de X, Y y Z
    rotation: Vector3,
}

impl Transform {
    fn matrix(&self) -> Matrix {
        create_model_matrix(self.translation, self.scale, self.rotation)
    }
}

/// cuanto tardo cada etapa de un dibujo y cuantos fragmentos genero
pub struct FrameStats {
    /// vertex shader, ensamblaje, rasterizacion y procesamiento de fragmentos
    pub stages: [Duration; 4],
    pub fragments: usize,
}

fn render(framebuffer: &mut Framebuffer, uniforms: &Uniforms, vertex_array: &[Vertex], mode: TriangleMode) -> FrameStats {
    let start = Instant::now();

    // 1. Procesamiento de vertices
    let transformed_vertices: Vec<Vertex> = vertex_array
        .iter()
        .map(|vertex| vertex_shader(vertex, uniforms))
        .collect();
    let vertices_done = Instant::now();

    // 2. Ensamblaje de primitivas
    let triangles = primitive_assembly(&transformed_vertices);
    let assembly_done = Instant::now();

    // 3. Rasterizacion
    let fragments = rasterize(&triangles, mode);
    let fragment_count = fragments.len();
    let raster_done = Instant::now();

    // 4. Procesamiento de fragmentos
    for fragment in fragments {
        let x = fragment.position.x as i32;
        let y = fragment.position.y as i32;

        // relleno: solo pasa lo que queda delante de lo ya dibujado. En el
        // contorno se ven todas las aristas, tambien las de atras
        if mode.is_filled() && !framebuffer.depth_test(x, y, fragment.depth) {
            continue;
        }

        let color = fragment_shader(&fragment);
        framebuffer.point(x, y, color);
    }

    FrameStats {
        stages: [
            vertices_done - start,
            assembly_done - vertices_done,
            raster_done - assembly_done,
            raster_done.elapsed(),
        ],
        fragments: fragment_count,
    }
}

/// Mueve el modelo para que su centro quede en el origen y devuelve su
/// radio. Asi rota sobre si mismo y se puede escalar a la ventana.
fn center_model(vertices: &mut [Vertex]) -> f32 {
    let mut min = Vector3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vector3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

    for vertex in vertices.iter() {
        let p = vertex.position;
        min = Vector3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
        max = Vector3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
    }

    let center = (min + max) * 0.5;
    let mut radius: f32 = 0.0;
    for vertex in vertices.iter_mut() {
        vertex.position = vertex.position - center;
        radius = radius.max(vertex.position.length());
    }

    radius.max(f32::EPSILON)
}

/// Las tres luces del raytracer: una principal arriba a la izquierda, una de
/// relleno a la derecha y una por detras. Estan en coordenadas de pantalla
/// (y hacia abajo, z hacia adentro), alrededor del centro de la ventana.
fn default_lights(center: Vector3, radius: f32) -> Vec<Light> {
    vec![
        Light::new(
            center + Vector3::new(-1.2 * radius, -2.2 * radius, -1.4 * radius),
            Rgb::new(1.0, 0.96, 0.88),
            0.9,
        ),
        Light::new(
            center + Vector3::new(1.8 * radius, -0.6 * radius, -1.1 * radius),
            Rgb::new(0.68, 0.80, 1.0),
            0.3,
        ),
        Light::new(
            center + Vector3::new(0.7 * radius, 2.0 * radius, 0.9 * radius),
            Rgb::new(0.74, 0.84, 1.0),
            0.34,
        ),
    ]
}

fn main() {
    let window_width = 800;
    let window_height = 600;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let has_flag = |flag: &str| args.iter().any(|arg| arg == flag);

    let obj = match Obj::load(MODEL_PATH) {
        Ok(obj) => obj,
        Err(problem) => {
            eprintln!("no se pudo cargar el modelo: {problem}");
            return;
        }
    };
    let mut vertex_array = obj.get_vertex_array();
    let radius = center_model(&mut vertex_array);
    println!("triangulos: {}", vertex_array.len() / 3);

    let screen_center = Vector3::new(window_width as f32 / 2.0, window_height as f32 / 2.0, 0.0);
    let fit_radius = FIT * window_width.min(window_height) as f32;

    // en Blender y sube y en la pantalla baja: casi -90 grados en X la deja
    // vista desde arriba, 180 en Z le pone la punta hacia arriba y el giro
    // en Y la muestra de tres cuartos
    let initial = Transform {
        translation: screen_center,
        scale: fit_radius / radius,
        rotation: Vector3::new(-1.1, 0.5, PI),
    };

    let mut uniforms = Uniforms {
        model_matrix: initial.matrix(),
        lighting: false,
        lights: default_lights(screen_center, fit_radius),
        ambient: Rgb::new(0.24, 0.27, 0.33),
        material: Material::polished(Color::new(196, 202, 214, 255)),
    };

    // la nave como empieza y acercada 6 veces, para comparar y medir
    let views = [
        ("1x", initial.matrix()),
        ("6x", Transform { scale: initial.scale * 6.0, ..initial }.matrix()),
    ];

    if has_flag("--benchmark-triangles") {
        benchmark::run(&vertex_array, &uniforms);
        return;
    }
    if has_flag("--compare-triangles") {
        benchmark::compare_algorithms(&vertex_array, &mut uniforms, &views);
        return;
    }
    if has_flag("--profile-stages") {
        let mut framebuffer = Framebuffer::new(window_width as u32, window_height as u32);
        benchmark::profile_stages(&mut framebuffer, &vertex_array, &mut uniforms, &views);
        return;
    }

    let (mut window, raylib_thread) = raylib::init()
        .size(window_width, window_height)
        .title("Rasterizador")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    window.set_target_fps(60);

    let mut framebuffer = Framebuffer::new(window_width as u32, window_height as u32);
    framebuffer.set_background_color(Vector3::new(0.0, 0.0, 0.0));
    framebuffer.init_texture(&mut window, &raylib_thread);

    let mut mode = TriangleMode::Wireframe;
    // el algoritmo de relleno que se usa al pasar a relleno con Tab
    let mut fill = TriangleMode::FilledBarycentric;
    let mut transform = initial;
    // lo que tiene dibujado el framebuffer ahora mismo
    let mut drawn: Option<(Transform, TriangleMode)> = None;

    println!("Tab = contorno / relleno | F = algoritmo de relleno");
    println!("flechas = mover | W/S, A/D, Q/E = rotar en X, Y, Z | Z/X = escala");
    println!("R = volver al inicio | P = guardar captura");

    while !window.window_should_close() {
        let dt = window.get_frame_time();
        let held = |key| window.is_key_down(key);

        if window.is_key_pressed(KeyboardKey::KEY_TAB) {
            mode = if mode.is_filled() { TriangleMode::Wireframe } else { fill };
        }
        if window.is_key_pressed(KeyboardKey::KEY_F) {
            fill = if fill == TriangleMode::FilledBarycentric {
                TriangleMode::FilledScanline
            } else {
                TriangleMode::FilledBarycentric
            };
            if mode.is_filled() {
                mode = fill;
            }
        }
        if window.is_key_pressed(KeyboardKey::KEY_R) {
            transform = initial;
        }

        // traslacion
        let step = MOVE_SPEED * dt;
        if held(KeyboardKey::KEY_LEFT) { transform.translation.x -= step; }
        if held(KeyboardKey::KEY_RIGHT) { transform.translation.x += step; }
        if held(KeyboardKey::KEY_UP) { transform.translation.y -= step; }
        if held(KeyboardKey::KEY_DOWN) { transform.translation.y += step; }

        // rotacion
        let turn = ROTATE_SPEED * dt;
        if held(KeyboardKey::KEY_W) { transform.rotation.x -= turn; }
        if held(KeyboardKey::KEY_S) { transform.rotation.x += turn; }
        if held(KeyboardKey::KEY_A) { transform.rotation.y -= turn; }
        if held(KeyboardKey::KEY_D) { transform.rotation.y += turn; }
        if held(KeyboardKey::KEY_Q) { transform.rotation.z -= turn; }
        if held(KeyboardKey::KEY_E) { transform.rotation.z += turn; }

        // escala: multiplicar en vez de sumar hace que el zoom se sienta
        // igual de rapido de cerca que de lejos
        if held(KeyboardKey::KEY_Z) { transform.scale /= SCALE_SPEED.powf(dt); }
        if held(KeyboardKey::KEY_X) { transform.scale *= SCALE_SPEED.powf(dt); }

        // con cientos de miles de triangulos un cuadro tarda mas de 100 ms:
        // se vuelve a dibujar solo si algo cambio
        if drawn != Some((transform, mode)) {
            // los uniforms de este dibujo
            uniforms.model_matrix = transform.matrix();
            uniforms.lighting = mode.is_filled();

            let start = Instant::now();
            framebuffer.clear();
            render(&mut framebuffer, &uniforms, &vertex_array, mode);
            let elapsed = start.elapsed().as_secs_f32() * 1000.0;

            window.set_window_title(
                &raylib_thread,
                &format!("Rasterizador | {} | {elapsed:.0} ms", mode.name()),
            );
            drawn = Some((transform, mode));
        }

        if window.is_key_pressed(KeyboardKey::KEY_P) {
            let path = format!("captura-{}.png", mode.name().replace(' ', "-"));
            match framebuffer.save(&path) {
                Ok(()) => println!("captura guardada en {path}"),
                Err(problem) => eprintln!("{problem}"),
            }
        }

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}
