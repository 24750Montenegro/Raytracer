
use raylib::prelude::*;

mod fragment;
mod framebuffer;
mod line;
mod triangle;
mod vertex;

// Luces y sombras que vienen del raytracer.
#[allow(dead_code)]
mod color;
#[allow(dead_code)]
mod light;
#[allow(dead_code)]
mod material;
#[allow(dead_code)]
mod shading;

use framebuffer::Framebuffer;
use triangle::triangle;
use vertex::Vertex;


pub struct Uniforms {
    pub model_matrix: Matrix,
}

fn render (framebuffer: &mut Framebuffer, _uniforms: &Uniforms, vertex_array: &[Vertex]) {
   let edge_color = Vector3::new(1.0, 1.0, 1.0);
   
    for vertices in vertex_array.chunks_exact(3) {
        let fragments = triangle(&vertices[0], &vertices[1], &vertices[2]);
        
        for fragment in fragments {
            framebuffer.point(
                fragment.position.x as i32,
                fragment.position.y as i32,
                edge_color,
            );
        }
    }

    /// Dibujar vertices encima
    for vertex in vertex_array {
        let x = vertex.transformed_position.x as i32;
        let y = vertex.transformed_position.y as i32;
        for dy in -2..=2 {
            for dx in -2..=2 {
                framebuffer.point(x + dx, y + dy, vertex.color);
            }
        }
    }
}




fn main() {
    let window_width = 800;
    let window_height = 600;

    let (mut window, raylib_thread) = raylib::init()
        .size(window_width, window_height)
        .title("un trinagulo")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    window.set_target_fps(60);

    let mut framebuffer = Framebuffer::new(window_width as u32, window_height as u32);
    framebuffer.set_background_color(Vector3::new(0.0, 0.0, 0.0));
    framebuffer.init_texture(&mut window, &raylib_thread);

    let positions = [
        Vector3::new(300.0, 200.0, 0.0),
        Vector3::new(400.0, 400.0, 0.0),
        Vector3::new(500.0, 200.0, 0.0),
    ];

    
    let vertex_array: Vec<Vertex> = positions
    .into_iter()
    .map(|position|{
        let mut vertex = Vertex::new_with_color(position, Vector3::new(1.0, 0.0, 0.0));
        vertex.transformed_position = position;
        vertex
    }).collect();

    let uniforms = Uniforms { model_matrix: Matrix::identity() };

    while !window.window_should_close() {
        framebuffer.clear();
        render(&mut framebuffer, &uniforms, &vertex_array);
        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}


