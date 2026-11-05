use super::kit::*;

pub fn shade(f: &Frag) -> Rgb {
    let mut color = hex(0x1b2440);

    let franjas = wave(f.p.y * 12.0 + f.time);
    color = mix(color, hex(0x5d7bd5), franjas);

    let calor = fbm(f.p * 4.0, 4);
    color = color + hex(0xffb347) * calor * 0.3;

    color = color * (0.6 + 0.4 * cellular(f.p * 6.0));

    f.lit(color)
}
