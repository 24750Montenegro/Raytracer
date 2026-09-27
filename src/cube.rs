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
