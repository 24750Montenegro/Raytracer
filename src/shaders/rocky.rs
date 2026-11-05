use super::kit::*;

const SPIN: f32 = 0.05;
const TERRAIN_SCALE: f32 = 1.7;
const SEA_LEVEL: f32 = 0.37;
const PEAK_LEVEL: f32 = 0.62;
const ICE_LATITUDE: f32 = 0.78;
const BUMP: f32 = 0.06;
const CLOUD_SCALE: f32 = 2.4;
const CLOUD_SPEED: f32 = 0.09;
const CLOUD_COVER: f32 = 0.55;
const CLOUD_HEIGHT: f32 = 0.04;
const CRACK_SCALE: f32 = 5.0;
const NIGHT_GLOW: f32 = 0.9;

const SEA_DEEP: Vector3 = Vector3::new(0.26, 0.64, 0.90);
const CELESTE: Vector3 = Vector3::new(0.58, 0.90, 1.0);
const FOAM: Vector3 = Vector3::new(0.90, 0.97, 1.0);
const BLUSH: Vector3 = Vector3::new(1.0, 0.78, 0.86);
const PINK: Vector3 = Vector3::new(0.96, 0.50, 0.70);
const HOT_PINK: Vector3 = Vector3::new(0.90, 0.38, 0.62);
const MAUVE: Vector3 = Vector3::new(0.62, 0.36, 0.52);
const SILVER: Vector3 = Vector3::new(0.84, 0.86, 0.90);
const CLOUD: Vector3 = Vector3::new(1.0, 0.93, 0.97);
const HAZE: Vector3 = Vector3::new(1.0, 0.62, 0.80);

fn terrain(p: Vector3) -> f32 {
    fbm(p * TERRAIN_SCALE + 3.0, 4)
}

fn clouds(p: Vector3, time: f32) -> f32 {
    let q = rotate_y(p, time * CLOUD_SPEED) * Vector3::new(1.0, 3.0, 1.0);
    smoothstep(CLOUD_COVER, CLOUD_COVER + 0.25, fbm(q * CLOUD_SCALE, 4))
}

pub fn shade(f: &Frag) -> Vector3 {
    let spin = f.time * SPIN;
    let height = |x: Vector3| terrain(rotate_y(x, spin)).max(SEA_LEVEL);
    let p = rotate_y(f.p, spin);
    let h = terrain(p);

    let ice = smoothstep(ICE_LATITUDE, ICE_LATITUDE + 0.06, latitude(p).abs() + (h - 0.5) * 0.4);
    let peak = smoothstep(PEAK_LEVEL, PEAK_LEVEL + 0.04, h).max(ice);

    let ground = if h < SEA_LEVEL {
        let depth = (SEA_LEVEL - h) / 0.2;
        let water = mix(CELESTE, SEA_DEEP, depth);
        let water = mix(FOAM, water, smoothstep(0.0, 0.08, depth));
        let glassy = Material::new(Albedo::new(0.9, 0.3), 140.0);
        f.shade(mix(water, SILVER, ice), &glassy, f.normal)
    } else {
        let land = (h - SEA_LEVEL) / (PEAK_LEVEL - SEA_LEVEL);
        let rock = ramp(&[(0.0, BLUSH), (0.12, PINK), (0.55, HOT_PINK), (1.0, MAUVE)], land);
        let normal = bump(f.normal, f.p, h, BUMP, height);
        let silver = Material { metallic: true, ..Material::new(Albedo::new(0.75, 0.3), 80.0) };
        mix(
            f.shade(rock, &Material::matte(), normal),
            f.shade(SILVER, &silver, normal),
            peak,
        )
    };

    let cloud = clouds(f.p, f.time);
    let shadow = clouds((f.p + f.to_sun * CLOUD_HEIGHT).normalized(), f.time);
    let mut color = ground * (1.0 - 0.55 * shadow);

    let night = 1.0 - smoothstep(0.0, 0.12, f.light);
    if night > 0.0 && h > SEA_LEVEL && peak < 1.0 {
        let crack = 1.0 - smoothstep(0.0, 0.05, cell_edge(p * CRACK_SCALE));
        color = color + CELESTE * crack * night * NIGHT_GLOW * (1.0 - peak) * (1.0 - cloud);
    }

    color = mix(color, f.lit(CLOUD), cloud * 0.92);
    color + HAZE * rim(f, 2.5) * (0.08 + 0.5 * f.light)
}
