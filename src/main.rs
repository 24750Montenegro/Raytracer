// main.rs
use raylib::prelude::*;

mod bvh;
mod camera;
mod color;
mod cube;
mod framebuffer;
mod light;
mod material;
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

    let scene = Scene::random();

    // encuadra el terreno completo, visto desde arriba
    let mut camera = Camera::framing(scene.center, scene.radius);

    let mut last_mouse = window.get_mouse_position();
    // arranca en false para que el primer frame ya salga en calidad final
    let mut refined = false;

    println!("semilla del terreno: {}", scene.seed);
    println!("arrastrar con click izquierdo = girar la escena | scroll = zoom");

    while !window.window_should_close() {
        let mouse = window.get_mouse_position();
        let delta = mouse - last_mouse;
        last_mouse = mouse;

        let orbited = window.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT)
            && camera.orbit(delta.x, delta.y);
        let zoomed = camera.zoom(window.get_mouse_wheel_move());

        // render escribe TODOS los pixeles, no hace falta limpiar antes
        if orbited || zoomed {
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
