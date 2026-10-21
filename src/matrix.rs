use raylib::prelude::*;

/// raylib guarda la matriz por columnas: m0, m1, m2, m3 son la primera
/// columna. Aca todo se escribe por filas, como en papel, y esta funcion lo
/// acomoda.
pub fn new_matrix4(rows: [[f32;4]; 4]) -> Matrix {
    let [r0, r1, r2, r3] = rows;
    Matrix {
        m0: r0[0], m4: r0[1], m8: r0[2], m12: r0[3],
        m1: r1[0], m5: r1[1], m9: r1[2], m13: r1[3],
        m2: r2[0], m6: r2[1], m10: r2[2], m14: r2[3],
        m3: r3[0], m7: r3[1], m11: r3[2], m15: r3[3],
    }
}

/// las filas de una matriz, el camino inverso de new_matrix4
fn rows(m: &Matrix) -> [[f32; 4]; 4] {
    [
        [m.m0, m.m4, m.m8, m.m12],
        [m.m1, m.m5, m.m9, m.m13],
        [m.m2, m.m6, m.m10, m.m14],
        [m.m3, m.m7, m.m11, m.m15],
    ]
}

pub fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let (a,b) = (rows(a), rows(b));
    let mut result = [[0.0; 4]; 4];

    for (row, result_row) in result.iter_mut().enumerate() {
        for (column, cell) in result_row.iter_mut().enumerate(){
            *cell = (0..4).map(|k| a [row][k] * b[k][column]).sum();
        }
    }
    new_matrix4(result)
}

/// matriz para mover el objeto
pub fn translation_matrix(offset: Vector3) -> Matrix {
    new_matrix4([
        [1.0, 0.0, 0.0, offset.x],
        [0.0, 1.0, 0.0, offset.y],
        [0.0, 0.0, 1.0, offset.z],
        [0.0, 0.0, 0.0, 1.0],
    ])
}


/// matriz diagonal para escalar el objeto
pub fn scale_matrix(s: f32) -> Matrix {
    new_matrix4([
        [s, 0.0, 0.0, 0.0],
        [0.0, s, 0.0, 0.0],
        [0.0, 0.0, s, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// matriz de rotación sobre x en radianes.
pub fn rotation_x(angle: f32) -> Matrix {
    let (sin, cos) = angle.sin_cos();
    new_matrix4([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, cos, -sin, 0.0],
        [0.0, sin, cos, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// rotación sobre y en radianes.
pub fn rotation_y(angle: f32) -> Matrix {
    let (sin, cos) = angle.sin_cos();
    new_matrix4([
        [cos, 0.0, sin, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [-sin, 0.0, cos, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// rotación sobre z en radianes.
pub fn rotation_z(angle: f32) -> Matrix {
    let (sin, cos) = angle.sin_cos();
    new_matrix4([
        [cos, -sin, 0.0, 0.0],
        [sin, cos, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}


/// matriz de modelo

pub fn create_model_matrix(translation: Vector3, scale: f32, rotation: Vector3) -> Matrix {
   let rotation_matrix = multiply(
        &rotation_z(rotation.z),
        &multiply(
            &rotation_y(rotation.y),
            &rotation_x(rotation.x),
        ),
   );

   multiply(
        &translation_matrix(translation),
        &multiply(
            &rotation_matrix,
            &scale_matrix(scale),
        )
   )
}