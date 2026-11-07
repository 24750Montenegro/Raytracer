//! Flora: una estrella emisiva de células, pétalos y filamentos.
//! El dominio se deforma con tiempo; no gira ni altera la malla.
use super::kit::*;

const CELL_SCALE: f32 = 5.5;
const WARP_STRENGTH: f32 = 1.25;
const FLOW_SPEED: f32 = 0.18;
const FILAMENT_STRENGTH: f32 = 0.65;

const PLUM: Vector3 = Vector3::new(0.24, 0.025, 0.19);
const ROSE: Vector3 = Vector3::new(0.95, 0.22, 0.44);
const PEACH: Vector3 = Vector3::new(1.0, 0.67, 0.49);
const CREAM: Vector3 = Vector3::new(1.0, 0.90, 0.72);

fn flow(p: Vector3, time: f32) -> Vector3 {
    let t = time * FLOW_SPEED;
    Vector3::new(
        noise(p * 2.5 + Vector3::new(t, 1.7, 0.0)) - 0.5,
        noise(p * 2.5 + Vector3::new(5.2, -t * 0.8, 3.1)) - 0.5,
        noise(p * 2.5 + Vector3::new(2.3, 8.1, t * 0.6)) - 0.5,
    )
}

pub fn shade(f: &Frag) -> Vector3 {
    let drift = flow(f.p, f.time);
    let q = f.p * CELL_SCALE + drift * WARP_STRENGTH;

    // Las células son zonas calientes; sus bordes forman una red distinta.
    let cell = 1.0 - smoothstep(0.05, 0.85, cellular(q));
    let filaments = 1.0 - smoothstep(0.025, 0.17, cell_edge(q));
    let petals = wave(f.p.y * 15.0 + drift.x * 7.0 + f.time * 0.35);
    let heat = (0.72 * cell + 0.28 * petals).clamp(0.0, 1.0);
    let base = ramp(&[(0.0, PLUM), (0.45, ROSE), (1.0, PEACH)], heat);
    let color = mix(base, CREAM, filaments * FILAMENT_STRENGTH);

    // Emisión independiente del sol. El borde da volumen sin Lambert.
    let facing = f.normal.dot(f.to_viewer).max(0.0);
    let limb = 0.72 + 0.28 * facing.sqrt();
    let pulse = 0.94 + 0.06 * (f.time * 1.4 + heat * 5.0).sin();
    color * limb * pulse
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_flow_changes_without_moving_geometry() {
        let p = Vector3::new(0.3, 0.4, 0.8).normalized();
        assert!((flow(p, 0.0) - flow(p, 2.0)).length() > 0.001);
    }
}
