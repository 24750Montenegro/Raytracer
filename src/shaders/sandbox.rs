use super::kit::*;

pub fn shade(f: &Frag) -> Vector3 {
    let mut color = Vector3::new(0.11, 0.14, 0.25);

    let franjas = wave(f.p.y * 12.0 + f.time);
    color = mix(color, Vector3::new(0.36, 0.48, 0.84), franjas);

    let calor = fbm(f.p * 4.0, 4);
    color = color + Vector3::new(1.0, 0.7, 0.28) * calor * 0.3;

    color = color * (0.6 + 0.4 * cellular(f.p * 6.0));

    f.lit(color)
}
