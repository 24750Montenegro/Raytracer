// main.rs
// Base para el rasterizador: abre la ventana y muestra el framebuffer.
// La guia en docs/guia-rasterizador.md explica como construir el pipeline
// (vertices -> triangulos -> fragmentos -> framebuffer) encima de esto.
use raylib::prelude::*;

mod framebuffer;

// Luces y sombras que vienen del raytracer. Todavia no las usa nadie: las
// va a llamar el fragment shader cuando se rellenen las caras.
#[allow(dead_code)]
mod color;
#[allow(dead_code)]
mod light;
#[allow(dead_code)]
mod material;
#[allow(dead_code)]
mod shading;

use framebuffer::Framebuffer;

fn main() {
    let window_width = 800;
    let window_height = 600;

    let (mut window, raylib_thread) = raylib::init()
        .size(window_width, window_height)
        .title("Rasterizador")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    window.set_target_fps(60);

    let mut framebuffer = Framebuffer::new(window_width as u32, window_height as u32);
    framebuffer.set_background_color(Color::BLACK);

    while !window.window_should_close() {
        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}
