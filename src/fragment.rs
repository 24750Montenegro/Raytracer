use raylib::math::{Vector2, Vector3};

pub struct Fragment { 
    pub position: Vector2, 
    pub color: Vector3,
    pub depth: f32,
}

impl Fragment {

    pub fn new(x: f32, y: f32, color: Vector3, depth: f32) -> Self{
        Self { position: Vector2::new(x,y), color, depth }

    }
}