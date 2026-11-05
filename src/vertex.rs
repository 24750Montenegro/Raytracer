use raylib::prelude::*;

#[derive(Debug, Clone)]
pub struct Vertex {
    pub position: Vector3,
    #[allow(dead_code)]
    pub normal: Vector3,
    #[allow(dead_code)]
    pub tex_coords: Vector2,
    pub world_position: Vector3,
    pub transformed_position: Vector3,
}

impl Vertex {
    pub fn new(position: Vector3, normal: Vector3, tex_coords: Vector2) -> Self {
        Vertex {
            position,
            normal,
            tex_coords,
            world_position: position,
            transformed_position: position,
        }
    }
}
