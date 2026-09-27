// object.rs
// Un cuerpo de la escena. Hoy la unica forma es el cubo: el mundo es un
// terreno de bloques.
//
// Es un enum y no un trait object a proposito: asi la escena es un Vec
// contiguo que el BVH puede reordenar y el despacho se resuelve en
// compilacion. Con Box<dyn Hittable> cada interseccion seria un salto
// indirecto, dentro del lazo mas caliente que hay.
use raylib::prelude::*;

use crate::cube::Cube;
use crate::material::Material;
use crate::texture::Surface;

#[derive(Clone, Copy)]
pub enum Object {
    Cube(Cube),
}

impl Object {
    pub fn cube(center: Vector3, size: f32, material: Material) -> Self {
        Object::Cube(Cube::new(center, size, material))
    }

    /// Una de las cajitas de un modelo 3D, en coordenadas locales del bloque.
    pub fn part(
        block_min: Vector3,
        block_size: f32,
        local_min: Vector3,
        local_max: Vector3,
        material: Material,
    ) -> Self {
        Object::Cube(Cube::part(
            block_min, block_size, local_min, local_max, material,
        ))
    }

    /// Distancia al impacto mas cercano por delante del origen.
    /// inv_dir es el inverso componente a componente de la direccion, que el
    /// BVH ya calcula una vez por rayo y el cubo necesita seis veces.
    #[inline(always)]
    pub fn hit_distance(&self, ray_origin: &Vector3, inv_dir: &Vector3) -> Option<f32> {
        match self {
            Object::Cube(cube) => cube.hit_distance(ray_origin, inv_dir),
        }
    }

    /// Normal, ejes de la textura y coordenadas en un punto de la superficie.
    #[inline(always)]
    pub fn surface(&self, point: Vector3) -> Surface {
        match self {
            Object::Cube(cube) => cube.surface(point),
        }
    }

    pub fn material(&self) -> &Material {
        match self {
            Object::Cube(cube) => &cube.material,
        }
    }

    /// Centro del cuerpo. El BVH parte la escena comparando centros.
    pub fn center(&self) -> Vector3 {
        match self {
            Object::Cube(cube) => cube.center(),
        }
    }

    /// Esquinas de la caja mas chica que lo contiene.
    pub fn bounds(&self) -> (Vector3, Vector3) {
        match self {
            Object::Cube(cube) => (cube.min, cube.max),
        }
    }
}
