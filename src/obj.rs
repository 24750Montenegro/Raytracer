use raylib::prelude::*;

use crate::vertex::Vertex;

#[derive(Clone, Copy)]

struct Corner{
    position: usize,
    tex_coords: Option<usize>,
    normal: Option<usize>,
}

pub struct Obj {
    positions: Vec<Vector3>,
    normals: Vec<Vector3>,
    tex_coords: Vec<Vector2>,
    triangles: Vec<[Corner; 3]>,
}

impl Obj {
    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;

        let mut obj = Obj {
            positions: Vec::new(),
            normals: Vec::new(),
            tex_coords: Vec::new(),
            triangles: Vec::new(),
        };

        for (index, line) in text.lines().enumerate() {
            let number = index + 1;
            let mut parts = line.split_whitespace();

            match parts.next() {
                Some("v") => {
                    let [x, y, z] = parse_numbers(&mut parts, number)?;
                    obj.positions.push(Vector3::new(x, y, z));
                }
                Some("vn") => {
                    let [x, y, z] = parse_numbers(&mut parts, number)?;
                    obj.normals.push(Vector3::new(x, y, z));
                }
                Some("vt") => {
                    let [u, v] = parse_numbers(&mut parts, number)?;
                    obj.tex_coords.push(Vector2::new(u, v));
                }
                Some("f") => {
                    let corners = parts.map(|text| obj.parse_corner(text, number)).collect::<Result<Vec<_>, _>>()?;

                    if corners.len() < 3 {
                        return Err(format!("linea {number}: se necesitan al menos 3 vertices para formar un triangulo"));
                    }

                    for i in 1..corners.len() - 1 {
                        obj.triangles.push([corners[0], corners[i], corners[i + 1]]);
                    }
                }

                _ => {} 
            }
        }

        Ok(obj)
    }

    pub fn get_vertex_array(&self) -> Vec<Vertex> {
        let mut vertices = Vec::with_capacity(self.triangles.len() * 3);

        for triangle in &self.triangles {
            for corner in triangle {
                let position = self.positions[corner.position];
                let normal = corner.normal.map_or(Vector3::zero(), |i| self.normals[i]);
                let tex_coords = corner.tex_coords.map_or(Vector2::new(0.0, 0.0), |i| self.tex_coords[i]);
                vertices.push(Vertex::new(position, normal, tex_coords));
            }
        }
        vertices
    }


    fn parse_corner(&self, text: &str, number: usize) -> Result <Corner, String> {
        let mut fields = text.split('/');

        let position = resolve(fields.next(), self.positions.len(), number)?.ok_or_else(|| format!("linea {number}: falta la posición en '{text}'"))?;
        let tex_coords = resolve(fields.next(), self.tex_coords.len(), number)?;
        let normal = resolve(fields.next(), self.normals.len(), number)?;

        Ok(Corner {
            position,
            tex_coords,
            normal,
        })  
    }
}

fn resolve(field: Option<&str>, count: usize, number: usize) -> Result<Option<usize>, String> {
    let Some(field) = field.filter(|field| !field.is_empty()) else {
        return Ok(None);
    };

    let index: i64 = field.parse().map_err(|_| format!("linea {number}: '{field}'"))?;
    let resolved = if index > 0 { index -1 } else { count as i64 + index};

    if index == 0 || resolved < 0 || resolved >= count as i64 {
        return Err(format!("linea {number}: el indice {index} no existe"));
    }
    Ok(Some(resolved as usize))
}


fn parse_numbers<const N: usize>(
    parts: &mut std::str::SplitWhitespace,
    number: usize,
) -> Result<[f32; N], String> {
    let mut values = [0.0; N];

    for value in &mut values {
        let text = parts.next().ok_or_else(|| format!("linea {number}: faltan valores"))?;
        *value = text.parse().map_err(|_| format!("linea {number}: '{text}' no es un numero"))?;

    }

    Ok(values)
}