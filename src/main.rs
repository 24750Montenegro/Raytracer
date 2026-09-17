// main.rs
use raylib::prelude::*;

mod bvh;
mod camera;
mod framebuffer;
mod material;
mod projection;
mod render;
mod scene;
mod sphere;

use bvh::Bvh;
use camera::Camera;
use framebuffer::Framebuffer;
use render::BACKGROUND_COLOR;
use sphere::Sphere;

/// Que tan lejos arranca la camara respecto al tamano de la escena.
const START_ZOOM: f32 = 1.6;

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

    let mut objects: Vec<Sphere> = scene::preset_scene();
    // la escena no cambia nunca, asi que el BVH se arma una sola vez
    let bvh = Bvh::build(&mut objects);

    // la camara arranca encuadrando la escena completa
    let (center, radius) = scene::bounding_sphere(&objects);
    let fit_distance = radius / (projection::FOV / 2.0).tan();
    let mut camera = Camera::new(center, fit_distance * START_ZOOM);

    let mut last_mouse = window.get_mouse_position();
    let mut dirty = true;

    println!("arrastrar con click izquierdo = girar la escena | scroll = zoom");

    while !window.window_should_close() {
        let mouse = window.get_mouse_position();
        let delta = mouse - last_mouse;
        last_mouse = mouse;

        if window.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT)
            && camera.orbit(delta.x, delta.y)
        {
            dirty = true;
        }

        if camera.zoom(window.get_mouse_wheel_move()) {
            dirty = true;
        }

        if dirty {
            // render escribe TODOS los pixeles, no hace falta limpiar antes
            render::render(&mut framebuffer, &objects, &bvh, &camera.basis());
            dirty = false;
        }

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}
