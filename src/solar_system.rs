use raylib::prelude::*;
use std::sync::LazyLock;

use crate::matrix::{create_model_matrix, multiply, rotation_y};
use crate::shaders::{Accessory, SHADERS};
use crate::vertex::Vertex;

pub struct Body {
    pub name: &'static str,
    pub shader_name: &'static str,
    pub orbit_radius: f32,
    pub orbit_speed: f32, // radianes por segundo
    pub orbit_phase: f32, // fase inicial en radianes
    pub spin_speed: f32,  // radianes por segundo
    pub radius: f32,
}

pub struct BodyPose {
    pub name: &'static str,
    pub center: Vector3,
    pub radius: f32,
    pub model_matrix: Matrix,
    pub shader: fn(&crate::shaders::Frag) -> Vector3,
    pub accessories: Vec<Accessory>,
}

pub const PLANETS: &[Body] = &[
    Body { name: "Bocchi", shader_name: "planeta rocoso", orbit_radius: 4.0, orbit_speed: 0.48, orbit_phase: 0.35, spin_speed: 0.8, radius: 0.68 },
    Body { name: "JupyterNotebook", shader_name: "gigante gaseoso", orbit_radius: 6.3, orbit_speed: 0.29, orbit_phase: 2.45, spin_speed: 0.55, radius: 0.82 },
    Body { name: "Flora", shader_name: "flora", orbit_radius: 8.6, orbit_speed: 0.21, orbit_phase: 4.25, spin_speed: 0.32, radius: 0.72 },
    Body { name: "Veridia", shader_name: "veridia", orbit_radius: 10.9, orbit_speed: 0.15, orbit_phase: 0.95, spin_speed: 0.24, radius: 0.78 },
    Body { name: "Iris", shader_name: "iris", orbit_radius: 13.2, orbit_speed: 0.11, orbit_phase: 3.25, spin_speed: 0.20, radius: 0.70 },
];

/// Radio de la orbita que comparten las dos estrellas, una frente a la otra.
const STAR_ORBIT: f32 = 1.5;

static ORBIT_MESHES: LazyLock<Vec<Vec<Vertex>>> = LazyLock::new(|| {
    orbit_radii()
        .map(|radius| crate::shaders::kit::ring_mesh(radius - 0.04, radius + 0.04, 180))
        .collect()
});

/// Radios de todas las orbitas, centradas en el origen sobre el plano y = 0.
pub fn orbit_radii() -> impl Iterator<Item = f32> {
    std::iter::once(STAR_ORBIT).chain(PLANETS.iter().map(|planet| planet.orbit_radius))
}

fn shader_index(name: &str) -> usize {
    SHADERS.iter().position(|shader| shader.name == name)
        .unwrap_or_else(|| panic!("No existe el shader '{name}' en SHADERS"))
}

pub fn planet_poses(time: f32) -> Vec<BodyPose> {
    PLANETS.iter().map(|body| {
        let orbit = body.orbit_phase + body.orbit_speed * time;
        let (sin, cos) = orbit.sin_cos();
        let center = Vector3::new(body.orbit_radius * cos, 0.0, body.orbit_radius * sin);
        let model_matrix = multiply(
            &create_model_matrix(center, body.radius, Vector3::zero()),
            &rotation_y(body.spin_speed * time),
        );
        let shader = &SHADERS[shader_index(body.shader_name)];
        let mut accessories = (shader.accessories)(time);
        for accessory in &mut accessories {
            accessory.model_matrix = multiply(&model_matrix, &accessory.model_matrix);
        }
        BodyPose {
            name: body.name,
            center,
            radius: body.radius,
            model_matrix,
            shader: shader.shade,
            accessories,
        }
    }).collect()
}

pub fn star_poses(time: f32) -> Vec<BodyPose> {
    let angle = time * 1.35;
    let (sin, cos) = angle.sin_cos();
    // misma orbita, separadas media vuelta
    let centers = [
        Vector3::new(STAR_ORBIT * cos, 0.0, STAR_ORBIT * sin),
        Vector3::new(-STAR_ORBIT * cos, 0.0, -STAR_ORBIT * sin),
    ];
    let star_shader = &SHADERS[shader_index("estrella")];
    centers.into_iter().enumerate().map(|(index, center)| {
        let radius = if index == 0 { 0.72 } else { 0.56 };
        let spin = if index == 0 { 0.25 } else { -0.25 };
        let model_matrix = create_model_matrix(
            center,
            radius,
            Vector3::new(0.0, time * spin, 0.0),
        );
        BodyPose {
            name: if index == 0 { "Lunan't" } else { "Asterión" },
            center,
            radius,
            model_matrix,
            shader: if index == 0 { star_shader.shade } else { companion_star_shade },
            accessories: Vec::new(),
        }
    }).collect()
}

pub fn orbit_guides() -> Vec<Accessory> {
    ORBIT_MESHES.iter().map(|mesh| Accessory {
        vertices: mesh.as_slice(),
        model_matrix: create_model_matrix(Vector3::zero(), 1.0, Vector3::zero()),
        shade: orbit_guide_shade,
    }).collect()
}

fn orbit_guide_shade(_: &crate::shaders::Frag) -> Vector3 {
    Vector3::new(0.18, 0.22, 0.28)
}

fn companion_star_shade(f: &crate::shaders::Frag) -> Vector3 {
    let violet = (SHADERS[shader_index("estrella")].shade)(f);
    Vector3::new(
        violet.z * 0.30,
        violet.z * 0.76 + violet.x * 0.05,
        violet.x * 0.40 + violet.z * 0.55,
    )
}