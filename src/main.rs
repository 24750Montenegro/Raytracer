// main.rs
#![allow(unused_imports)]
#![allow(dead_code)]

use raylib::prelude::*;
use std::f32::consts::PI;
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



const DEF_DEPTH: f32 = 5.0;
const DEPH_SPEED: f32 = 1.0;
const MIN_RADIUS: f32 = 0.001;
// solo evita dividir entre cero al reescalar la profundidad
const DEPTH_EPS: f32 = 0.001;

const PALLETTE: [Color; 5] = [
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
    let mut pixel_color = Color::new(14, 27, 61, 255); //default para el color de fondo

    for object in objects {
        let intersect = object.ray_intersect(ray_origin, ray_direction);
        if intersect.is_intersecting && intersect.distance < zbuffer {
            zbuffer = intersect.distance;
            pixel_color = intersect.material.diffuse;
        }
    }
    pixel_color
}


pub fn render(framebuffer: &mut Framebuffer, objects: &[Sphere]) {
    let width = framebuffer.width as f32;
    let height = framebuffer.height as f32;
    let ray_origin = Vector3::new(0.0, 0.0, 0.0);


    for y in 0..framebuffer.height {
        for x in 0..framebuffer.width {
            let ray_direction = ray_dir(x as f32, y as f32, width, height);

            //
            let pixel_color = cast_ray(&ray_origin, &ray_direction, objects);
            framebuffer.set_current_color(pixel_color);
            framebuffer.set_pixel(x, y)
        }
    }

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

    let mut objects: Vec<Sphere> = Vec::new();
    let mut dragging: Option<usize> = None;
    let mut selected: Option<usize> = None;
    let mut dirty = true;

   

    while !window.window_should_close() {
        let mouse = window.get_mouse_position();
        let dt = window.get_frame_time();

        //crear esferas con click izquierdo
        if window.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
            let depth = match selected {
                Some(i) => -objects[i].center.z,
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
            selected = Some(index);
            dirty = true;
        }

        if let Some(i) = dragging {
            let index = i;
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

        //mover z con w/s
        if let Some(i) = selected {
            let mut delta = 0.0;
            if window.is_key_down(KeyboardKey::KEY_W) {
                delta += DEPH_SPEED * dt; // más profundo
            }
            if window.is_key_down(KeyboardKey::KEY_S) {
                delta -= DEPH_SPEED * dt; // más cerca
            }

            // cambiar color con a/d siguiendo el orden del array
            let mut step = 0i32;
            if window.is_key_pressed(KeyboardKey::KEY_A) {
                step -= 1;
            }
            if window.is_key_pressed(KeyboardKey::KEY_D) {
                step += 1;
            }

            if step != 0 {
                let len = PALLETTE.len() as i32;
                let current = palette_index(objects[i].material.diffuse) as i32;
                let next = (current + step).rem_euclid(len) as usize;
                objects[i].material.diffuse = PALLETTE[next];
                dirty = true;
            }

            if delta != 0.0 {
                let sphere = &mut objects[i];
                let depth = -sphere.center.z;
                let new_depth = depth + delta;

                // sin tope: tan lejos o tan cerca como quiera, pero sin cruzar la camara
                if new_depth.abs() >= DEPTH_EPS && depth.abs() >= DEPTH_EPS {
                    let k = new_depth / depth;
                    sphere.center = sphere.center * k;
                    dirty = true;
                }
            }
        }
    
        if dirty {
            framebuffer.clear();
            render(&mut framebuffer, &objects);
            dirty = false;
        }

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}