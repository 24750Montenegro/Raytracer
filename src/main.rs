use raylib::prelude::*;
use std::cell::RefCell;
use std::time::Instant;

mod flight;
mod fragment;
mod framebuffer;
mod matrix;
mod obj;
mod pool;
mod primitive;
mod rasterizer;
mod solar_system;
mod selector;
mod shaders;
mod ships;
mod triangle;
mod vertex;

// Luces y sombras que vienen del raytracer.
mod color;
mod light;
mod material;
mod shading;

use color::Rgb;
use fragment::Fragment;
use flight::{Controls, Flight};
use framebuffer::Framebuffer;
use light::Light;
use matrix::{create_model_matrix, multiply, new_matrix4, rotation_x, rotation_y};
use obj::Obj;
use primitive::primitive_assembly;
use rasterizer::rasterize;
use selector::Selector;
use shaders::{fragment_shader_with, transform, vertex_shader_with_matrix, Accessory, Frag, Normals, SHADERS};
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

const SHIP_MARKER: Color = Color::new(255, 180, 80, 255);

/// En el sombreado grueso, cuanto pueden diferir las muestras alrededor de
/// un pixel (sumando los tres canales) antes de que se sombree entero.
const COARSE_CONTRAST: f32 = 0.3;

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

thread_local! {
    /// El fragmento mas cercano de cada pixel y de que objeto es. Se reusa
    /// cuadro a cuadro: pedir memoria nueva para toda la pantalla en cada
    /// cuadro costaba unos 4 ms.
    static NEAREST: RefCell<Vec<Option<(Fragment, usize)>>> = const { RefCell::new(Vec::new()) };
}

/// Referencia temporal a cualquier malla de la escena. A diferencia de
/// `Accessory` (cuyas mallas son estáticas), permite dibujar la esfera cargada
/// desde OBJ junto con los planetas sin copiar su geometría cada cuadro.
struct SceneObject<'a> {
    vertices: &'a [Vertex],
    model_matrix: Matrix,
    shade: fn(&Frag) -> Vector3,
    normals: Normals,
    /// Va entre la vista y el viewport, en el espacio de la camara. En vuelo
    /// agranda el objeto segun lo cerca que este; en los demas modos es la
    /// identidad.
    projection: Matrix,
    /// Desde donde se mira este objeto, si no es `Uniforms::to_viewer`. En
    /// vuelo cada objeto se ve por la linea que lo une con la camara.
    to_viewer: Option<Vector3>,
    /// Si es una esfera cerrada: su mitad de atras no se ve nunca y se
    /// descarta antes de rasterizar.
    cull_back_faces: bool,
    /// Sombrear un pixel de cada 2x2 e interpolar el resto. Para objetos que
    /// ocupan mucha pantalla, donde el detalle del shader ya mide varios
    /// pixeles y la interpolacion casi no se nota.
    coarse_shading: bool,
}

impl<'a> SceneObject<'a> {
    fn new(vertices: &'a [Vertex], model_matrix: Matrix, shade: fn(&Frag) -> Vector3) -> Self {
        SceneObject {
            vertices,
            model_matrix,
            shade,
            normals: Normals::Sphere,
            projection: Matrix::identity(),
            to_viewer: None,
            cull_back_faces: false,
            coarse_shading: false,
        }
    }

    /// Una luna, un anillo o cualquier otra malla que acompana a un cuerpo.
    fn accessory(accessory: &Accessory) -> Self {
        SceneObject {
            normals: accessory.normals,
            ..SceneObject::new(accessory.vertices, accessory.model_matrix.clone(), accessory.shade)
        }
    }

    /// Una estrella o un planeta: esferas cerradas, sin cara de atras visible.
    fn body(vertices: &'a [Vertex], model_matrix: Matrix, shade: fn(&Frag) -> Vector3) -> Self {
        SceneObject { cull_back_faces: true, ..SceneObject::new(vertices, model_matrix, shade) }
    }
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
    /// El sistema solar completo, visto desde arriba y un poco de costado.
    fn system() -> Self {
        Camera { yaw: 0.22, pitch: 0.82, zoom: 0.035 * WIDTH.min(HEIGHT) as f32 }
    }

