use super::kit::*;

pub fn shade(f: &Frag) -> Rgb {
    let bands = wave(latitude(f.p) * 20.0);

    f.lit(gray(bands))
}
