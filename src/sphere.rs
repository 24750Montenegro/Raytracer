use raylib::prelude::*;
use crate::material::Material;
use crate::ray_intersect::{Intersect, RayIntersect};



pub struct Sphere {
    pub center: Vector3,
    pub radius: f32,
    pub material: Material,
}

impl Sphere {
    /// Version barata del test: solo devuelve la distancia al hit.
    /// El render no usa punto ni normal, asi que no vale la pena calcularlos
    /// (eso ahorra una raiz y una normalizacion por esfera y por pixel).
    #[inline(always)]
    pub fn hit_distance(&self, ray_origin: &Vector3, ray_direction: &Vector3) -> Option<f32> {
        let l = self.center - *ray_origin;
        let tca = l.dot(*ray_direction);
        let l2 = l.dot(l);
        let r2 = self.radius * self.radius;

        // esfera detras de la camara y origen fuera de ella -> descarte inmediato
        if tca < 0.0 && l2 > r2 {
            return None;
        }

        let d2 = l2 - tca * tca;
        if d2 > r2 {
            return None;
        }

        let thc = (r2 - d2).sqrt();
        let t0 = tca - thc;
        if t0 > 0.0 {
            return Some(t0);
        }
        let t1 = tca + thc;
        if t1 > 0.0 {
            return Some(t1);
        }
        None
    }
}

impl RayIntersect for Sphere {
    fn ray_intersect(&self, ray_origin: &Vector3, ray_direction: &Vector3) -> Intersect{
        match self.hit_distance(ray_origin, ray_direction) {
            None => Intersect::empty(),
            Some(distance) => {
                let point = *ray_origin + *ray_direction * distance;
                let normal = (point - self.center).normalized();

                Intersect {
                    distance,
                    point,
                    normal,
                    material: self.material,
                    is_intersecting: true,
                }
            }
        }
    }
}
