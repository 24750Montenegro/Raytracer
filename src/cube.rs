// cube.rs
// Cubo alineado a los ejes: la otra forma que sabe trazar el raytracer.
use raylib::prelude::*;

use crate::material::Material;
use crate::texture::Surface;

/// Lado minimo. Un cubo de lado cero no tiene adentro ni caras que orientar,
/// y ademas partiria por cero al buscar la normal.
const MIN_SIZE: f32 = 1e-4;

/// Se guarda por sus dos esquinas opuestas y no por centro y lado porque el
/// test de interseccion trabaja contra los seis planos de las caras, que son
/// exactamente x = min.x, x = max.x, y asi con los tres ejes.
#[derive(Clone, Copy)]
pub struct Cube {
    pub min: Vector3,
    pub max: Vector3,
    /// Esquina y lado del bloque al que pertenece, que es contra lo que se
    /// miden las coordenadas de textura. En un bloque entero coinciden con
    /// los suyos; en una de las cajitas de un modelo 3D no, y ahi esta la
    /// gracia: una piedra que sobresale medio texel del adoquin tiene que
    /// pintarse con el texel del adoquin que le toca, no con la textura
    /// entera estirada sobre una cajita de medio texel de lado.
    uv_origin: Vector3,
    uv_scale: f32,
    pub material: Material,
}

impl Cube {
    pub fn new(center: Vector3, size: f32, material: Material) -> Self {
        let size = size.max(MIN_SIZE);
        let half = Vector3::new(size * 0.5, size * 0.5, size * 0.5);
        let min = center - half;
        Cube {
            min,
            max: center + half,
            uv_origin: min,
            uv_scale: 1.0 / size,
            material,
        }
    }

    /// Una cajita de un modelo de bloque, en coordenadas locales de 0 a 1.
    ///
    /// La decoracion del pack sobresale un poco del bloque, asi que local
    /// puede salirse de ese rango; lo que no cambia nunca es contra que se
    /// miden las coordenadas de textura, que es el bloque entero.
    pub fn part(
        block_min: Vector3,
        block_size: f32,
        local_min: Vector3,
        local_max: Vector3,
        material: Material,
    ) -> Self {
        let block_size = block_size.max(MIN_SIZE);
        let corner = |local: Vector3| block_min + local * block_size;
        let (min, max) = (corner(local_min), corner(local_max));

        Cube {
            // una cajita de lado cero no tiene caras que orientar
            min,
            max: Vector3::new(
                max.x.max(min.x + MIN_SIZE),
                max.y.max(min.y + MIN_SIZE),
                max.z.max(min.z + MIN_SIZE),
            ),
            uv_origin: block_min,
            uv_scale: 1.0 / block_size,
            material,
        }
    }

    pub fn center(&self) -> Vector3 {
        (self.min + self.max) * 0.5
    }

    /// Distancia al primer impacto, o None si el rayo pasa de largo.
    ///
    /// Metodo de las rebanadas: cada par de caras paralelas deja al rayo
    /// adentro durante un intervalo de t, y el cubo se toca donde los tres
    /// intervalos se pisan. Recibe el inverso de la direccion ya calculado
    /// porque el recorrido del BVH lo saca una sola vez por rayo y aca se
    /// usaria seis veces.
    ///
    /// inv_dir trae infinitos cuando la direccion tiene componentes en cero.
    /// Eso da NaN en los productos, pero f32::min y f32::max devuelven el otro
    /// operando cuando uno es NaN, asi que el eje paralelo al rayo queda
    /// ignorado, que es justo lo correcto.
    #[inline(always)]
    pub fn hit_distance(&self, ray_origin: &Vector3, inv_dir: &Vector3) -> Option<f32> {
        let tx1 = (self.min.x - ray_origin.x) * inv_dir.x;
        let tx2 = (self.max.x - ray_origin.x) * inv_dir.x;
        let mut t_near = tx1.min(tx2);
        let mut t_far = tx1.max(tx2);

        let ty1 = (self.min.y - ray_origin.y) * inv_dir.y;
        let ty2 = (self.max.y - ray_origin.y) * inv_dir.y;
        t_near = t_near.max(ty1.min(ty2));
        t_far = t_far.min(ty1.max(ty2));

        let tz1 = (self.min.z - ray_origin.z) * inv_dir.z;
        let tz2 = (self.max.z - ray_origin.z) * inv_dir.z;
        t_near = t_near.max(tz1.min(tz2));
        t_far = t_far.min(tz1.max(tz2));

        if t_far < t_near.max(0.0) {
            return None;
        }

        // t_near es la cara por la que entra; si quedo detras del origen el
        // rayo arranco adentro del cubo y lo que toca es la cara de salida,
        // que es por donde sale un rayo refractado
        if t_near > 0.0 {
            Some(t_near)
        } else if t_far > 0.0 {
            Some(t_far)
        } else {
            None
        }
    }

