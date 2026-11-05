use super::kit::*;

pub fn shade(f: &Frag) -> Vector3 {
    let bands = wave(latitude(f.p) * 20.0);

    f.lit(gray(bands))
}
