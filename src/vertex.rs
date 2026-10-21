use raylib::prelude:: *;

#[derive(Debug, Clone)]

pub struct Vertex {
    pub position: Vector3, 
    pub normal: Vector3,
    #[allow(dead_code)]
    pub tex_coords: Vector2,
    pub color: Vector3,
    pub transformed_position: Vector3,
    pub transformed_normal: Vector3,
}

impl Vertex {

    pub fn new(position: Vector3, normal: Vector3, tex_coords: Vector2) -> Self{
        Vertex {
            position,
            normal,
            tex_coords,   
            color: Vector3::new(1.0, 1.0, 1.0),
            transformed_position: position,
            transformed_normal: normal,
        }
    }

    
    pub fn new_with_color(position: Vector3, color: Vector3) -> Self{
        Vertex {
            position,
            normal: Vector3::zero(),
            tex_coords: Vector2::new(0.0, 0.0),
            color,
            transformed_position: Vector3::zero(),
            transformed_normal: Vector3::zero(),
        }
    }
}