use raylib::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct Material{
    pub diffuse: Color,

}

impl Material {
    pub fn black() -> Self{
        Material { diffuse: Color::BLACK }
    }
}