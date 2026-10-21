use crate::fragment::Fragment;
use crate::triangle::triangle;
use crate::vertex::Vertex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriangleMode {
    /// Solo los tres bordes.
    Wireframe,
}

impl TriangleMode {
    pub fn name(self) -> &'static str {
        match self {
            TriangleMode::Wireframe => "wireframe",
        }
    }
}

pub fn rasterize(triangles: &[[Vertex; 3]], mode: TriangleMode) -> Vec<Fragment> {
    let mut fragments = Vec::new();

    for tri in triangles {
        let generated = match mode {
            TriangleMode::Wireframe => triangle(&tri[0], &tri[1], &tri[2]),
        };
        fragments.extend(generated);
    }
    fragments
}
