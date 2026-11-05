use super::kit::*;

pub fn shade(f: &Frag) -> Vector3 {
    let turbulence = fbm(f.p * 3.0 + f.time * 0.1, 5);

    gray(turbulence)
}
