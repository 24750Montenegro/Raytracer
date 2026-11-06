use raylib::prelude::*;
use std::time::Instant;

mod fragment;
mod framebuffer;
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
use matrix::{create_model_matrix, multiply, new_matrix4, rotation_x, rotation_y};
use obj::Obj;
use primitive::primitive_assembly;
use rasterizer::rasterize;
use shaders::{fragment_shader_with, vertex_shader_with_matrix, Accessory, Frag, SHADERS};
use vertex::Vertex;

const MODEL_PATH: &str = "assets/models/sphere.obj";
const CAPTURES_DIR: &str = "assets/capturas";

const WIDTH: i32 = 800;
const HEIGHT: i32 = 600;

/// radio de la esfera en pantalla / lado corto de la ventana
const FIT: f32 = 0.42;

const ORBIT_SPEED: f32 = 1.5;
const ZOOM_SPEED: f32 = 1.8;
const SUN_SPEED: f32 = 1.0;

/// tan lejos que en toda la esfera llega casi en la misma direccion
const SUN_DISTANCE: f32 = 1000.0;
const SUN_HEIGHT: f32 = 0.35;
const INITIAL_SUN_ANGLE: f32 = -0.8;

const SHADER_KEYS: [KeyboardKey; 9] = [
    KeyboardKey::KEY_ONE,
    KeyboardKey::KEY_TWO,
    KeyboardKey::KEY_THREE,
    KeyboardKey::KEY_FOUR,
    KeyboardKey::KEY_FIVE,
    KeyboardKey::KEY_SIX,
    KeyboardKey::KEY_SEVEN,
    KeyboardKey::KEY_EIGHT,
    KeyboardKey::KEY_NINE,
];

pub struct Uniforms {
    pub model_matrix: Matrix,
    pub view_matrix: Matrix,
    pub viewport_matrix: Matrix,
    pub to_viewer: Vector3,
    pub sun: Light,
    pub ambient: Rgb,
    pub time: f32,
    pub shader: fn(&Frag) -> Vector3,
}

/// Camara ortografica que orbita alrededor de la esfera.
#[derive(Clone, Copy, PartialEq)]
struct Camera {
    yaw: f32,
    pitch: f32,
    /// pixeles por unidad del mundo (la esfera mide 1 de radio)
    zoom: f32,
}

impl Camera {
    fn view_matrix(&self) -> Matrix {
        multiply(&rotation_x(self.pitch), &rotation_y(-self.yaw))
    }

