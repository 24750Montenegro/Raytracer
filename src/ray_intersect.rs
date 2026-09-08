use raylib::prelude::*;
use crate::material::Material;

pub struct Intersect{
    pub distance: f32,
    pub point: Vector3, 
    pub normal: Vector3, 
    pub material: Material,
    pub is_intersecting: bool,
}

impl Intersect{
    pub fn empty() -> Self{
        Intersect {
            distance: 0.0,
            point: Vector3::zero(),
            normal:Vector3::zero(),
            material: Material::black(),
            is_intersecting: false,
        }
    }
}

pub trait RayIntersect {
    fn ray_intersect(&self, ray_origin: &Vector3, ray_direction: &Vector3) -> Intersect;
}