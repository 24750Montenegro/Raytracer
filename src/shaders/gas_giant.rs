use super::kit::*;

const BANDS: f32 = 12.0;
const FALL_SPEED: f32 = 0.80;
const TURBULENCE: f32 = 0.32;
const COLOR_SPEED: f32 = 0.90;
const GLOW: f32 = 1.0;

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

    f.shade(color, &Material::lamp(GLOW), f.normal)
}
