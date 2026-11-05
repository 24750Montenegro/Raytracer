use raylib::prelude::*;

use crate::color::Rgb;
use crate::fragment::Fragment;
use crate::material::Material;
use crate::shading::{self, SurfacePoint};
use crate::vertex::Vertex;
use crate::Uniforms;

pub mod kit;

mod gas_giant;
mod rocky;
mod sandbox;
mod star;

pub struct Shader {
    pub name: &'static str,
    pub shade: fn(&Frag) -> Vector3,
}

/// Lo que recorre Tab, en orden. Un shader nuevo se agrega aca.
pub const SHADERS: &[Shader] = &[
    Shader { name: "estrella", shade: star::shade },
    Shader { name: "planeta rocoso", shade: rocky::shade },
    Shader { name: "gigante gaseoso", shade: gas_giant::shade },
    Shader { name: "pruebas", shade: sandbox::shade },
];

/// Todo lo que un shader sabe del punto que esta pintando.
pub struct Frag<'a> {
    /// Punto de la esfera de radio 1, en coordenadas del modelo: los
    /// patrones calculados con p se quedan pegados a la esfera.
    pub p: Vector3,
    pub normal: Vector3,
    pub to_sun: Vector3,
    pub to_viewer: Vector3,
    /// Lambert del sol con la normal de la esfera: 0 de noche, 1 de frente.
    pub light: f32,
    /// Segundos desde que arranco el programa (Espacio lo pausa).
    pub time: f32,
    point: Vector3,
    uniforms: &'a Uniforms,
}

impl Frag<'_> {
    /// Iluminacion difusa basica: ambiente mas el sol.
    pub fn lit(&self, base: Vector3) -> Vector3 {
        self.shade(base, &Material::matte(), self.normal)
    }

    /// Ambiente, Lambert, Phong y emision del material, con la normal que se
    /// le pase (la de la esfera u otra calculada por el shader).
    pub fn shade(&self, base: Vector3, material: &Material, normal: Vector3) -> Vector3 {
        let surface = SurfacePoint {
            point: self.point,
            normal,
            view_dir: self.to_viewer,
        };
        shading::shade(
            std::slice::from_ref(&self.uniforms.sun),
            self.uniforms.ambient,
            material,
            Rgb::from_vector(base),
            &surface,
            shading::no_shadows,
        )
        .to_vector()
    }
}

/// Matriz por vector. raylib guarda por columnas, asi que la primera fila
/// de la matriz es m0, m4, m8, m12. w = 1 para puntos, w = 0 para
/// direcciones (la traslacion no las mueve).
fn transform(matrix: &Matrix, v: Vector3, w: f32) -> Vector3 {
    Vector3::new(
        matrix.m0 * v.x + matrix.m4 * v.y + matrix.m8 * v.z + matrix.m12 * w,
        matrix.m1 * v.x + matrix.m5 * v.y + matrix.m9 * v.z + matrix.m13 * w,
        matrix.m2 * v.x + matrix.m6 * v.y + matrix.m10 * v.z + matrix.m14 * w,
    )
}

pub fn vertex_shader(vertex: &Vertex, uniforms: &Uniforms) -> Vertex {
    let world = transform(&uniforms.model_matrix, vertex.position, 1.0);
    let view = transform(&uniforms.view_matrix, world, 1.0);
    let screen = transform(&uniforms.viewport_matrix, view, 1.0);

    Vertex {
        world_position: world,
        transformed_position: screen,
        ..vertex.clone()
    }
}

pub fn fragment_shader(fragment: &Fragment, uniforms: &Uniforms) -> Vector3 {
    let p = fragment.object_position.normalized();
    // en una esfera la normal es la direccion desde el centro: sale suave
    // aunque la malla tenga pocos triangulos
    let normal = transform(&uniforms.model_matrix, p, 0.0).normalized();
    let (to_sun, _) = uniforms.sun.direction_from(fragment.world_position);

    let frag = Frag {
        p,
        normal,
        to_sun,
        to_viewer: uniforms.to_viewer,
        light: normal.dot(to_sun).max(0.0),
        time: uniforms.time,
        point: fragment.world_position,
        uniforms,
    };

    (uniforms.shader)(&frag)
}
