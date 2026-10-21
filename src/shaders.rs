
use raylib::prelude::*;

use crate::color::Rgb;
use crate::fragment::Fragment;
use crate::shading::{self, SurfacePoint};
use crate::vertex::Vertex;
use crate::Uniforms;

/// Hacia donde esta el observador. No hay camara todavia: se mira la
/// pantalla de frente, y como x va a la derecha e y hacia abajo, el eje z
/// apunta hacia adentro de la pantalla. El observador queda en -z.
const TO_VIEWER: Vector3 = Vector3::new(0.0, 0.0, -1.0);

/// Matriz por vector. raylib guarda por columnas, asi que la primera fila
/// de la matriz es m0, m4, m8, m12.
fn multiply_matrix_vector4(matrix: &Matrix, vector: &Vector4) -> Vector4 {
    Vector4::new(
        matrix.m0 * vector.x + matrix.m4 * vector.y + matrix.m8 * vector.z + matrix.m12 * vector.w,
        matrix.m1 * vector.x + matrix.m5 * vector.y + matrix.m9 * vector.z + matrix.m13 * vector.w,
        matrix.m2 * vector.x + matrix.m6 * vector.y + matrix.m10 * vector.z + matrix.m14 * vector.w,
        matrix.m3 * vector.x + matrix.m7 * vector.y + matrix.m11 * vector.z + matrix.m15 * vector.w,
    )
}

pub fn vertex_shader(vertex: &Vertex, uniforms: &Uniforms) -> Vertex {
    // w = 1: es un punto, asi que la traslacion lo mueve
    let position = Vector4::new(vertex.position.x, vertex.position.y, vertex.position.z, 1.0);
    let transformed = multiply_matrix_vector4(&uniforms.model_matrix, &position);

    // w = 0: es una direccion, asi que la traslacion no la mueve. Con escala
    // pareja alcanza con volver a normalizarla; con una escala distinta en
    // cada eje habria que usar la inversa transpuesta de la matriz
    let normal = Vector4::new(vertex.normal.x, vertex.normal.y, vertex.normal.z, 0.0);
    let normal = multiply_matrix_vector4(&uniforms.model_matrix, &normal);
    let normal = Vector3::new(normal.x, normal.y, normal.z);
    let length = normal.length();

    let mut result = vertex.clone();
    result.transformed_position = Vector3::new(transformed.x, transformed.y, transformed.z);
    result.transformed_normal = if length > 0.0 { normal / length } else { normal };

    if uniforms.lighting {
        result.color = light_vertex(&result, uniforms);
    }

    result
}

/// Color de un vertice con las luces de la escena (iluminacion por vertice).
fn light_vertex(vertex: &Vertex, uniforms: &Uniforms) -> Vector3 {
    // la normal del lado que se ve: asi una cara vista de espaldas no queda
    // negra
    let normal = if vertex.transformed_normal.dot(TO_VIEWER) < 0.0 {
        -vertex.transformed_normal
    } else {
        vertex.transformed_normal
    };

    let surface = SurfacePoint {
        point: vertex.transformed_position,
        normal,
        view_dir: TO_VIEWER,
    };
    let base = Rgb::from_color(uniforms.material.diffuse) * Rgb::from_vector(vertex.color);

    shading::shade(
        &uniforms.lights,
        uniforms.ambient,
        &uniforms.material,
        base,
        &surface,
        shading::no_shadows,
    )
    .to_vector()
}

/// Passthrough: el fragmento sale con el color que ya traia.
pub fn fragment_shader(fragment: &Fragment) -> Vector3 {
    fragment.color
}
