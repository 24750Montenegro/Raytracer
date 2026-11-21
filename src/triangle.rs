// - se prueba el centro de cada pixel, (x + 0.5, y + 0.5);
// - un centro que cae justo sobre un borde se dibuja solo si ese borde es
//   izquierdo o superior (regla "top-left"). Asi dos triangulos que
//   comparten una arista no dibujan dos veces el mismo pixel ni dejan un
//   hueco entre ellos.
use raylib::prelude::*;
use std::ops::Range;

use crate::fragment::Fragment;
use crate::vertex::Vertex;

/// por debajo de esta area (en pixeles cuadrados) el triangulo se trata como
/// degenerado: sus tres vertices estan practicamente sobre una misma linea
const DEGENERATE_AREA: f32 = 1e-6;

/// Un pixel que cubre el triangulo. Trae la profundidad para la prueba de
/// profundidad, y el fragmento completo se arma solo si la pasa: la mayoria
/// de los pixeles tapados nunca llegan a interpolar nada mas.
pub struct Covered<'a> {
    pub x: i32,
    pub y: i32,
    pub depth: f32,
    weights: [f32; 3],
    corners: [&'a Vertex; 3],
}

impl Covered<'_> {
    /// Interpola las posiciones y la normal que usa el fragment shader.
    pub fn fragment(&self) -> Fragment {
        let [u, v, w] = self.weights;
        let [a, b, c] = self.corners;
        Fragment {
            object_position: a.position * u + b.position * v + c.position * w,
            world_position: a.world_position * u + b.world_position * v + c.world_position * w,
            normal: a.normal * u + b.normal * v + c.normal * w,
        }
    }
}

/// Relleno con coordenadas baricentricas, solo en las filas `rows`: asi
/// varios hilos pueden rellenar el mismo triangulo, cada uno en sus filas.
pub fn triangle<'a>(
    v1: &'a Vertex,
    v2: &'a Vertex,
    v3: &'a Vertex,
    width: i32,
    rows: Range<i32>,
    emit: &mut impl FnMut(Covered<'a>),
) {
    let (a, mut b, mut c) = (v1, v2, v3);

    // lo mas barato primero: un triangulo que no toca estas filas se descarta
    // sin calcular nada mas
    let (ya, yb, yc) = (a.transformed_position.y, b.transformed_position.y, c.transformed_position.y);
    let min_y = first_pixel(ya.min(yb).min(yc)).max(rows.start);
    let max_y = last_pixel(ya.max(yb).max(yc)).min(rows.end - 1);
    if min_y > max_y {
        return;
    }

    let mut area = edge_function(
        a.transformed_position,
        b.transformed_position,
        c.transformed_position.x,
        c.transformed_position.y,
    );
    if area.abs() < DEGENERATE_AREA {
        return;
    }
    if area < 0.0 {
        std::mem::swap(&mut b, &mut c);
        area = -area;
    }

    let (pa, pb, pc) = (a.transformed_position, b.transformed_position, c.transformed_position);

    let min_x = first_pixel(pa.x.min(pb.x).min(pc.x)).max(0);
    let max_x = last_pixel(pa.x.max(pb.x).max(pc.x)).min(width - 1);

    let keep_bc = is_top_left(pb, pc);
    let keep_ca = is_top_left(pc, pa);
    let keep_ab = is_top_left(pa, pb);
    let inverse_area = 1.0 / area;

    for y in min_y..=max_y {
        let py = y as f32 + 0.5;

        for x in min_x..=max_x {
            let px = x as f32 + 0.5;

            let w_a = edge_function(pb, pc, px, py);
            let w_b = edge_function(pc, pa, px, py);
            let w_c = edge_function(pa, pb, px, py);

            if covers(w_a, keep_bc) && covers(w_b, keep_ca) && covers(w_c, keep_ab) {
                let (u, v, w) = (w_a * inverse_area, w_b * inverse_area, w_c * inverse_area);
                emit(Covered {
                    x,
                    y,
                    depth: pa.z * u + pb.z * v + pc.z * w,
                    weights: [u, v, w],
                    corners: [a, b, c],
                });
            }
        }
    }
}

/// doble del area con signo del triangulo (a, b, p). Es positiva de un lado
/// del borde a -> b, negativa del otro y cero sobre el borde
fn edge_function(a: Vector3, b: Vector3, px: f32, py: f32) -> f32 {
    (b.x - a.x) * (py - a.y) - (b.y - a.y) * (px - a.x)
}

/// un centro sobre el borde (peso cero) cuenta solo si el borde es
/// izquierdo o superior
fn covers(weight: f32, keep_edge: bool) -> bool {
    weight > 0.0 || (weight == 0.0 && keep_edge)
}

/// si el borde from -> to es izquierdo o superior en un triangulo cuyo
/// interior da pesos positivos
fn is_top_left(from: Vector3, to: Vector3) -> bool {
    to.y < from.y || (to.y == from.y && to.x > from.x)
}

/// primer pixel cuyo centro queda en `value` o despues
fn first_pixel(value: f32) -> i32 {
    (value - 0.5).ceil() as i32
}

/// ultimo pixel cuyo centro queda en `value` o antes
fn last_pixel(value: f32) -> i32 {
    (value - 0.5).floor() as i32
}
