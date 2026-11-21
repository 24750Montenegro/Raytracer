use std::ops::Range;

use crate::triangle::{triangle, Covered};
use crate::vertex::Vertex;

/// Recorre los triangulos y avisa cada pixel que cubren dentro de las filas
/// `rows`.
pub fn rasterize<'a>(triangles: &'a [[Vertex; 3]], width: i32, rows: Range<i32>, mut emit: impl FnMut(Covered<'a>)) {
    for [a, b, c] in triangles {
        triangle(a, b, c, width, rows.clone(), &mut emit);
    }
}