    fn viewport_matrix(&self) -> Matrix {
        let s = self.zoom;
        new_matrix4([
            [s, 0.0, 0.0, WIDTH as f32 / 2.0],
            [0.0, -s, 0.0, HEIGHT as f32 / 2.0],
            [0.0, 0.0, -s, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    fn to_viewer(&self) -> Vector3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        Vector3::new(sin_yaw * cos_pitch, sin_pitch, cos_yaw * cos_pitch)
    }
}

fn sun(angle: f32) -> Light {
    let direction = Vector3::new(angle.sin(), SUN_HEIGHT, angle.cos()).normalized();
    Light::new(direction * SUN_DISTANCE, Rgb::new(1.0, 0.97, 0.92), 1.15)
}

/// Lleva el modelo a una esfera de radio 1 centrada en el origen, sin tocar
/// sus vertices.
fn fit_unit_sphere(vertices: &[Vertex]) -> Matrix {
    let mut min = Vector3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vector3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for vertex in vertices {
        min = min.min(vertex.position);
        max = max.max(vertex.position);
    }

    let center = (min + max) * 0.5;
    let radius = vertices
        .iter()
        .map(|vertex| (vertex.position - center).length())
        .fold(f32::EPSILON, f32::max);

    create_model_matrix(center * (-1.0 / radius), 1.0 / radius, Vector3::zero())
}

fn render(
    framebuffer: &mut Framebuffer,
    uniforms: &Uniforms,
    vertex_array: &[Vertex],
    accessories: &[Accessory],
) {
    let mut objects = vec![(&vertex_array[..], &uniforms.model_matrix, uniforms.shader)];
    objects.extend(
        accessories
            .iter()
            .map(|object| (object.vertices, &object.model_matrix, object.shade)),
    );

    let mut nearest = std::iter::repeat_with(|| None)
        .take(framebuffer.width as usize * framebuffer.height as usize)
        .collect::<Vec<_>>();
    for (object_index, (vertices, model_matrix, _)) in objects.iter().enumerate() {
        let transformed: Vec<Vertex> = vertices
            .iter()
            .map(|vertex| vertex_shader_with_matrix(vertex, model_matrix, uniforms))
            .collect();
        let triangles = primitive_assembly(&transformed);
        let fragments = rasterize(
            &triangles,
            framebuffer.width as i32,
            framebuffer.height as i32,
        );

        for fragment in fragments {
            let (x, y) = (fragment.position.x as i32, fragment.position.y as i32);
            if let Some(index) = framebuffer.depth_test(x, y, fragment.depth) {
                nearest[index] = Some((fragment, object_index));
            }
        }
    }

    framebuffer.shade(|index| {
        nearest[index].as_ref().map(|(fragment, object_index)| {
            let (_, model_matrix, shade) = objects[*object_index];
            Rgb::from_vector(fragment_shader_with(
                fragment,
                uniforms,
                model_matrix,
                shade,
            ))
            .to_display()
        })
    });
}

fn capture_path(shader: usize) -> String {
    format!("{CAPTURES_DIR}/{}.png", SHADERS[shader].name.replace(' ', "-"))
}

/// Dibuja cada shader en el instante `time` y lo guarda en assets/capturas.
fn save_all_captures(framebuffer: &mut Framebuffer, uniforms: &mut Uniforms, vertex_array: &[Vertex], time: f32) {
    uniforms.time = time;
    for (index, shader) in SHADERS.iter().enumerate() {
        uniforms.shader = shader.shade;
        let start = Instant::now();
        framebuffer.clear();
        let accessories = (shader.accessories)(time);
        render(framebuffer, uniforms, vertex_array, &accessories);
        let elapsed = start.elapsed().as_secs_f32() * 1000.0;

        let path = capture_path(index);
        match framebuffer.save(&path) {
            Ok(()) => println!("{:<18} {elapsed:>6.1} ms   {path}", shader.name),
            Err(problem) => eprintln!("{problem}"),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let obj = match Obj::load(MODEL_PATH) {
        Ok(obj) => obj,
        Err(problem) => {
            eprintln!("no se pudo cargar el modelo: {problem}");
            return;
        }
    };
    let vertex_array = obj.get_vertex_array();
    println!("triangulos: {}", vertex_array.len() / 3);

    let initial_camera = Camera {
        yaw: 0.0,
        pitch: 0.25,
        zoom: FIT * WIDTH.min(HEIGHT) as f32,
    };

    let mut uniforms = Uniforms {
        model_matrix: fit_unit_sphere(&vertex_array),
        view_matrix: initial_camera.view_matrix(),
        viewport_matrix: initial_camera.viewport_matrix(),
        to_viewer: initial_camera.to_viewer(),
        sun: sun(INITIAL_SUN_ANGLE),
        ambient: Rgb::new(0.03, 0.035, 0.05),
        time: 0.0,
        shader: SHADERS[0].shade,
    };

    let mut framebuffer = Framebuffer::new(WIDTH as u32, HEIGHT as u32);
    framebuffer.set_background_color(Vector3::new(0.0, 0.0, 0.0));

    if let Some(position) = args.iter().position(|arg| arg == "--capturas") {
        let time = args.get(position + 1).and_then(|text| text.parse().ok()).unwrap_or(0.0);
        save_all_captures(&mut framebuffer, &mut uniforms, &vertex_array, time);
        return;
    }

    let (mut window, raylib_thread) = raylib::init()
        .size(WIDTH, HEIGHT)
        .title("Laboratorio de shaders")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    window.set_target_fps(60);
    framebuffer.init_texture(&mut window, &raylib_thread);

    let mut camera = initial_camera;
    let mut sun_angle = INITIAL_SUN_ANGLE;
    let mut shader = 0;
    let mut time = 0.0;
    let mut paused = false;
    let mut drawn = None;

    println!("Tab / Shift+Tab / 1-{} = cambiar shader", SHADERS.len().min(9));
    println!("flechas = orbitar la camara | Z/X = zoom | Q/E = mover el sol");
    println!("Espacio = pausar el tiempo | R = volver al inicio | P = guardar captura");

    while !window.window_should_close() {
        let dt = window.get_frame_time();
        let held = |key| window.is_key_down(key);
        let shift = held(KeyboardKey::KEY_LEFT_SHIFT) || held(KeyboardKey::KEY_RIGHT_SHIFT);

        if window.is_key_pressed(KeyboardKey::KEY_TAB) {
            let step = if shift { SHADERS.len() - 1 } else { 1 };
            shader = (shader + step) % SHADERS.len();
        }
        for (index, key) in SHADER_KEYS.iter().enumerate().take(SHADERS.len()) {
            if window.is_key_pressed(*key) {
                shader = index;
            }
        }
        if window.is_key_pressed(KeyboardKey::KEY_SPACE) {
            paused = !paused;
        }
        if window.is_key_pressed(KeyboardKey::KEY_R) {
            camera = initial_camera;
            sun_angle = INITIAL_SUN_ANGLE;
            time = 0.0;
        }
        if !paused {
            time += dt;
        }

        let turn = ORBIT_SPEED * dt;
        if held(KeyboardKey::KEY_LEFT) { camera.yaw -= turn; }
        if held(KeyboardKey::KEY_RIGHT) { camera.yaw += turn; }
        if held(KeyboardKey::KEY_UP) { camera.pitch += turn; }
        if held(KeyboardKey::KEY_DOWN) { camera.pitch -= turn; }
        camera.pitch = camera.pitch.clamp(-1.5, 1.5);

        if held(KeyboardKey::KEY_Q) { sun_angle -= SUN_SPEED * dt; }
        if held(KeyboardKey::KEY_E) { sun_angle += SUN_SPEED * dt; }

        if held(KeyboardKey::KEY_Z) { camera.zoom /= ZOOM_SPEED.powf(dt); }
        if held(KeyboardKey::KEY_X) { camera.zoom *= ZOOM_SPEED.powf(dt); }

        let state = (camera, sun_angle, shader, time);
        if drawn != Some(state) {
            uniforms.view_matrix = camera.view_matrix();
            uniforms.viewport_matrix = camera.viewport_matrix();
            uniforms.to_viewer = camera.to_viewer();
            uniforms.sun = sun(sun_angle);
            uniforms.time = time;
            uniforms.shader = SHADERS[shader].shade;

            let start = Instant::now();
            framebuffer.clear();
            let accessories = (SHADERS[shader].accessories)(time);
            render(&mut framebuffer, &uniforms, &vertex_array, &accessories);
            let elapsed = start.elapsed().as_secs_f32() * 1000.0;

            window.set_window_title(
                &raylib_thread,
                &format!(
                    "Shaders | {}/{} {} ({}) | {elapsed:.0} ms | t = {time:.1} s{}",
                    shader + 1,
                    SHADERS.len(),
                    SHADERS[shader].body,
                    SHADERS[shader].name,
                    if paused { " (pausa)" } else { "" },
                ),
            );
            drawn = Some(state);
        }

        if window.is_key_pressed(KeyboardKey::KEY_P) {
            let path = capture_path(shader);
            match framebuffer.save(&path) {
                Ok(()) => println!("captura guardada en {path}"),
                Err(problem) => eprintln!("{problem}"),
            }
        }

        let current = &SHADERS[shader];
        framebuffer.swap_buffers(&mut window, &raylib_thread, |overlay| {
            overlay.draw_text(current.body, 20, 18, 32, Color::WHITE);
            overlay.draw_text(current.name, 22, 56, 18, Color::GRAY);
        });
    }
}
