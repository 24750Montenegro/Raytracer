#![allow(dead_code, unused_imports)]

use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::LazyLock;

use fastnoise_lite::{CellularReturnType, FastNoiseLite, NoiseType};

pub use super::Frag;
pub use crate::material::Material;
pub use raylib::math::Vector3;

// ---------- color: Vector3 con x = rojo, y = verde, z = azul, de 0 a 1 ----------

pub fn gray(value: f32) -> Vector3 {
    Vector3::new(value, value, value)
}

/// `a` cuando t = 0, `b` cuando t = 1.
pub fn mix(a: Vector3, b: Vector3, t: f32) -> Vector3 {
    let t = t.clamp(0.0, 1.0);
    a * (1.0 - t) + b * t
}

/// Degradado por paradas `(posicion, color)` ordenadas por posicion.
pub fn ramp(stops: &[(f32, Vector3)], t: f32) -> Vector3 {
    let (first, last) = (stops[0], stops[stops.len() - 1]);
    if t <= first.0 {
        return first.1;
    }
    for pair in stops.windows(2) {
        let ((t0, c0), (t1, c1)) = (pair[0], pair[1]);
        if t <= t1 {
            return mix(c0, c1, (t - t0) / (t1 - t0).max(f32::EPSILON));
        }
    }
    last.1
}

// ---------- forma ----------

/// 0 antes de `edge0`, 1 despues de `edge1` y una curva suave entre los dos.
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn step(edge: f32, x: f32) -> f32 {
    if x < edge { 0.0 } else { 1.0 }
}

/// Seno pasado a 0..1.
pub fn wave(x: f32) -> f32 {
    0.5 + 0.5 * x.sin()
}

/// Parte fraccionaria: 0..1 que se repite, para franjas con borde duro.
pub fn fract(x: f32) -> f32 {
    x - x.floor()
}

// ---------- ruido (todos devuelven 0..1) ----------

fn noise_source(noise_type: NoiseType, cellular: Option<CellularReturnType>) -> FastNoiseLite {
    let mut noise = FastNoiseLite::new();
    noise.set_noise_type(Some(noise_type));
    noise.set_frequency(Some(1.0));
    noise.set_cellular_return_type(cellular);
    noise
}

static SIMPLEX: LazyLock<FastNoiseLite> = LazyLock::new(|| noise_source(NoiseType::OpenSimplex2, None));
static CELL_DISTANCE: LazyLock<FastNoiseLite> =
    LazyLock::new(|| noise_source(NoiseType::Cellular, Some(CellularReturnType::Distance)));
static CELL_EDGE: LazyLock<FastNoiseLite> =
    LazyLock::new(|| noise_source(NoiseType::Cellular, Some(CellularReturnType::Distance2Sub)));
static CELL_VALUE: LazyLock<FastNoiseLite> =
    LazyLock::new(|| noise_source(NoiseType::Cellular, Some(CellularReturnType::CellValue)));

/// OpenSimplex2. Multiplicar p sube la frecuencia; sumarle algo lo desplaza.
pub fn noise(p: Vector3) -> f32 {
    0.5 + 0.5 * SIMPLEX.get_noise_3d(p.x, p.y, p.z)
}

/// Varias octavas de `noise`, cada una al doble de frecuencia y la mitad de
/// fuerza.
pub fn fbm(p: Vector3, octaves: u32) -> f32 {
    let (mut sum, mut amplitude, mut frequency, mut total) = (0.0, 1.0, 1.0, 0.0);
    for _ in 0..octaves {
        sum += SIMPLEX.get_noise_3d(p.x * frequency, p.y * frequency, p.z * frequency) * amplitude;
        total += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    0.5 + 0.5 * sum / total.max(f32::EPSILON)
}

/// Distancia al punto de celda mas cercano: 0 en el centro de cada celda.
pub fn cellular(p: Vector3) -> f32 {
    (CELL_DISTANCE.get_noise_3d(p.x, p.y, p.z) + 1.0).clamp(0.0, 1.0)
}

/// Distancia al borde entre dos celdas: 0 sobre el borde.
pub fn cell_edge(p: Vector3) -> f32 {
    (CELL_EDGE.get_noise_3d(p.x, p.y, p.z) + 1.0).clamp(0.0, 1.0)
}

/// Un valor al azar, el mismo en toda la celda.
pub fn cell_value(p: Vector3) -> f32 {
    0.5 + 0.5 * CELL_VALUE.get_noise_3d(p.x, p.y, p.z)
}

// ---------- coordenadas sobre la esfera ----------

/// -1 en el polo sur, 0 en el ecuador, 1 en el polo norte.
pub fn latitude(p: Vector3) -> f32 {
    p.y.clamp(-1.0, 1.0).asin() / FRAC_PI_2
}

/// 0..1 alrededor del eje y.
pub fn longitude(p: Vector3) -> f32 {
    p.z.atan2(p.x) / TAU + 0.5
}

/// Gira p alrededor del eje y (el de los polos).
pub fn rotate_y(p: Vector3, angle: f32) -> Vector3 {
    let (sin, cos) = angle.sin_cos();
    Vector3::new(cos * p.x + sin * p.z, p.y, -sin * p.x + cos * p.z)
}

/// Gira p alrededor de cualquier eje (Rodrigues).
pub fn rotate(p: Vector3, axis: Vector3, angle: f32) -> Vector3 {
    let k = axis.normalized();
    let (sin, cos) = angle.sin_cos();
    p * cos + k.cross(p) * sin + k * (k.dot(p) * (1.0 - cos))
}
