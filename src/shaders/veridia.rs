//! Veridia: océanos azules, vegetación, campos rosados y bioluminiscencia.
//! Una sola malla: profundidad, relieve, nubes y sombras se calculan aquí.
use super::kit::*;

const SEA_LEVEL: f32 = 0.48;
const TERRAIN_SCALE: f32 = 2.1;
const BUMP_STRENGTH: f32 = 0.055; // 0 desactiva el relieve.
const PARALLAX_DEPTH: f32 = 0.075; // 0 desactiva el desplazamiento por vista.
const PARALLAX_STEPS: u32 = 12;
const CLOUD_COVERAGE: f32 = 0.44; // 0 = despejado; 1 = cubierto.
const CLOUD_SPEED: f32 = 0.045;
const CLOUD_HEIGHT: f32 = 0.08;
const SHADOW_STRENGTH: f32 = 0.58; // 0 conserva nubes pero quita sombras.
const NIGHT_GLOW: f32 = 0.7;
const OCEAN_SPECULAR: f32 = 0.30;

const DEEP_BLUE: Vector3 = Vector3::new(0.025, 0.10, 0.30);
const TURQUOISE: Vector3 = Vector3::new(0.08, 0.55, 0.63);
const FOREST: Vector3 = Vector3::new(0.06, 0.30, 0.19);
const MEADOW: Vector3 = Vector3::new(0.39, 0.66, 0.31);
const BLOSSOM: Vector3 = Vector3::new(0.80, 0.36, 0.59);
const MOUNTAIN: Vector3 = Vector3::new(0.48, 0.35, 0.62);
const ICE: Vector3 = Vector3::new(0.82, 0.91, 0.94);
const CLOUD: Vector3 = Vector3::new(0.89, 0.85, 0.97);
const GLOW: Vector3 = Vector3::new(0.31, 0.94, 0.68);

fn terrain(p: Vector3) -> f32 {
    let warp = Vector3::new(
        noise(p * 1.8 + Vector3::new(3.0, 1.0, 7.0)),
        noise(p * 1.8 + Vector3::new(8.0, 5.0, 2.0)),
        noise(p * 1.8 + Vector3::new(1.0, 9.0, 4.0)),
    ) - Vector3::new(0.5, 0.5, 0.5);
    fbm(p * TERRAIN_SCALE + warp * 0.65, 4)
}

fn height(p: Vector3) -> f32 {
    smoothstep(SEA_LEVEL, 0.74, terrain(p))
}

/// Marcha en el plano tangente. La profundidad depende de altura y vista,
/// nunca del tiempo; las posiciones de la malla permanecen intactas.
fn parallax(p: Vector3, normal: Vector3, viewer: Vector3, depth: f32) -> Vector3 {
    if depth <= 0.0 {
        return p;
    }
    let facing = normal.dot(viewer);
    let direction = (viewer - normal * facing) * (depth / facing.max(0.25));
    let mut previous = p;
    let mut previous_delta = -(1.0 - height(p));
    for step in 1..=PARALLAX_STEPS {
        let layer = step as f32 / PARALLAX_STEPS as f32;
        let q = (p - direction * layer).normalized();
        let delta = layer - (1.0 - height(q));
        if delta >= 0.0 {
            let weight = -previous_delta / (delta - previous_delta).max(1e-6);
            return mix(previous, q, weight).normalized();
        }
        previous = q;
        previous_delta = delta;
    }
    previous
}

/// Derivadas centrales del mismo campo que usa parallax.
/// Compatible con el modelo actual: escala uniforme, sin rotación de malla.
fn terrain_normal(p: Vector3, normal: Vector3, strength: f32) -> Vector3 {
    if strength <= 0.0 {
        return normal;
    }
    let helper = if normal.y.abs() < 0.95 {
        Vector3::new(0.0, 1.0, 0.0)
    } else {
        Vector3::new(1.0, 0.0, 0.0)
    };
    let east = helper.cross(normal).normalized();
    let north = normal.cross(east);
    let epsilon = 0.004;
    let slope_x = (height((p + east * epsilon).normalized())
        - height((p - east * epsilon).normalized())) / (2.0 * epsilon);
    let slope_y = (height((p + north * epsilon).normalized())
        - height((p - north * epsilon).normalized())) / (2.0 * epsilon);
    (normal - (east * slope_x + north * slope_y) * strength).normalized()
}

/// Coordenada analítica de una capa a altura `altitude`; no crea geometría.
/// Resolver |p + t*d|² = (1 + altitude)² con d unitario.
fn shell_point(p: Vector3, direction: Vector3, altitude: f32) -> Vector3 {
    let radius = 1.0 + altitude;
    let along = p.dot(direction);
    let distance = -along + (along * along + radius * radius - p.dot(p)).max(0.0).sqrt();
    (p + direction * distance).normalized()
}

