///Bresenham

use crate::fragment::Fragment;
use crate::vertex::Vertex;

pub fn line(a: &Vertex, b: &Vertex) -> Vec<Fragment> {
    let mut fragments = Vec::new();

    let start = a.transformed_position;
    let end = b.transformed_position;

    let (mut x, mut y) = (start.x as i32, start.y as i32);
    let (x_end, y_end) = (end.x as i32, end.y as i32);

    let dx = (x_end - x).abs();
    let dy = -(y_end - y).abs();
    let step_x = if x < x_end { 1 } else { -1 }; 
    let step_y = if y < y_end { 1 } else { -1 };
    let mut error = dx + dy;

    //profundidad por pareja
    let steps = dx.max(-dy).max(1) as f32;
    let mut step = 0;

    loop{
        let depth = start.z + (end.z - start.z) * (step as f32 / steps);
        fragments.push(Fragment::new(x as f32, y as f32, a.color, depth));

        if x == x_end && y == y_end {
            break;
        }

        let doubled = 2 * error;
        if doubled >= dy {
            error += dy;
            x += step_x;
        }
        if doubled <= dx {
            error += dx;
            y += step_y;
        }
        step += 1;
    }
    fragments
}