use super::kit::*;
use super::{Accessory, Normals};

use std::sync::LazyLock;

const BANDS: f32 = 12.0;
const FALL_SPEED: f32 = 0.80;
const TURBULENCE: f32 = 0.32;
const COLOR_SPEED: f32 = 0.90;
const RING_COLOR: Vector3 = Vector3::new(0.96, 0.98, 1.0);
/// Cuanta luz llega al lado oscuro de un anillo: sus particulas de hielo
/// dispersan un poco de la luz que les da del otro lado, y bastante mas
/// cuando se mira hacia el sol a traves de ellas (a contraluz).
const RING_TRANSMISSION: f32 = 0.25;
const RING_BACKLIT: f32 = 0.9;
/// Franjas del hielo por unidad de distancia al centro del planeta.
const RING_BANDS: f32 = 14.0;

static INNER_RING: LazyLock<Vec<Vertex>> = LazyLock::new(|| ring_mesh(1.18, 1.31, 128));
static OUTER_RING: LazyLock<Vec<Vertex>> = LazyLock::new(|| ring_mesh(1.40, 1.56, 128));

// Paleta Dracula
const LIGHT: [Vector3; 4] = [
    Vector3::new(0.31, 0.98, 0.48), // green #50fa7b
    Vector3::new(0.55, 0.91, 0.99), // cyan #8be9fd
    Vector3::new(1.00, 0.47, 0.78), // pink #ff79c6
    Vector3::new(0.74, 0.58, 0.98), // purple #bd93f9
];

const DARK: [Vector3; 4] = [
    Vector3::new(0.10, 0.10, 0.13), // darker #191a21
    Vector3::new(0.16, 0.16, 0.21), // background #282a36
    Vector3::new(0.27, 0.28, 0.35), // current line #44475a
    Vector3::new(0.13, 0.13, 0.17), // dark #21222c
];

/// Recorre los colores en circulo: 0 es el primero, 1 el segundo, y entre
/// medio se mezclan.
fn cycle(colors: &[Vector3], index: f32) -> Vector3 {
    let n = colors.len();
    let i = index.rem_euclid(n as f32);
    let a = i.floor() as usize % n;
    mix(colors[a], colors[(a + 1) % n], fract(i))
}

pub fn shade(f: &Frag) -> Vector3 {
    let swirl = fbm(f.p * Vector3::new(2.0, 8.0, 2.0), 4) - 0.5;
    let franjas = wave(f.p.y * BANDS + f.time * FALL_SPEED + swirl * TURBULENCE).powf(3.0);

    let shift = f.time * COLOR_SPEED;
    let light = cycle(&LIGHT, f.p.y * 2.0 + shift);
    let dark = cycle(&DARK, f.p.y * 3.0 - shift * 1.3);
    let mut color = mix(light, dark, franjas);

    let vetas = fbm(f.p * Vector3::new(4.0, 20.0, 4.0), 2);
    color = color * (0.4 + 0.8 * vetas);

    // no emite: solo las estrellas dan luz, asi que de noche queda oscuro
    f.lit(color)
}

pub fn accessories(time: f32) -> Vec<Accessory> {
    let model_matrix =
        create_model_matrix(Vector3::zero(), 1.0, Vector3::new(0.0, time * 0.04, 0.0));
    vec![
        Accessory {
            vertices: INNER_RING.as_slice(),
            model_matrix,
            shade: shade_ring,
            normals: Normals::Mesh,
        },
        Accessory {
            vertices: OUTER_RING.as_slice(),
            model_matrix,
            shade: shade_ring,
            normals: Normals::Mesh,
        },
    ]
}

/// Hielo, como los anillos de Saturno: brillan donde les da el sol, con
/// franjas finas segun la distancia al planeta, y se apagan en su sombra.
fn shade_ring(f: &Frag) -> Vector3 {
    let radius = (f.p.x * f.p.x + f.p.z * f.p.z).sqrt();
    let bands = 0.65 + 0.35 * fbm(Vector3::new(radius * RING_BANDS, 0.0, 0.0), 3);

    // la cara del anillo que mira al sol; la luz le llega de ese lado
    let sun_side = if f.normal.dot(f.to_sun) >= 0.0 { f.normal } else { -f.normal };
    let ice = Material::new(Albedo::new(0.85, 0.3), 24.0);
    let lambert = f.shade(RING_COLOR * bands, &ice, sun_side);

    // Un anillo no es una superficie mate sino una nube de trozos de hielo.
    // Para eso se usa la ley de Lommel-Seeliger, mu0 / (mu0 + mu): Lambert
    // por 1 / (mu0 + mu). Cuanto mas de canto se lo mira, mas trozos se ven
    // iluminados uno detras del otro, y mas brilla.
    let incidence = sun_side.dot(f.to_sun).max(0.0);
    let emission = sun_side.dot(f.to_viewer).abs();
    let mut color = lambert * (2.0 / (incidence + emission).max(0.05)).min(3.0);

    // desde el lado sin sol solo se ve la luz que atraviesa el hielo, que es
    // mucha cuando el sol queda detras del anillo
    if sun_side.dot(f.to_viewer) < 0.0 {
        let backlit = (-f.to_viewer.dot(f.to_sun)).max(0.0).powf(4.0);
        color = color * (RING_TRANSMISSION + RING_BACKLIT * backlit);
    }
    color * (1.0 - 0.9 * planet_shadow(f))
}

/// 1 donde el planeta le tapa el sol al anillo y 0 donde no, con el borde de
/// la sombra suavizado.
fn planet_shadow(f: &Frag) -> f32 {
    let offset = f.point - f.center;
    let along = offset.dot(f.to_sun);
    // si el sol queda del lado de afuera, el planeta no esta en el camino
    if along >= 0.0 {
        return 0.0;
    }
    // que tan cerca del centro del planeta pasa el rayo hacia el sol
    let miss = (offset.dot(offset) - along * along).max(0.0).sqrt();
    1.0 - smoothstep(f.size * 0.96, f.size * 1.04, miss)
}
