use super::kit::*;

const SPIN: f32 = 0.03;
const FLOW_SPEED: f32 = 0.40;
const SCALE: f32 = 2.0;
const VEINS: (f32, f32) = (0.25, 0.85);
const BRIGHTNESS: f32 = 1.4;
const PULSE: f32 = 0.80;

const DARK: Vector3 = Vector3::new(0.30, 0.30, 0.33);
const PURPLE: Vector3 = Vector3::new(71.0, 5.0, 97.0);
const PURPLE_HOT: Vector3 = Vector3::new(224.0, 177.0, 242.0);

pub fn shade(f: &Frag) -> Vector3 {
    let p = rotate_y(f.p, f.time * SPIN) * SCALE + f.time * FLOW_SPEED;
    let heat = smoothstep(VEINS.0, VEINS.1, ridged(p, 5));

    let mut color = ramp(&[(0.0, DARK), (0.5, PURPLE), (1.0, PURPLE_HOT)], heat);

    let facing = f.normal.dot(f.to_viewer).max(0.0);
    color = color * (0.65 + 0.35 * facing.sqrt());

    // el fondo (heat 0) solo se oscurece con el pulso; las venas laten completas
    let pulse = 1.0 + PULSE * (f.time * 1.2).sin();
    let pulse = pulse.min(1.0) + (pulse - 1.0).max(0.0) * heat;

    color * BRIGHTNESS * pulse
}
