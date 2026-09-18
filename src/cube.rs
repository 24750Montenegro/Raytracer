// cube.rs
// Cubo alineado a los ejes: la otra forma que sabe trazar el raytracer.
use raylib::prelude::*;

use crate::material::Material;

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
    pub material: Material,
}

impl Cube {
    pub fn new(center: Vector3, size: f32, material: Material) -> Self {
        let half = size.max(MIN_SIZE) * 0.5;
        let half = Vector3::new(half, half, half);
        Cube {
            min: center - half,
            max: center + half,
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

    /// La normal sigue siendo la de la cara aunque el impacto caiga un pelo
    /// adentro o afuera del plano, que es lo que pasa en la practica.
    #[test]
    fn normal_survives_rounding_off_the_face() {
        let cube = unit_cube();
        let point = cube.center() + Vector3::new(0.9999, 0.4, -0.3);

        assert_eq!(cube.normal_at(point).x, 1.0);
    }
}
