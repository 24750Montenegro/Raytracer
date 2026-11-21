//! Iris: bandas fluidas con una distorsión elíptica localizada.
use super::kit::*;

const BAND_FREQUENCY: f32 = 27.0;
const TURBULENCE: f32 = 4.2;
const FLOW_SPEED: f32 = 0.045;
const VORTEX_STRENGTH: f32 = 4.5; // 0 = bandas sin tormenta.
const VORTEX_WIDTH: f32 = 0.36;
const VORTEX_HEIGHT: f32 = 0.24;

const BLUE: Vector3 = Vector3::new(0.19, 0.30, 0.56);
const MINT: Vector3 = Vector3::new(0.48, 0.80, 0.67);
const LAVENDER: Vector3 = Vector3::new(0.49, 0.30, 0.68);
const PINK: Vector3 = Vector3::new(0.94, 0.58, 0.73);

fn vortex(p: Vector3, strength: f32) -> Vector3 {
    let center = Vector3::new(0.32, -0.18, 0.93).normalized();
    let east = Vector3::new(center.z, 0.0, -center.x).normalized();
    let north = center.cross(east);
    let x = p.dot(east);
    let y = p.dot(north);
    let z = p.dot(center);
    let radius = ((x / VORTEX_WIDTH).powi(2) + (y / VORTEX_HEIGHT).powi(2)).sqrt();
    // La máscara de hemisferio evita una segunda tormenta en la cara opuesta.
    let angle = strength * (1.0 - smoothstep(0.0, 1.0, radius))
        * smoothstep(0.5, 0.85, z);
    let (sin, cos) = angle.sin_cos();
    (east * (x * cos - y * sin) + north * (x * sin + y * cos) + center * z).normalized()
}

pub fn shade(f: &Frag) -> Vector3 {
    // Deformar las coordenadas ANTES de las bandas curva su continuidad.
    let p = vortex(f.p, VORTEX_STRENGTH);
    let drift = Vector3::new(f.time * FLOW_SPEED, 0.0, 0.0);
    let turbulence = fbm(p * 4.0 + drift, 3);
    let fine = noise(p * 18.0 + drift * 1.3);
    let phase = p.y * BAND_FREQUENCY + turbulence * TURBULENCE + fine * 0.65;
    let band = smoothstep(0.08, 0.92, wave(phase));
    let cool = mix(BLUE, MINT, band);
    let warm = mix(LAVENDER, PINK, band);
    let color = mix(cool, warm, wave(phase * 0.43 + 1.7));
    f.lit(color * (0.88 + 0.18 * fine))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vortex_is_local_and_can_be_disabled() {
        let pole = Vector3::new(0.0, 1.0, 0.0);
        assert!((vortex(pole, 4.5) - pole).length() < 1e-5);
        let p = Vector3::new(0.42, -0.14, 0.90).normalized();
        assert!((vortex(p, 0.0) - p).length() < 1e-5);
        assert!((vortex(p, 4.5) - p).length() > 0.01);
    }
}
