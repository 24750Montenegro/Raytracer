// main.rs
use raylib::prelude::*;

mod bvh;
mod camera;
mod color;
mod cube;
mod framebuffer;
mod json;
mod light;
mod material;
mod model;
mod noise;
mod object;
mod projection;
mod render;
mod scene;
mod shading;
mod terrain;
mod texture;

use camera::Camera;
use framebuffer::Framebuffer;
use render::Quality;
use scene::{Scene, BACKGROUND_COLOR};

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

    let args: Vec<String> = std::env::args().skip(1).collect();
    // `cargo run -- <semilla>` repite un terreno que gusto.
    let seed = args.first().and_then(|arg| arg.parse().ok());
    let scene = seed.map_or_else(Scene::random, Scene::from_seed);

    // encuadra el terreno completo, visto desde arriba
    let mut camera = Camera::framing(scene.center, scene.radius);

    println!("semilla del terreno: {}", scene.seed);
    println!("cuerpos trazados: {}", scene.objects.len());
    println!("luces: {}", scene.lights.len());

    // `cargo run -- [semilla] --shot salida.png [yaw pitch zoom isla]` traza
    // un cuadro, lo guarda y sale. Sirve para sacar capturas sin depender de
    // agarrar la ventana en la posicion justa. `isla` es 0, 1, 2 o 3 y le
    // apunta la camara a esa isla en vez de al hueco que queda entre las
    // cuatro.
    if let Some(at) = args.iter().position(|arg| arg == "--shot") {
        let Some(path) = args.get(at + 1) else {
            eprintln!("--shot necesita el archivo donde guardar el cuadro");
            return;
        };

        let number = |offset: usize| args.get(at + offset).and_then(|arg| arg.parse::<f32>().ok());
        if let Some(yaw) = number(2) {
            camera.yaw = yaw;
        }
        if let Some(pitch) = number(3) {
            camera.set_pitch(pitch);
        }
        if let Some(zoom) = number(4) {
            camera.zoom_by(zoom);
        }
        if let Some(island) = number(5) {
            let centers = terrain::island_centers();
            camera.target = centers[(island as usize).min(centers.len() - 1)];
        }

        render::render(&mut framebuffer, &scene, &camera.basis(), Quality::FULL);
        match framebuffer.save(path) {
            Ok(()) => println!("cuadro guardado en {path}"),
            Err(problem) => eprintln!("{problem}"),
        }
        return;
    }

    let mut last_mouse = window.get_mouse_position();
    // arranca en false para que el primer frame ya salga en calidad final
    let mut refined = false;

    println!(
        "click izquierdo = girar | click derecho = correr la vista | scroll = zoom"
    );

    while !window.window_should_close() {
        let mouse = window.get_mouse_position();
        let delta = mouse - last_mouse;
        last_mouse = mouse;

        let orbited = window.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT)
            && camera.orbit(delta.x, delta.y);
        let panned = window.is_mouse_button_down(MouseButton::MOUSE_BUTTON_RIGHT)
            && camera.pan(delta.x, delta.y);
        let zoomed = camera.zoom(window.get_mouse_wheel_move());

        // render escribe TODOS los pixeles, no hace falta limpiar antes
        if orbited || panned || zoomed {
            // mientras se mueve alcanza con el trazado grueso: la imagen
            // cambia entera en el cuadro siguiente igual
            render::render(&mut framebuffer, &scene, &camera.basis(), Quality::MOVING);
            refined = false;
        } else if !refined {
            // ya quedo quieta: un unico pase fino y nada mas hasta que se mueva
            render::render(&mut framebuffer, &scene, &camera.basis(), Quality::FULL);
            refined = true;
        }

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}
