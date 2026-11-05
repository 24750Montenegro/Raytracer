// - se prueba el centro de cada pixel, (x + 0.5, y + 0.5);
// - un centro que cae justo sobre un borde se dibuja solo si ese borde es
//   izquierdo o superior (regla "top-left"). Asi dos triangulos que
//   comparten una arista no dibujan dos veces el mismo pixel ni dejan un
//   hueco entre ellos.
use raylib::prelude::*;

use crate::fragment::Fragment;
use crate::vertex::Vertex;

/// por debajo de esta area (en pixeles cuadrados) el triangulo se trata como
/// degenerado: sus tres vertices estan practicamente sobre una misma linea
const DEGENERATE_AREA: f32 = 1e-6;

/// Relleno con coordenadas baricentricas. Ademas de la profundidad,
/// interpola las posiciones que usa el fragment shader.
pub fn triangle(v1: &Vertex, v2: &Vertex, v3: &Vertex, width: i32, height: i32, fragments: &mut Vec<Fragment>) {
    let (a, mut b, mut c) = (v1, v2, v3);

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
    let min_y = first_pixel(pa.y.min(pb.y).min(pc.y)).max(0);
    let max_y = last_pixel(pa.y.max(pb.y).max(pc.y)).min(height - 1);

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
                fragments.push(Fragment {
                    position: Vector2::new(x as f32, y as f32),
                    depth: pa.z * u + pb.z * v + pc.z * w,
                    object_position: a.position * u + b.position * v + c.position * w,
                    world_position: a.world_position * u + b.world_position * v + c.world_position * w,
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