    /// Normal unitaria en un punto de la superficie.
    ///
    /// La cara se decide por la componente mas grande del punto medido desde
    /// el centro y en unidades de medio lado: sobre una cara esa componente
    /// vale 1 y las otras dos se quedan cortas. Normalizar asi tolera el error
    /// de redondeo del impacto, que nunca cae exacto sobre el plano.
    #[inline(always)]
    pub fn normal_at(&self, point: Vector3) -> Vector3 {
        let center = self.center();
        let half = (self.max - self.min) * 0.5;

        let dx = (point.x - center.x) / half.x;
        let dy = (point.y - center.y) / half.y;
        let dz = (point.z - center.z) / half.z;

        let (ax, ay, az) = (dx.abs(), dy.abs(), dz.abs());
        if ax >= ay && ax >= az {
            Vector3::new(dx.signum(), 0.0, 0.0)
        } else if ay >= az {
            Vector3::new(0.0, dy.signum(), 0.0)
        } else {
            Vector3::new(0.0, 0.0, dz.signum())
        }
    }

    /// Donde cae un punto sobre la textura, y con que ejes.
    ///
    /// Cada cara lleva la textura entera, asi que un bloque se ve como un
    /// bloque de Minecraft y no como un recorte de un mosaico.
    ///
    /// En las cuatro caras de costado la bitangente va sobre +Y, que aca es
    /// hacia abajo, igual que las filas de la imagen: si no, la textura
    /// saldria acostada y la corteza del tronco correria en horizontal. Los
    /// ejes cumplen tangente x bitangente = normal; al reves, el mapa normal
    /// hundiria el relieve en vez de levantarlo.
    pub fn surface(&self, point: Vector3) -> Surface {
        let normal = self.normal_at(point);
        let down = Vector3::new(0.0, 1.0, 0.0);
        let (tangent, bitangent) = match (normal.x, normal.y, normal.z) {
            (x, _, _) if x > 0.0 => (Vector3::new(0.0, 0.0, -1.0), down),
            (x, _, _) if x < 0.0 => (Vector3::new(0.0, 0.0, 1.0), down),
            (_, _, z) if z > 0.0 => (Vector3::new(1.0, 0.0, 0.0), down),
            (_, _, z) if z < 0.0 => (Vector3::new(-1.0, 0.0, 0.0), down),
            // las tapas no tienen un "arriba" propio: cualquier par sirve
            (_, y, _) if y < 0.0 => (Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 1.0)),
            _ => (Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 0.0, -1.0)),
        };

        // contra el bloque, no contra la cajita: asi la decoracion del
        // modelo 3D sale pintada con los texels que le corresponden
        let local = point - self.uv_origin;

        Surface {
            normal,
            tangent,
            bitangent,
            // los ejes negativos dan coordenadas negativas: rem_euclid las
            // devuelve al rango, con la textura espejada en esa cara
            u: (local.dot(tangent) * self.uv_scale).rem_euclid(1.0),
            v: (local.dot(bitangent) * self.uv_scale).rem_euclid(1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_cube() -> Cube {
        Cube::new(
            Vector3::new(0.0, 0.0, -5.0),
            2.0,
            Material::matte(Color::new(200, 100, 50, 255)),
        )
    }

    fn inverse(direction: Vector3) -> Vector3 {
        Vector3::new(1.0 / direction.x, 1.0 / direction.y, 1.0 / direction.z)
    }

    fn hit(cube: &Cube, origin: Vector3, direction: Vector3) -> Option<f32> {
        cube.hit_distance(&origin, &inverse(direction))
    }

    /// De frente pega en la cara mas cercana, no en la de atras.
    #[test]
    fn frontal_ray_hits_the_near_face() {
        let cube = unit_cube();
        let distance = hit(&cube, Vector3::zero(), Vector3::new(0.0, 0.0, -1.0))
            .expect("tenia que pegar");

        assert!((distance - 4.0).abs() < 1e-4, "{distance}");
    }

    /// Un rayo que pasa por al lado no toca nada, aunque apunte al mismo lado.
    #[test]
    fn ray_beside_the_cube_misses() {
        let cube = unit_cube();
        assert!(hit(&cube, Vector3::new(3.0, 0.0, 0.0), Vector3::new(0.0, 0.0, -1.0)).is_none());
    }

    /// Mirando al lado contrario el cubo queda detras: no cuenta.
    #[test]
    fn cube_behind_the_ray_is_ignored() {
        let cube = unit_cube();
        assert!(hit(&cube, Vector3::zero(), Vector3::new(0.0, 0.0, 1.0)).is_none());
    }

    /// Desde adentro devuelve la cara de salida: es lo que necesita el rayo
    /// que viaja dentro de un cubo de vidrio.
    #[test]
    fn ray_starting_inside_returns_the_exit_face() {
        let cube = unit_cube();
        let distance = hit(&cube, Vector3::new(0.0, 0.0, -5.0), Vector3::new(0.0, 0.0, -1.0))
            .expect("tenia que salir por la cara de atras");

        assert!((distance - 1.0).abs() < 1e-4, "{distance}");
    }

    /// El rayo de canto contra una cara paralela no tiene que colgarse en el
    /// NaN de 0 por infinito.
    #[test]
    fn rays_parallel_to_a_face_behave() {
        let cube = unit_cube();

        // viaja en X justo a la altura del cubo: lo cruza
        assert!(hit(&cube, Vector3::new(-9.0, 0.0, -5.0), Vector3::new(1.0, 0.0, 0.0)).is_some());
        // misma direccion pero por encima: no lo toca
        assert!(hit(&cube, Vector3::new(-9.0, 4.0, -5.0), Vector3::new(1.0, 0.0, 0.0)).is_none());
    }

    /// Cada cara devuelve su normal, y siempre apuntando hacia afuera.
    #[test]
    fn each_face_has_its_own_outward_normal() {
        let cube = unit_cube();
        let center = cube.center();

        let faces = [
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(-1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(0.0, 0.0, -1.0),
        ];

        for expected in faces {
            // el medio de esa cara: queda a un medio lado del centro
            let point = center + expected;
            let normal = cube.normal_at(point);

            assert!(
                (normal - expected).length() < 1e-5,
                "cara {expected:?} devolvio {normal:?}"
            );
        }
    }

    /// Los ejes de la textura tienen que ser perpendiculares entre si y a la
    /// cara, y en ese orden: si se dieran vuelta, el mapa normal hundiria el
    /// relieve en vez de levantarlo.
    #[test]
    fn every_face_has_a_right_handed_texture_frame() {
        let cube = unit_cube();
        let center = cube.center();

        for offset in [
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(-1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(0.0, 0.0, -1.0),
        ] {
            let surface = cube.surface(center + offset);

            assert!(surface.tangent.dot(surface.bitangent).abs() < 1e-5);
            assert!(surface.tangent.dot(surface.normal).abs() < 1e-5);
            assert!(
                (surface.tangent.cross(surface.bitangent) - surface.normal).length() < 1e-5,
                "la cara {offset:?} quedo con los ejes al reves"
            );
        }
    }

    /// Cada cara lleva la textura entera: las coordenadas barren de 0 a 1 de
    /// un borde al otro del bloque.
    #[test]
    fn the_texture_spans_the_whole_face() {
        let cube = unit_cube();
        let center = cube.center();

        // el medio de la cara de arriba cae en el medio de la textura
        let middle = cube.surface(center + Vector3::new(0.0, -1.0, 0.0));
        assert!((middle.u - 0.5).abs() < 1e-5, "u = {}", middle.u);
        assert!((middle.v - 0.5).abs() < 1e-5, "v = {}", middle.v);

        // y las esquinas, en los extremos
        let corner = cube.surface(cube.min + Vector3::new(0.0, 0.0, 0.0));
        assert!((0.0..=1.0).contains(&corner.u) && (0.0..=1.0).contains(&corner.v));

        for point in [
            center + Vector3::new(0.9, 0.5, -0.3),
            center + Vector3::new(-0.2, 0.7, 1.0),
            center + Vector3::new(0.4, -1.0, 0.8),
        ] {
            let surface = cube.surface(point);
            assert!((0.0..=1.0).contains(&surface.u), "u fuera de rango");
            assert!((0.0..=1.0).contains(&surface.v), "v fuera de rango");
        }
    }

    /// En las cuatro caras de costado la textura tiene que quedar parada:
    /// bajar por la cara es avanzar en v. Si se acostara, la corteza del
    /// tronco correria en horizontal en dos de las cuatro.
    #[test]
    fn the_texture_stands_upright_on_every_side() {
        let cube = unit_cube();
        let center = cube.center();

        for side in [
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(-1.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(0.0, 0.0, -1.0),
        ] {
            let surface = cube.surface(center + side);
            assert_eq!(
                surface.bitangent,
                Vector3::new(0.0, 1.0, 0.0),
                "la cara {side:?} quedo acostada"
            );

            // -Y es arriba: mas alto en la cara es menos v
            let high = cube.surface(center + side + Vector3::new(0.0, -0.4, 0.0));
            let low = cube.surface(center + side + Vector3::new(0.0, 0.4, 0.0));
            assert!(high.v < low.v, "la cara {side:?} quedo dada vuelta");
        }
    }

    /// Dos bloques pegados no comparten el dibujo: cada uno arranca la
    /// textura de cero, como pasa en Minecraft.
    #[test]
    fn each_block_starts_its_own_texture() {
        let material = Material::matte(Color::new(200, 100, 50, 255));
        let left = Cube::new(Vector3::new(0.0, 0.0, 0.0), 1.0, material);
        let right = Cube::new(Vector3::new(1.0, 0.0, 0.0), 1.0, material);

        // el mismo punto del mundo, visto desde la cara de arriba de cada uno
        let a = left.surface(Vector3::new(0.25, -0.5, 0.0));
        let b = right.surface(Vector3::new(1.25, -0.5, 0.0));

        assert!((a.u - b.u).abs() < 1e-5, "{} contra {}", a.u, b.u);
    }

    /// La normal sigue siendo la de la cara aunque el impacto caiga un pelo
    /// adentro o afuera del plano, que es lo que pasa en la practica.
    #[test]
    fn normal_survives_rounding_off_the_face() {
        let cube = unit_cube();
        let point = cube.center() + Vector3::new(0.9999, 0.4, -0.3);

        assert_eq!(cube.normal_at(point).x, 1.0);
    }
}
