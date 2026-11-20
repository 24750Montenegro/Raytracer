use raylib::prelude::*;

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
    pub model_matrix: Matrix,
    pub shader: fn(&crate::shaders::Frag) -> Vector3,
    pub accessories: Vec<Accessory>,
}

pub const PLANETS: &[Body] = &[
    Body { name: "Bocchi", shader_name: "planeta rocoso", orbit_radius: 3.1, orbit_speed: 0.48, orbit_phase: 0.35, spin_speed: 0.8, radius: 0.68 },
    Body { name: "JupyterNotebook", shader_name: "gigante gaseoso", orbit_radius: 5.2, orbit_speed: 0.29, orbit_phase: 2.45, spin_speed: 0.55, radius: 0.82 },
    Body { name: "Flora", shader_name: "flora", orbit_radius: 7.3, orbit_speed: 0.21, orbit_phase: 4.25, spin_speed: 0.32, radius: 0.72 },
    Body { name: "Veridia", shader_name: "veridia", orbit_radius: 9.5, orbit_speed: 0.15, orbit_phase: 0.95, spin_speed: 0.24, radius: 0.78 },
    Body { name: "Iris", shader_name: "iris", orbit_radius: 11.6, orbit_speed: 0.11, orbit_phase: 3.25, spin_speed: 0.20, radius: 0.70 },
];

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
        // Los anillos/lunas nacen en coordenadas locales del planeta.
        // Componer con el modelo del planeta los coloca en la órbita correcta.
        for accessory in &mut accessories {
            accessory.model_matrix = multiply(&model_matrix, &accessory.model_matrix);
        }
        BodyPose {
            name: body.name,
            center,
            model_matrix,
            shader: shader.shade,
            accessories,
        }
    }).collect()
}

pub fn star_poses(time: f32) -> Vec<BodyPose> {
    let angle = time * 0.34;
    let (sin, cos) = angle.sin_cos();
    let centers = [
        Vector3::new(0.72 * cos, 0.0, 0.72 * sin),
        Vector3::new(-0.72 * cos, 0.0, -0.72 * sin),
    ];
    let star_shader = &SHADERS[shader_index("estrella")];
    centers.into_iter().enumerate().map(|(index, center)| {
        let radius = if index == 0 { 0.82 } else { 0.68 };
        let spin = if index == 0 { 0.12 } else { -0.16 };
        let model_matrix = create_model_matrix(
            center,
            radius,
            Vector3::new(0.0, time * spin, 0.0),
        );
        BodyPose {
            name: if index == 0 { "Lunan't" } else { "Asterión" },
            center,
            model_matrix,
            shader: if index == 0 { star_shader.shade } else { companion_star_shade },
            accessories: Vec::new(),
        }
    }).collect()
}

fn companion_star_shade(f: &crate::shaders::Frag) -> Vector3 {
    let violet = (SHADERS[shader_index("estrella")].shade)(f);
    Vector3::new(
        violet.z * 0.30,
        violet.z * 0.76 + violet.x * 0.05,
        violet.x * 0.40 + violet.z * 0.55,
    )
}

pub fn sphere_mesh<'a>(vertices: &'a [Vertex]) -> &'a [Vertex] { vertices }