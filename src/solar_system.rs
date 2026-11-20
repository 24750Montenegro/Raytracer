use raylib::prelude::*;

use crate::matrix::{create_model_matrix, multiply, rotation_y};
use crate::shaders::{Accessory, SHADERS};
use crate::vertex::Vertex;

pub struct Body {
    pub name: &'static str,
    pub shader_name: &'static str,
    pub orbit_radius: f32,
    pub orbit_speed: f32, // radianes por segundo
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
    Body { name: "Bocchi", shader_name: "planeta rocoso", orbit_radius: 3.2, orbit_speed: 0.52, spin_speed: 0.8, radius: 0.48 },
    Body { name: "JupyterNotebook", shader_name: "gigante gaseoso", orbit_radius: 5.4, orbit_speed: 0.23, spin_speed: 0.55, radius: 0.82 },
    Body { name: "Flora", shader_name: "flora", orbit_radius: 7.4, orbit_speed: 0.15, spin_speed: 0.32, radius: 0.64 },
    Body { name: "Veridia", shader_name: "veridia", orbit_radius: 9.5, orbit_speed: 0.10, spin_speed: 0.24, radius: 0.72 },
];

fn shader_index(name: &str) -> usize {
    SHADERS.iter().position(|shader| shader.name == name)
        .unwrap_or_else(|| panic!("No existe el shader '{name}' en SHADERS"))
}

pub fn planet_poses(time: f32) -> Vec<BodyPose> {
    PLANETS.iter().map(|body| {
        let orbit = body.orbit_speed * time;
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
            model_matrix,
            shader: shader.shade,
            accessories,
        }
    }).collect()
}

pub fn star_pose() -> BodyPose {
    let shader = &SHADERS[shader_index("estrella")];
    BodyPose {
        name: "Lunan't",
        center: Vector3::zero(),
        model_matrix: create_model_matrix(Vector3::zero(), 1.15, Vector3::zero()),
        shader: shader.shade,
        accessories: (shader.accessories)(0.0),
    }
}

pub fn sphere_mesh<'a>(vertices: &'a [Vertex]) -> &'a [Vertex] { vertices }