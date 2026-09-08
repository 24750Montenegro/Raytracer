// main.rs
#![allow(unused_imports)]
#![allow(dead_code)]

use raylib::prelude::*;
use std::f32::consts::PI;

mod framebuffer;
mod ray_intersect;
mod sphere;
mod material;

use framebuffer::Framebuffer;
use ray_intersect::RayIntersect;
use sphere::Sphere;
use material::Material;

pub fn cast_ray(
    ray_origin: &Vector3,
    ray_direction: &Vector3,
    objects: &[Sphere],
) -> Color {
    for object in objects {
        let material = object.ray_intersect(ray_origin, ray_direction);

        if material.diffuse != Color::BLACK {
            return material.diffuse;
        }
    }
    Color::new(4, 12, 36, 255)
}


pub fn render(framebuffer: &mut Framebuffer, objects: &[Sphere]) {
    let width = framebuffer.width as f32;
    let height = framebuffer.height as f32;
    let fov = PI/3.0;
    let aspect_ratio = width / height;
    let perspective_scale = (fov / 2.0).tan();

    for y in 0..framebuffer.height {
        for x in 0..framebuffer.width {
            let screen_x = (2.0 * x as f32) / width - 1.0;   // 0 .. 1
            let screen_y = (2.0 * y as f32) / height - 1.0;

            let screen_x = screen_x * aspect_ratio * perspective_scale;
            let screen_y = screen_y * perspective_scale;

            let ray_direction = Vector3::new(screen_x, screen_y, -1.0).normalized();
            let ray_origin = Vector3::new(0.0, 0.0, 0.0);

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

    let mut framebuffer = Framebuffer::new(window_width as u32, window_height as u32);

    framebuffer.set_background_color(Color::new(80, 80, 200, 255));

    let rubber = Material {
        diffuse: Color::new(80, 0, 0, 255),
    };

    let ivory = Material {
        diffuse: Color::new(100, 100, 80, 255),
    };

    let objects = [
        Sphere {
            center: Vector3::new(0.0, 0.0, -5.0),
            radius: 1.0,
            material: ivory,
        },
        Sphere {
            center: Vector3::new(0.0, 3.0, -10.0),
            radius: 0.5,
            material: rubber,
        }
    ];

    while !window.window_should_close() {
        framebuffer.clear();

        render(&mut framebuffer, &objects);

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}