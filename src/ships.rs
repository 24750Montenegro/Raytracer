// ships.rs
// Las naves que se pueden elegir: cada OBJ terminado en "-reduced" de
// assets/models. Se usan las versiones reducidas porque todo se dibuja en
// CPU y la nave esta en pantalla todo el tiempo.
use raylib::prelude::*;
use std::path::Path;

use crate::fit_unit_sphere;
use crate::obj::Obj;
use crate::vertex::Vertex;

const MODELS_DIR: &str = "assets/models";
const REDUCED_SUFFIX: &str = "-reduced.obj";

pub struct Ship {
    pub name: String,
    pub vertices: Vec<Vertex>,
    /// Lleva la malla a una esfera de radio 1 centrada en el origen, con la
    /// nariz hacia -z (hacia donde mira la camara).
    pub fit: Matrix,
}

impl Ship {
    pub fn triangles(&self) -> usize {
        self.vertices.len() / 3
    }
}

/// Todas las naves reducidas, ordenadas por nombre de archivo. Las que no se
/// pueden leer se avisan por consola y se saltan.
pub fn load_reduced() -> Vec<Ship> {
    let entries = match std::fs::read_dir(MODELS_DIR) {
        Ok(entries) => entries,
        Err(problem) => {
            eprintln!("{MODELS_DIR}: {problem}");
            return Vec::new();
        }
    };

    let mut paths: Vec<_> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| file_name(path).ends_with(REDUCED_SUFFIX))
        .collect();
    paths.sort();

    paths.iter().filter_map(|path| load(path)).collect()
}

fn load(path: &Path) -> Option<Ship> {
    let obj = Obj::load(&path.to_string_lossy())
        .map_err(|problem| eprintln!("no se pudo cargar la nave: {problem}"))
        .ok()?;
    let mut vertices = obj.get_vertex_array();
    fill_missing_normals(&mut vertices);

    Some(Ship {
        name: file_name(path).trim_end_matches(REDUCED_SUFFIX).to_string(),
        // Blender ya exporta con el frente del modelo hacia -z
        fit: fit_unit_sphere(&vertices),
        vertices,
    })
}

fn file_name(path: &Path) -> &str {
    path.file_name().and_then(|name| name.to_str()).unwrap_or_default()
}

/// Las esquinas que vienen sin `vn` en el OBJ se quedan con la normal de su
/// triangulo; sin esto el fragment shader normalizaria un vector cero.
fn fill_missing_normals(vertices: &mut [Vertex]) {
    for triangle in vertices.chunks_exact_mut(3) {
        let [a, b, c] = [&triangle[0], &triangle[1], &triangle[2]].map(|vertex| vertex.position);
        let face = (b - a).cross(c - a).normalized();
        for vertex in triangle {
            if vertex.normal.dot(vertex.normal) < 1e-12 {
                vertex.normal = face;
            }
        }
    }
}
