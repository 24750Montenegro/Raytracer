use raylib::prelude::*;
use std::f32::consts::PI;

pub const FOV: f32 = PI/3.0;

pub fn ray_dir_raw(x: f32, y: f32, width: f32, height: f32) -> Vector3 {
    let aspect_ratio = width / height;
    let perspective_scale = (FOV / 2.0).tan();

    let screen_x = (2.0 * x) / width - 1.0;
    let screen_y = (2.0 * y) / height - 1.0;

    let screen_x = screen_x * aspect_ratio * perspective_scale;
    let screen_y = screen_y * perspective_scale;

    Vector3::new(screen_x, screen_y, -1.0)
}

pub fn ray_dir(x: f32, y: f32, width: f32, height: f32) -> Vector3 {
    ray_dir_raw(x, y, width, height).normalized()
}

pub fn screen_to_world(x: f32, y: f32, width: f32, height: f32, depth: f32) -> Vector3 {
    ray_dir_raw(x, y, width, height) * depth
}