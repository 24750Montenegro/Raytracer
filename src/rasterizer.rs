use crate::fragment::Fragment;
use crate::triangle::{triangle, triangle_barycentric, triangle_scanline};
use crate::vertex::Vertex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriangleMode {
    /// Solo los tres bordes.
    Wireframe,
    /// Relleno probando cada pixel del rectangulo que lo contiene.
    FilledBarycentric,
    /// Relleno fila por fila.
    FilledScanline,
}

impl TriangleMode {
    pub fn name(self) -> &'static str {
        match self {
            TriangleMode::Wireframe => "wireframe",
            TriangleMode::FilledBarycentric => "relleno baricentrico",
            TriangleMode::FilledScanline => "relleno por lineas",
        }
    }

    pub fn is_filled(self) -> bool {
        self != TriangleMode::Wireframe
    }
}

pub fn rasterize(triangles: &[[Vertex; 3]], mode: TriangleMode) -> Vec<Fragment> {
    let mut fragments = Vec::new();

    for tri in triangles {
        let generated = match mode {
            TriangleMode::Wireframe => triangle(&tri[0], &tri[1], &tri[2]),
            TriangleMode::FilledBarycentric => triangle_barycentric(&tri[0], &tri[1], &tri[2]),
            TriangleMode::FilledScanline => triangle_scanline(&tri[0], &tri[1], &tri[2]),
        };
        fragments.extend(generated);
    }
    fragments
}
