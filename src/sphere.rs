use raylib::prelude::*;
use crate::material::Material;
use crate::ray_intersect::{Intersect, RayIntersect};



pub struct Sphere {
    pub center: Vector3,
    pub radius: f32,
    pub material: Material,
}

impl RayIntersect for Sphere {
    fn ray_intersect(&self, ray_origin: &Vector3, ray_direction: &Vector3) -> Intersect{
        let l = self.center - *ray_origin;
        let tca = l.dot(*ray_direction);
        let d2 = l.dot(l) - tca * tca;
        let r2 = self.radius * self.radius;

        if d2 > r2 {
            return Intersect::empty();
        }

        let thc = (r2 - d2).sqrt();
        let mut t0 = tca - thc;
        let t1 = tca + thc;

        if t0 < 0.0 { t0 = t1; }
        if t0 < 0.0 {
            return Intersect::empty();
        }

        let point = *ray_origin + *ray_direction * t0;
        let normal = (point - self.center).normalized();

        Intersect {
            distance: t0,
            point,
            normal,
            material: self.material,
            is_intersecting: true,

        }

    }
}