    /// Flechas para orbitar y Z/X para alejar o acercar.
    fn steer(&mut self, window: &RaylibHandle, dt: f32) {
        let held = |key| window.is_key_down(key);
        let turn = ORBIT_SPEED * dt;
        if held(KeyboardKey::KEY_LEFT) { self.yaw -= turn; }
        if held(KeyboardKey::KEY_RIGHT) { self.yaw += turn; }
        if held(KeyboardKey::KEY_UP) { self.pitch += turn; }
        if held(KeyboardKey::KEY_DOWN) { self.pitch -= turn; }
        self.pitch = self.pitch.clamp(-1.5, 1.5);
        if held(KeyboardKey::KEY_Z) { self.zoom /= ZOOM_SPEED.powf(dt); }
        if held(KeyboardKey::KEY_X) { self.zoom *= ZOOM_SPEED.powf(dt); }
        self.zoom = self.zoom.clamp(8.0, 900.0);
    }

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

fn transform_point(matrix: &Matrix, point: Vector3) -> Vector3 {
    Vector3::new(
        matrix.m0 * point.x + matrix.m4 * point.y + matrix.m8 * point.z + matrix.m12,
        matrix.m1 * point.x + matrix.m5 * point.y + matrix.m9 * point.z + matrix.m13,
        matrix.m2 * point.x + matrix.m6 * point.y + matrix.m10 * point.z + matrix.m14,
    )
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

/// El sistema solar completo con la camara orbital: estrellas, planetas con
/// sus lunas y anillos, y las orbitas.
fn draw_system(framebuffer: &mut Framebuffer, uniforms: &mut Uniforms, sphere: &[Vertex], camera: &Camera, time: f32) {
    uniforms.view_matrix = camera.view_matrix();
    uniforms.viewport_matrix = camera.viewport_matrix();
    uniforms.to_viewer = camera.to_viewer();
    uniforms.time = time;
    // Una sola luz puntual en el centro de la orbita de las dos estrellas.
    uniforms.sun = Light::new(Vector3::zero(), Rgb::new(1.0, 0.92, 0.78), 1.4);

    let stars = solar_system::star_poses(time);
    let planets = solar_system::planet_poses(time);
    let orbit_guides = solar_system::orbit_guides();
    let unit_sphere = fit_unit_sphere(sphere);
    let mut objects = Vec::with_capacity(stars.len() + planets.len() * 3 + orbit_guides.len());
    for star in &stars {
        objects.push(SceneObject::body(sphere, multiply(&star.model_matrix, &unit_sphere), star.shader));
    }
    for planet in &planets {
        objects.push(SceneObject::body(sphere, multiply(&planet.model_matrix, &unit_sphere), planet.shader));
        for accessory in &planet.accessories {
            objects.push(SceneObject::accessory(accessory));
        }
    }
    for guide in &orbit_guides {
        objects.push(SceneObject::accessory(guide));
    }
    framebuffer.clear_with_starfield();
    render_objects(framebuffer, uniforms, &objects);
    for (index, star) in stars.iter().enumerate() {
        let view_position = transform_point(&uniforms.view_matrix, star.center);
        let screen_position = transform_point(&uniforms.viewport_matrix, view_position);
        let glow_color = if index == 0 {
            Color::new(135, 190, 255, 255)
        } else {
            Color::new(90, 205, 255, 255)
        };
        framebuffer.add_glow(
            screen_position.x as i32,
            screen_position.y as i32,
            camera.zoom * 1.50,
            glow_color,
            0.75,
        );
    }
}

/// Titulo y nombres de la vista del sistema; `controls` cambia segun el modo.
fn system_overlay(d: &mut RaylibDrawHandle, controls: &str) {
    d.draw_text("Sistema solar", 18, 16, 24, Color::WHITE);
    d.draw_text(controls, 18, 46, 16, Color::LIGHTGRAY);
    d.draw_text("Estrellas: Lunan't + Asterion | Planetas:", 18, 68, 16, Color::LIGHTGRAY);
    d.draw_text("Bocchi, JupyterNotebook, Flora, Veridia e Iris", 18, 88, 16, Color::LIGHTGRAY);
}

/// Un circulo donde esta la nave, para ubicarse en la vista del sistema.
fn mark_ship(framebuffer: &mut Framebuffer, camera: &Camera, position: Vector3) {
    let at = transform_point(&camera.viewport_matrix(), transform_point(&camera.view_matrix(), position));
    framebuffer.ring(at.x, at.y, 7.0, 1.6, SHIP_MARKER, 1.0, |_| true);
    framebuffer.ring(at.x, at.y, 0.0, 3.0, SHIP_MARKER, 1.0, |_| true);
}

/// Lo que se ve durante el vuelo. Tab pasa a la siguiente.
#[derive(Clone, Copy, PartialEq)]
enum View {
    Ship,
    ShipWithOrbits,
    System,
}

impl View {
    fn next(self) -> Self {
        match self {
            View::Ship => View::ShipWithOrbits,
            View::ShipWithOrbits => View::System,
            View::System => View::Ship,
        }
    }
}

/// La mira reemplaza al puntero y arranca en el centro de la pantalla, para
/// no salir girando hacia donde habia quedado el mouse.
fn start_aiming(window: &mut RaylibHandle) {
    window.set_mouse_position(Vector2::new(WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0));
    window.hide_cursor();
}

fn render(
    framebuffer: &mut Framebuffer,
    uniforms: &Uniforms,
    vertex_array: &[Vertex],
    accessories: &[Accessory],
) {
    let mut objects = vec![SceneObject::body(vertex_array, uniforms.model_matrix.clone(), uniforms.shader)];
    objects.extend(accessories.iter().map(SceneObject::accessory));
    render_objects(framebuffer, uniforms, &objects);
}

fn render_objects(framebuffer: &mut Framebuffer, uniforms: &Uniforms, objects: &[SceneObject<'_>]) {
    let width = framebuffer.width as usize;

    // vertex shader y ensamblaje: cada objeto pasa sus vertices a pantalla
    let triangles: Vec<Vec<[Vertex; 3]>> = objects
        .iter()
        .map(|object| {
            let to_screen = multiply(
                &uniforms.viewport_matrix,
                &multiply(&object.projection, &uniforms.view_matrix),
            );
            let transformed: Vec<Vertex> = object.vertices
                .iter()
                .map(|vertex| vertex_shader_with_matrix(vertex, &object.model_matrix, &to_screen))
                .collect();
            let mut triangles = primitive_assembly(&transformed);
            if object.cull_back_faces {
                // un triangulo queda si alguna de sus esquinas mira a la camara,
                // con la misma normal que usa el sombreado de la esfera
                let to_viewer = object.to_viewer.unwrap_or(uniforms.to_viewer);
                let faces_viewer = |vertex: &Vertex| {
                    transform(&object.model_matrix, vertex.position.normalized(), 0.0).dot(to_viewer) > 0.0
                };
                triangles.retain(|corners| corners.iter().any(faces_viewer));
            }
            triangles
        })
        .collect();

    // rasterizacion y prueba de profundidad, en paralelo por franjas de
    // filas. El fragmento se arma solo cuando el pixel queda delante.
    let mut nearest = NEAREST.take();
    nearest.resize_with(width * framebuffer.height as usize, || None);
    framebuffer.rasterize_bands(&mut nearest, |rows, depth, cells| {
        // lo que quedo del cuadro anterior se borra aca, cada hilo sus filas
        cells.fill_with(|| None);
        let first_row = rows.start as usize;
        for (object_index, triangles) in triangles.iter().enumerate() {
            rasterize(triangles, width as i32, rows.clone(), |covered| {
                let index = (covered.y as usize - first_row) * width + covered.x as usize;
                if covered.depth < depth[index] {
                    depth[index] = covered.depth;
                    cells[index] = Some((covered.fragment(), object_index));
                }
            });
        }
    });

    let shade_at = |fragment: &Fragment, object: &SceneObject| {
        Rgb::from_vector(fragment_shader_with(
            fragment,
            uniforms,
            &object.model_matrix,
            object.shade,
            object.normals,
            object.to_viewer.unwrap_or(uniforms.to_viewer),
        ))
    };

    // sombreado grueso: en los objetos que lo piden se sombrean solo los
    // pixeles de columna y fila pares (las muestras) y el resto se interpola
    let (columns, rows) = (width / 2, framebuffer.height as usize / 2);
    let mut samples: Vec<Option<Rgb>> = Vec::new();
    if objects.iter().any(|object| object.coarse_shading) {
        samples = vec![None; columns * rows];
        framebuffer.pool().for_each_row(&mut samples, columns, |row, line| {
            for (column, sample) in line.iter_mut().enumerate() {
                if let Some((fragment, object_index)) = &nearest[2 * row * width + 2 * column] {
                    let object = &objects[*object_index];
                    if object.coarse_shading {
                        *sample = Some(shade_at(fragment, object));
                    }
                }
            }
        });
    }
    // promedio de las muestras que rodean al pixel. No sirve si alguna cae en
    // otro objeto (un borde) o si difieren mucho entre si (una linea fina,
    // como las grietas de Bocchi, que quedaria punteada): ahi el pixel se
    // sombrea entero
    let blended = |index: usize, object_index: usize| -> Option<Rgb> {
        let around = |value: usize| if value % 2 == 0 { [value, value] } else { [value - 1, value + 1] };
        let mut sum = Rgb::new(0.0, 0.0, 0.0);
        let (mut darkest, mut brightest) = (f32::INFINITY, f32::NEG_INFINITY);
        for y in around(index / width) {
            for x in around(index % width) {
                if x / 2 >= columns || y / 2 >= rows {
                    return None;
                }
                if !matches!(&nearest[y * width + x], Some((_, object)) if *object == object_index) {
                    return None;
                }
                let sample = samples[(y / 2) * columns + x / 2]?;
                let brightness = sample.r + sample.g + sample.b;
                darkest = darkest.min(brightness);
                brightest = brightest.max(brightness);
                sum = sum + sample;
            }
        }
        (brightest - darkest < COARSE_CONTRAST).then(|| sum * 0.25)
    };

    framebuffer.shade(|index| {
        let (fragment, object_index) = nearest[index].as_ref()?;
        let object = &objects[*object_index];
        let color = object.coarse_shading
            .then(|| blended(index, *object_index))
            .flatten()
            .unwrap_or_else(|| shade_at(fragment, object));
        Some(color.to_display())
    });
    NEAREST.set(nearest);
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

/// Pantalla de inicio para elegir nave y despues el vuelo por el sistema
/// solar con esa nave.
fn run_game(
    window: &mut RaylibHandle,
    raylib_thread: &RaylibThread,
    framebuffer: &mut Framebuffer,
    uniforms: &mut Uniforms,
    sphere: &[Vertex],
) {
    let ships = ships::load_reduced();
    if ships.is_empty() {
        eprintln!("no hay naves *-reduced.obj en assets/models");
        return;
    }
    let mut selector = Selector::new(ships.len());
    // None mientras se esta eligiendo nave
    let mut flight: Option<Flight> = None;
    let mut view = View::Ship;
    let mut system_camera = Camera::system();

    println!("mouse o flechas = mirar las naves | click o Enter = despegar");
    println!("W = avanzar | mouse = apuntar | R = reiniciar | Backspace = cambiar nave");
    println!("Tab = orbitas, despues el sistema completo (flechas y Z/X), despues la nave");

    while !window.window_should_close() {
        let dt = window.get_frame_time();

        if flight.is_none() {
            if let Some(chosen) = selector.update(window, dt) {
                flight = Some(Flight::new(chosen));
                view = View::Ship;
                start_aiming(window);
            }
        } else if window.is_key_pressed(KeyboardKey::KEY_BACKSPACE) {
            flight = None;
            window.show_cursor();
        } else if window.is_key_pressed(KeyboardKey::KEY_TAB) {
            view = view.next();
            match view {
                View::Ship => start_aiming(window),
                View::ShipWithOrbits => {}
                // en la vista del sistema el mouse no apunta
                View::System => window.show_cursor(),
            }
        }

        let start = Instant::now();
        match &mut flight {
            None => {
                selector.render(framebuffer, uniforms, &ships);
                let elapsed = start.elapsed().as_secs_f32() * 1000.0;
                window.set_window_title(raylib_thread, &format!("Selecciona tu nave | {elapsed:.0} ms"));
                framebuffer.swap_buffers(window, raylib_thread, |overlay| selector.overlay(overlay, &ships));
            }
            Some(game) => {
                if window.is_key_pressed(KeyboardKey::KEY_R) {
                    *game = Flight::new(game.ship);
                }
                let ship = &ships[game.ship];
                if view == View::System {
                    // la nave queda sin mando y se va frenando; el sistema sigue
                    system_camera.steer(window, dt);
                    game.update(&Controls::idle(), dt);
                    draw_system(framebuffer, uniforms, sphere, &system_camera, game.time());
                    mark_ship(framebuffer, &system_camera, game.position());
                    let elapsed = start.elapsed().as_secs_f32() * 1000.0;
                    window.set_window_title(raylib_thread, &format!("Sistema solar | {elapsed:.0} ms"));
                    framebuffer.swap_buffers(window, raylib_thread, |overlay| {
                        system_overlay(overlay, "Flechas: orbitar camara | Z/X: zoom | Tab: volver a la nave");
                        overlay.draw_text("Circulo naranja: tu nave", 18, 110, 16, SHIP_MARKER);
                    });
                } else {
                    game.update(&Controls::read(window), dt);
                    game.render(framebuffer, uniforms, ship, sphere, view == View::ShipWithOrbits);
                    let elapsed = start.elapsed().as_secs_f32() * 1000.0;
                    window.set_window_title(raylib_thread, &format!("Vuelo | {} | {elapsed:.0} ms", ship.name));
                    framebuffer.swap_buffers(window, raylib_thread, |overlay| game.overlay(overlay, ship));
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Elegir nave y volar por el sistema solar es la experiencia principal.
    // --orbital conserva la vista del sistema con camara orbital y --shaders
    // el visor para inspeccionar los shaders individualmente.
    let lab_mode = args.iter().any(|arg| arg == "--shaders");
    let solar_mode = !lab_mode && args.iter().any(|arg| arg == "--orbital");
    let game_mode = !lab_mode && !solar_mode;

    let obj = match Obj::load(MODEL_PATH) {
        Ok(obj) => obj,
        Err(problem) => {
            eprintln!("no se pudo cargar el modelo: {problem}");
            return;
        }
    };
    let vertex_array = obj.get_vertex_array();
    println!("triangulos: {}", vertex_array.len() / 3);

    let initial_camera = if solar_mode {
        Camera::system()
    } else {
        Camera { yaw: 0.0, pitch: 0.25, zoom: FIT * WIDTH.min(HEIGHT) as f32 }
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
        .title(if lab_mode { "Laboratorio de shaders" } else { "Sistema solar — renderer por software" })
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    window.set_target_fps(60);
    framebuffer.init_texture(&mut window, &raylib_thread);

    if game_mode {
        run_game(&mut window, &raylib_thread, &mut framebuffer, &mut uniforms, &vertex_array);
        return;
    }

    let mut camera = initial_camera;
    let mut sun_angle = INITIAL_SUN_ANGLE;
    let mut shader = 0;
    let mut time = 0.0;
    let mut paused = false;
    let mut drawn = None;
    let mut solar_drawn: Option<(Camera, f32)> = None;

    println!("Tab / Shift+Tab / 1-{} = cambiar shader", SHADERS.len().min(9));
    println!("flechas = orbitar la camara | Z/X = zoom | Q/E = mover el sol");
    println!("Espacio = pausar el tiempo | R = volver al inicio | P = guardar captura");

    while !window.window_should_close() {
        let dt = window.get_frame_time();
        let held = |key| window.is_key_down(key);

        if solar_mode {
            if window.is_key_pressed(KeyboardKey::KEY_SPACE) {
                paused = !paused;
                solar_drawn = None;
            }
            if window.is_key_pressed(KeyboardKey::KEY_R) {
                camera = initial_camera;
                time = 0.0;
                paused = false;
            }
            if !paused { time += dt; }
            camera.steer(&window, dt);

            let state = (camera, time);
            if solar_drawn != Some(state) {
                let start = Instant::now();
                draw_system(&mut framebuffer, &mut uniforms, &vertex_array, &camera, time);
                let elapsed = start.elapsed().as_secs_f32() * 1000.0;
                window.set_window_title(
                    &raylib_thread,
                    &format!("Sistema solar | {} planetas | {elapsed:.0} ms | t = {time:.1} s{}", solar_system::PLANETS.len(), if paused { " (pausa)" } else { "" }),
                );
                solar_drawn = Some(state);
            }

            framebuffer.swap_buffers(&mut window, &raylib_thread, |overlay| {
                system_overlay(overlay, "Flechas: orbitar camara | Z/X: zoom | Espacio: pausa | R: reiniciar");
            });
            continue;
        }

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