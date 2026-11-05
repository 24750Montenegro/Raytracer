use super::kit::*;

pub fn shade(f: &Frag) -> Vector3 {
    let height = fbm(f.p * 2.0, 6);

    f.lit(gray(height))
}
