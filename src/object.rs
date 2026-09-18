// object.rs
// Un cuerpo de la escena, sea cual sea su forma.
//
// Es un enum y no un trait object a proposito: las dos formas son Copy y
// chicas, asi que la escena sigue siendo un Vec contiguo que el BVH puede
// reordenar de un lado a otro, y el despacho se resuelve en compilacion. Con
// Box<dyn Hittable> cada test de interseccion seria un salto indirecto y un
// acceso a otra parte de la memoria, dentro del lazo mas caliente que hay.
use raylib::prelude::*;

use crate::cube::Cube;
use crate::material::Material;
use crate::sphere::Sphere;

#[derive(Clone, Copy)]
pub enum Object {
    Sphere(Sphere),
    Cube(Cube),
}

impl Object {
    pub fn sphere(center: Vector3, radius: f32, material: Material) -> Self {
        Object::Sphere(Sphere {
            center,
            radius,
            material,
        })
    }

    /// Cubo por centro y lado: asi se ubica igual que una esfera.
    pub fn cube(center: Vector3, size: f32, material: Material) -> Self {
        Object::Cube(Cube::new(center, size, material))
    }

    /// Distancia al impacto mas cercano por delante del origen.
    /// inv_dir es el inverso componente a componente de direction, que el BVH
    /// ya calcula una vez por rayo; la esfera no lo necesita pero el cubo si.
    #[inline(always)]
    pub fn hit_distance(
        &self,
        ray_origin: &Vector3,
        ray_direction: &Vector3,
        inv_dir: &Vector3,
    ) -> Option<f32> {
        match self {
            Object::Sphere(sphere) => sphere.hit_distance(ray_origin, ray_direction),
            Object::Cube(cube) => cube.hit_distance(ray_origin, inv_dir),
        }
    }

    /// Normal unitaria hacia afuera en un punto de la superficie.
    #[inline(always)]
    pub fn normal_at(&self, point: Vector3) -> Vector3 {
        match self {
            Object::Sphere(sphere) => sphere.normal_at(point),
            Object::Cube(cube) => cube.normal_at(point),
        }
    }

    pub fn material(&self) -> &Material {
        match self {
            Object::Sphere(sphere) => &sphere.material,
            Object::Cube(cube) => &cube.material,
        }
    }

    /// Centro del cuerpo. El BVH parte la escena comparando centros.
    pub fn center(&self) -> Vector3 {
        match self {
            Object::Sphere(sphere) => sphere.center,
            Object::Cube(cube) => cube.center(),
        }
    }

    /// Esquinas de la caja mas chica que lo contiene.
    pub fn bounds(&self) -> (Vector3, Vector3) {
        match self {
            Object::Sphere(sphere) => {
                let r = Vector3::new(sphere.radius, sphere.radius, sphere.radius);
                (sphere.center - r, sphere.center + r)
            }
            Object::Cube(cube) => (cube.min, cube.max),
        }
    }
}
