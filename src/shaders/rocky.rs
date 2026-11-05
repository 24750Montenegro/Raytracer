use super::kit::*;

pub fn shade(f: &Frag) -> Rgb {
    let height = fbm(f.p * 2.0, 6);

    f.lit(gray(height))
}
