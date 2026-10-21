// Los dos rellenos usan la misma regla, asi que generan las mismas
// posiciones:
// - se prueba el centro de cada pixel, (x + 0.5, y + 0.5);
// - un centro que cae justo sobre un borde se dibuja solo si ese borde es
//   izquierdo o superior (regla "top-left"). Asi dos triangulos que
//   comparten una arista no dibujan dos veces el mismo pixel ni dejan un
//   hueco entre ellos.
use raylib::prelude::*;

use crate::fragment::Fragment;
use crate::line::line;
use crate::vertex::Vertex;

/// por debajo de esta area (en pixeles cuadrados) el triangulo se trata como
/// degenerado: sus tres vertices estan practicamente sobre una misma linea
const DEGENERATE_AREA: f32 = 1e-6;

pub fn triangle(v1: &Vertex, v2: &Vertex, v3: &Vertex) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    fragments.extend(line(v1, v2));
    fragments.extend(line(v2, v3));
    fragments.extend(line(v3, v1));
    fragments
}

/// Relleno con coordenadas baricentricas.
pub fn triangle_barycentric(v1: &Vertex, v2: &Vertex, v3: &Vertex) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    let color = flat_color(v1, v2, v3);

    let a = v1.transformed_position;
    let mut b = v2.transformed_position;
    let mut c = v3.transformed_position;

    // el doble del area con signo; el signo depende del orden de los vertices
    let mut area = edge_function(a, b, c.x, c.y);
    if area.abs() < DEGENERATE_AREA {
        return fragments;
    }
    // con el orden invertido todos los pesos salen negativos: se da vuelta
    // el triangulo para que adentro siempre sea positivo
    if area < 0.0 {
        std::mem::swap(&mut b, &mut c);
        area = -area;
    }

    // el rectangulo mas chico que lo contiene, en pixeles
    let min_x = first_pixel(a.x.min(b.x).min(c.x));
    let max_x = last_pixel(a.x.max(b.x).max(c.x));
    let min_y = first_pixel(a.y.min(b.y).min(c.y));
    let max_y = last_pixel(a.y.max(b.y).max(c.y));

    // que hacer con un centro que cae justo sobre cada borde
    let keep_bc = is_top_left(b, c);
    let keep_ca = is_top_left(c, a);
    let keep_ab = is_top_left(a, b);

    for y in min_y..=max_y {
        let py = y as f32 + 0.5;

        for x in min_x..=max_x {
            let px = x as f32 + 0.5;

            // cada peso es el area del sub-triangulo opuesto a un vertice
            let w_a = edge_function(b, c, px, py);
            let w_b = edge_function(c, a, px, py);
            let w_c = edge_function(a, b, px, py);

            if covers(w_a, keep_bc) && covers(w_b, keep_ca) && covers(w_c, keep_ab) {
                // coordenadas baricentricas: los pesos divididos por el area
                let depth = (w_a * a.z + w_b * b.z + w_c * c.z) / area;
                fragments.push(Fragment::new(x as f32, y as f32, color, depth));
            }
        }
    }

    fragments
}

/// Relleno por lineas horizontales.
pub fn triangle_scanline(v1: &Vertex, v2: &Vertex, v3: &Vertex) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    let color = flat_color(v1, v2, v3);

    let mut points = [
        v1.transformed_position,
        v2.transformed_position,
        v3.transformed_position,
    ];

    // tres puntos alineados no cubren ningun centro: no hay nada que rellenar
    if edge_function(points[0], points[1], points[2].x, points[2].y).abs() < DEGENERATE_AREA {
        return fragments;
    }

    // de arriba hacia abajo (y crece hacia abajo)
    points.sort_by(|p, q| p.y.total_cmp(&q.y));
    let [top, middle, bottom] = points;

    // filas cuyo centro cae en [top.y, bottom.y): la fila de arriba se
    // incluye y la de abajo no, igual que un borde superior y uno inferior
    let first_row = first_pixel(top.y);
    let last_row = first_pixel(bottom.y) - 1;

    for y in first_row..=last_row {
        let py = y as f32 + 0.5;

        // el borde largo va de top a bottom y cruza todas las filas; el otro
        // lado cambia de borde al pasar por el vertice del medio. Ninguno de
        // los dos puede ser horizontal aca: una fila nunca usa un borde que
        // empieza y termina a la misma altura
        let long_side = cross_at(top, bottom, py);
        let short_side = if py < middle.y {
            cross_at(top, middle, py)
        } else {
            cross_at(middle, bottom, py)
        };

        let (left, right) = if long_side.0 <= short_side.0 {
            (long_side, short_side)
        } else {
            (short_side, long_side)
        };

        // pixeles cuyo centro cae en [left, right): el borde izquierdo se
        // incluye y el derecho no
        let first_column = first_pixel(left.0);
        let last_column = first_pixel(right.0) - 1;
        let width = right.0 - left.0;

        for x in first_column..=last_column {
            let px = x as f32 + 0.5;
            let t = if width > 0.0 { (px - left.0) / width } else { 0.0 };
            let depth = left.1 + (right.1 - left.1) * t;
            fragments.push(Fragment::new(x as f32, y as f32, color, depth));
        }
    }

    fragments
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
/// interior da pesos positivos. Izquierdo: el interior queda a su derecha.
/// Superior: es horizontal y el interior queda debajo
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

/// donde cruza la fila `y` el borde from -> to: su x y su profundidad
fn cross_at(from: Vector3, to: Vector3, y: f32) -> (f32, f32) {
    let t = (y - from.y) / (to.y - from.y);
    (from.x + (to.x - from.x) * t, from.z + (to.z - from.z) * t)
}

/// un solo color para toda la cara: el promedio de los tres vertices
fn flat_color(v1: &Vertex, v2: &Vertex, v3: &Vertex) -> Vector3 {
    (v1.color + v2.color + v3.color) * (1.0 / 3.0)
}
