// Casco de las naves: metal claro laqueado, con un filo azul en el borde para
// que la silueta se lea contra el fondo negro aunque le de la espalda al sol.
use super::kit::*;

const HULL: Vector3 = Vector3::new(0.74, 0.76, 0.82);
const RIM: Vector3 = Vector3::new(0.25, 0.48, 0.95);
/// Luz de relleno que sale de la camara: sin ella la cara que no ve al sol
/// queda negra, y en vuelo esa es justo la que se tiene enfrente.
const FILL: f32 = 0.22;

pub fn shade(f: &Frag) -> Vector3 {
    let fill = f.normal.dot(f.to_viewer).max(0.0) * FILL;
    f.shade(HULL, &Material::lacquer(), f.normal) + HULL * fill + RIM * (rim(f, 3.0) * 0.55)
}