fn cloud_mask(p: Vector3, time: f32, coverage: f32) -> f32 {
    if coverage <= 0.0 {
        return 0.0;
    }
    if coverage >= 1.0 {
        return 1.0;
    }
    let p = rotate_y(p, time * CLOUD_SPEED);
    let warp = noise(p * 3.0 + Vector3::new(7.0, 2.0, 1.0));
    let density = fbm(p * 5.5 + Vector3::new(warp, warp * 0.4, 0.0), 3);
    let threshold = 0.78 - 0.56 * coverage;
    smoothstep(threshold - 0.07, threshold + 0.07, density)
}

pub fn shade(f: &Frag) -> Vector3 {
    let p = parallax(f.p, f.normal, f.to_viewer, PARALLAX_DEPTH);
    let h = terrain(p);
    let land = smoothstep(SEA_LEVEL - 0.01, SEA_LEVEL + 0.02, h);
    let mountain = smoothstep(0.60, 0.72, h);
    let forest = mix(FOREST, MEADOW, smoothstep(0.47, 0.63, h));

    // Bioma floral: máscara independiente, no otro umbral de la misma altura.
    let humidity = fbm(p * 7.0 + Vector3::new(4.0, 1.0, 9.0), 2);
    let flowers = smoothstep(0.57, 0.70, humidity) * (1.0 - mountain);
    let vegetation = mix(forest, BLOSSOM, flowers * 0.75);
    let rock = mix(vegetation, MOUNTAIN, mountain);
    let ice = smoothstep(0.80, 0.97, p.y.abs() + mountain * 0.12);
    let ground = mix(rock, ICE, ice);
    let ocean = mix(DEEP_BLUE, TURQUOISE, smoothstep(0.29, SEA_LEVEL, h));

    let normal = terrain_normal(p, f.normal, BUMP_STRENGTH * land);
    let water_material = Material::new(Albedo::new(0.65, OCEAN_SPECULAR), 100.0);
    let water_lit = f.shade(ocean, &water_material, f.normal);
    let land_lit = f.shade(ground, &Material::matte(), normal);
    let mut color = mix(water_lit, land_lit, land);

    // Misma función/tiempo/altura, pero dos rayos diferentes: observador y sol.
    let cloud = cloud_mask(shell_point(f.p, f.to_viewer, CLOUD_HEIGHT), f.time, CLOUD_COVERAGE);
    let shadow = cloud_mask(shell_point(f.p, f.to_sun, CLOUD_HEIGHT), f.time, CLOUD_COVERAGE);
    let signed_light = f.normal.dot(f.to_sun);
    color = color * (1.0 - SHADOW_STRENGTH * shadow * smoothstep(0.0, 0.15, signed_light));

    let night = 1.0 - smoothstep(-0.20, 0.12, signed_light);
    let veins = 1.0 - smoothstep(0.015, 0.07, cell_edge(p * 22.0));
    color = color + GLOW * veins * land * night * NIGHT_GLOW;
    color = mix(color, f.lit(CLOUD), cloud * 0.90);
    color + Vector3::new(0.22, 0.39, 0.70) * rim(f, 3.5) * 0.11
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clouds_have_explicit_coverage_extremes() {
        let p = Vector3::new(0.0, 0.0, 1.0);
        assert_eq!(cloud_mask(p, 2.0, 0.0), 0.0);
        assert_eq!(cloud_mask(p, 2.0, 1.0), 1.0);
    }

    #[test]
    fn cloud_projection_follows_light_direction() {
        let p = Vector3::new(0.0, 0.0, 1.0);
        let left = Vector3::new(-0.6, 0.0, 0.8);
        let right = Vector3::new(0.6, 0.0, 0.8);
        assert!(shell_point(p, left, 0.1).x < 0.0);
        assert!(shell_point(p, right, 0.1).x > 0.0);
        assert!((shell_point(p, right, 0.0) - p).length() < 1e-5);
    }

    #[test]
    fn parallax_requires_an_oblique_view() {
        let p = Vector3::new(0.0, 0.0, 1.0);
        assert!((parallax(p, p, p, 0.1) - p).length() < 1e-5);
        let viewer = Vector3::new(0.6, 0.0, 0.8);
        assert!((parallax(p, p, viewer, 0.0) - p).length() < 1e-5);
        assert!((parallax(p, p, viewer, 0.1) - p).length() > 1e-4);
    }
}
