use crate::fragment::Fragment;
use crate::triangle::triangle;
use crate::vertex::Vertex;

pub fn rasterize(triangles: &[[Vertex; 3]], width: i32, height: i32) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    for [a, b, c] in triangles {
        triangle(a, b, c, width, height, &mut fragments);
    }
    fragments
}
