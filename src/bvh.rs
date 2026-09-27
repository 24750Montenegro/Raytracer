// bvh.rs
// Arbol de volumenes envolventes (BVH) sobre los cuerpos de la escena.
//
// Sin esto cada rayo prueba TODOS los cuerpos: 1000x1000 pixeles por los mas
// de mil bloques del terreno son miles de millones de tests por frame, y con
// la camara moviendose hay que rehacerlos en cada cuadro. El BVH deja cada
// rayo en unos pocos tests. Como la escena es estatica, se construye una sola
// vez: orbitar la camara no lo invalida.
use raylib::prelude::*;

use crate::object::Object;

/// Maximo de cuerpos por hoja. Con pocos ya no compensa seguir partiendo:
/// probarlos de corrido es mas barato que bajar otro nivel.
const MAX_LEAF_SIZE: usize = 4;
/// Profundidad maxima soportada por la pila de recorrido.
const MAX_STACK: usize = 64;

#[derive(Clone, Copy)]
pub struct Aabb {
    pub min: Vector3,
    pub max: Vector3,
}

impl Aabb {
    fn empty() -> Self {
        Aabb {
            min: Vector3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
            max: Vector3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
        }
    }

    fn from_object(object: &Object) -> Self {
        let (min, max) = object.bounds();
        Aabb { min, max }
    }

    fn grow(&mut self, other: &Aabb) {
        self.min.x = self.min.x.min(other.min.x);
        self.min.y = self.min.y.min(other.min.y);
        self.min.z = self.min.z.min(other.min.z);
        self.max.x = self.max.x.max(other.max.x);
        self.max.y = self.max.y.max(other.max.y);
        self.max.z = self.max.z.max(other.max.z);
    }

    fn grow_point(&mut self, point: Vector3) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.min.z = self.min.z.min(point.z);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
        self.max.z = self.max.z.max(point.z);
    }

    /// Eje en el que la caja es mas larga: por ahi conviene partir.
    fn largest_axis(&self) -> usize {
        let size = self.max - self.min;
        if size.x >= size.y && size.x >= size.z {
            0
        } else if size.y >= size.z {
            1
        } else {
            2
        }
    }

    /// Test de rebanadas (slabs). Devuelve la distancia de entrada si el rayo
    /// toca la caja antes de t_max.
    ///
    /// inv_dir puede traer infinitos cuando la direccion tiene componentes en
    /// cero; f32::min y f32::max descartan los NaN que salen de 0 por inf, que
    /// es justo el comportamiento que se quiere aca.
    #[inline(always)]
    fn hit(&self, origin: &Vector3, inv_dir: &Vector3, t_max: f32) -> bool {
        let tx1 = (self.min.x - origin.x) * inv_dir.x;
        let tx2 = (self.max.x - origin.x) * inv_dir.x;
        let mut t_near = tx1.min(tx2);
        let mut t_far = tx1.max(tx2);

        let ty1 = (self.min.y - origin.y) * inv_dir.y;
        let ty2 = (self.max.y - origin.y) * inv_dir.y;
        t_near = t_near.max(ty1.min(ty2));
        t_far = t_far.min(ty1.max(ty2));

        let tz1 = (self.min.z - origin.z) * inv_dir.z;
        let tz2 = (self.max.z - origin.z) * inv_dir.z;
        t_near = t_near.max(tz1.min(tz2));
        t_far = t_far.min(tz1.max(tz2));

        t_far >= t_near.max(0.0) && t_near < t_max
    }
}

/// Nodo aplanado. count == 0 marca nodo interno: el hijo izquierdo es el
/// siguiente del vector y payload apunta al derecho. En las hojas payload es
/// el primer cuerpo y count cuantos van seguidos.
#[derive(Clone, Copy)]
struct Node {
    bounds: Aabb,
    payload: u32,
    count: u32,
}

pub struct Bvh {
    nodes: Vec<Node>,
}

impl Bvh {
    /// Construye el arbol y reordena los cuerpos para que cada hoja quede en
    /// posiciones contiguas, asi el recorrido lee memoria seguida.
    pub fn build(objects: &mut Vec<Object>) -> Self {
        let mut bvh = Bvh { nodes: Vec::new() };
        if objects.is_empty() {
            return bvh;
        }

        let mut order: Vec<u32> = (0..objects.len() as u32).collect();
        bvh.nodes.reserve(2 * objects.len());
        bvh.build_node(objects, &mut order, 0, objects.len());

        let sorted: Vec<Object> = order.iter().map(|&i| objects[i as usize]).collect();
        *objects = sorted;
        bvh
    }

    /// Arma el nodo que cubre order[start..end] y devuelve su indice.
    fn build_node(
        &mut self,
        objects: &[Object],
        order: &mut [u32],
        start: usize,
        end: usize,
    ) -> u32 {
        let mut bounds = Aabb::empty();
        let mut centroids = Aabb::empty();
        for &index in &order[start..end] {
            let object = &objects[index as usize];
            bounds.grow(&Aabb::from_object(object));
            centroids.grow_point(object.center());
        }

        let node_index = self.nodes.len() as u32;
        self.nodes.push(Node {
            bounds,
            payload: start as u32,
            count: (end - start) as u32,
        });

        if end - start <= MAX_LEAF_SIZE {
            return node_index;
        }

        // Particion por el punto medio del eje mas largo. Es mucho mas barata
        // que una SAH completa y para esta escena reparte igual de bien.
        let axis = centroids.largest_axis();
        let split = (axis_value(centroids.min, axis) + axis_value(centroids.max, axis)) * 0.5;

        let mut mid = partition(objects, order, start, end, axis, split);
        // cuerpos apilados en el mismo centro: partir a la mitad y seguir
        if mid == start || mid == end {
            mid = (start + end) / 2;
        }

        self.build_node(objects, order, start, mid);
        let right = self.build_node(objects, order, mid, end);

        self.nodes[node_index as usize].count = 0; // pasa a ser nodo interno
        self.nodes[node_index as usize].payload = right;
        node_index
    }

    /// Cuerpo mas cercano que toca el rayo, con la distancia al impacto.
    /// El indice es sobre la lista ya reordenada por build. Devolver la
    /// distancia aca evita repetir el test del cuerpo ganador al sombrear.
    pub fn nearest_hit(
        &self,
        origin: &Vector3,
        direction: &Vector3,
        objects: &[Object],
    ) -> Option<(usize, f32)> {
        if self.nodes.is_empty() {
            return None;
        }

        let inv_dir = Vector3::new(1.0 / direction.x, 1.0 / direction.y, 1.0 / direction.z);
        let mut best_distance = f32::INFINITY;
        let mut best: Option<(usize, f32)> = None;

        let mut stack = [0u32; MAX_STACK];
        let mut depth = 0usize;
        let mut current = 0u32;

        loop {
            let node = &self.nodes[current as usize];

            if node.bounds.hit(origin, &inv_dir, best_distance) {
                if node.count > 0 {
                    let first = node.payload as usize;
                    let last = first + node.count as usize;
                    for (offset, object) in objects[first..last].iter().enumerate() {
                        if let Some(distance) = object.hit_distance(origin, &inv_dir) {
                            if distance < best_distance {
                                best_distance = distance;
                                best = Some((first + offset, distance));
                            }
                        }
                    }
                } else if depth < MAX_STACK {
                    // el izquierdo es el siguiente nodo, el derecho queda pendiente
                    stack[depth] = node.payload;
                    depth += 1;
                    current += 1;
                    continue;
                }
            }

            if depth == 0 {
                return best;
            }
            depth -= 1;
            current = stack[depth];
        }
    }

    /// Hay algo que tape la luz entre origin y una distancia dada? A
    /// diferencia de nearest_hit alcanza con encontrar uno, asi que corta
    /// apenas lo ve. Es lo que necesitan los rayos de sombra.
    ///
    /// Los cuerpos que dejan pasar la luz no cuentan como tapa: si contaran,
    /// el agua se haria sombra a si misma y las lagunas saldrian negras. Lo
    /// correcto seria atenuar y tintar la sombra; esto es la version barata.
    pub fn any_hit(
        &self,
        origin: &Vector3,
        direction: &Vector3,
        max_distance: f32,
        objects: &[Object],
    ) -> bool {
        if self.nodes.is_empty() {
            return false;
        }

        let inv_dir = Vector3::new(1.0 / direction.x, 1.0 / direction.y, 1.0 / direction.z);

        let mut stack = [0u32; MAX_STACK];
        let mut depth = 0usize;
        let mut current = 0u32;

        loop {
            let node = &self.nodes[current as usize];

            if node.bounds.hit(origin, &inv_dir, max_distance) {
                if node.count > 0 {
                    let first = node.payload as usize;
                    let last = first + node.count as usize;
                    for object in &objects[first..last] {
                        if object.material().albedo.refraction > 0.0 {
                            continue;
                        }
                        if let Some(distance) = object.hit_distance(origin, &inv_dir) {
                            if distance < max_distance {
                                return true;
                            }
                        }
                    }
                } else if depth < MAX_STACK {
                    stack[depth] = node.payload;
                    depth += 1;
                    current += 1;
                    continue;
                }
            }

            if depth == 0 {
                return false;
            }
            depth -= 1;
            current = stack[depth];
        }
    }
}

#[inline(always)]
fn axis_value(v: Vector3, axis: usize) -> f32 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// Deja a la izquierda los cuerpos con centro por debajo de split y devuelve
/// donde quedo el corte.
fn partition(
    objects: &[Object],
    order: &mut [u32],
    start: usize,
    end: usize,
    axis: usize,
    split: f32,
) -> usize {
    let mut mid = start;
    for i in start..end {
        let center = objects[order[i] as usize].center();
        if axis_value(center, axis) < split {
            order.swap(i, mid);
            mid += 1;
        }
    }
    mid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::Material;

    /// Generador congruencial minimo: escenas reproducibles sin dependencias.
    struct Lcg(u32);

    impl Lcg {
        fn unit(&mut self) -> f32 {
            self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
            (self.0 >> 8) as f32 / (1u32 << 24) as f32
        }

        fn range(&mut self, low: f32, high: f32) -> f32 {
            low + (high - low) * self.unit()
        }
    }

    /// Cuerpos sueltos y de tamanos distintos: al BVH le tiene que dar igual
    /// como estan repartidos.
    fn random_scene(count: usize, seed: u32) -> Vec<Object> {
        let mut rng = Lcg(seed);
        (0..count)
            .map(|_| {
                let center = Vector3::new(
                    rng.range(-5.0, 5.0),
                    rng.range(-5.0, 5.0),
                    rng.range(-5.0, 5.0),
                );
                Object::cube(
                    center,
                    rng.range(0.1, 1.2),
                    Material::matte(Color::new(10, 20, 30, 255)),
                )
            })
            .collect()
    }

    /// La version lenta: probar todos los cuerpos. Es la referencia.
    fn brute_force(objects: &[Object], origin: &Vector3, direction: &Vector3) -> Option<f32> {
        let inv_dir = Vector3::new(1.0 / direction.x, 1.0 / direction.y, 1.0 / direction.z);
        objects
            .iter()
            .filter_map(|o| o.hit_distance(origin, &inv_dir))
            .fold(None, |best: Option<f32>, d| Some(best.map_or(d, |b| b.min(d))))
    }

    fn bvh_distance(
        bvh: &Bvh,
        objects: &[Object],
        origin: &Vector3,
        direction: &Vector3,
    ) -> Option<f32> {
        bvh.nearest_hit(origin, direction, objects).map(|(_, d)| d)
    }

    #[test]
    fn matches_brute_force_on_random_rays() {
        let mut objects = random_scene(200, 7);
        let bvh = Bvh::build(&mut objects);

        let mut rng = Lcg(1234);
        for _ in 0..5000 {
            let origin = Vector3::new(
                rng.range(-12.0, 12.0),
                rng.range(-12.0, 12.0),
                rng.range(-12.0, 12.0),
            );
            let direction = Vector3::new(
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
            );
            if direction.length() < 0.1 {
                continue;
            }
            let direction = direction.normalized();

            match (
                brute_force(&objects, &origin, &direction),
                bvh_distance(&bvh, &objects, &origin, &direction),
            ) {
                (None, None) => {}
                (Some(expected), Some(got)) => {
                    assert!((expected - got).abs() < 1e-3, "{expected} vs {got}");
                }
                (expected, got) => panic!("el BVH no coincide: {expected:?} vs {got:?}"),
            }
        }
    }

    /// Rayos alineados a un eje: inv_dir queda con infinitos y el test de
    /// cajas tiene que seguir funcionando.
    #[test]
    fn axis_aligned_rays_still_hit() {
        let mut objects = vec![
            Object::cube(
                Vector3::new(0.0, 0.0, -5.0),
                2.0,
                Material::matte(Color::new(1, 2, 3, 255)),
            ),
            Object::cube(
                Vector3::new(4.0, 0.0, -5.0),
                2.0,
                Material::matte(Color::new(1, 2, 3, 255)),
            ),
        ];
        let bvh = Bvh::build(&mut objects);

        let origin = Vector3::zero();
        let direction = Vector3::new(0.0, 0.0, -1.0);
        let hit = bvh_distance(&bvh, &objects, &origin, &direction).expect("tenia que pegar");
        assert!((hit - 4.0).abs() < 1e-4);

        // de canto: el rayo viaja en X justo por la altura del segundo cubo y
        // le pega en la cara de x = 5 antes de llegar al primero
        let sideways = bvh_distance(
            &bvh,
            &objects,
            &Vector3::new(9.0, 0.0, -5.0),
            &Vector3::new(-1.0, 0.0, 0.0),
        )
        .expect("tenia que pegar en el cubo");
        assert!((sideways - 4.0).abs() < 1e-4, "{sideways}");

        // mirando al otro lado no hay nada
        assert!(bvh_distance(&bvh, &objects, &origin, &Vector3::new(0.0, 0.0, 1.0)).is_none());
    }

    #[test]
    fn empty_scene_never_hits() {
        let mut objects: Vec<Object> = Vec::new();
        let bvh = Bvh::build(&mut objects);
        assert!(bvh
            .nearest_hit(&Vector3::zero(), &Vector3::new(0.0, 0.0, -1.0), &objects)
            .is_none());
    }

    /// Cuerpos todos en el mismo centro: la particion por punto medio no
    /// separa nada y tiene que caer en el corte a la mitad sin colgarse.
    #[test]
    fn degenerate_split_terminates() {
        let mut objects: Vec<Object> = (0..32)
            .map(|i| {
                Object::cube(
                    Vector3::new(0.0, 0.0, -5.0),
                    0.2 + i as f32 * 0.02,
                    Material::matte(Color::new(0, 0, 0, 255)),
                )
            })
            .collect();
        let bvh = Bvh::build(&mut objects);

        let hit = bvh_distance(&bvh, &objects, &Vector3::zero(), &Vector3::new(0.0, 0.0, -1.0));
        assert!(hit.is_some());
    }

    /// any_hit tiene que coincidir con revisar todos los cuerpos a mano.
    #[test]
    fn any_hit_matches_brute_force() {
        let mut objects = random_scene(120, 21);
        let bvh = Bvh::build(&mut objects);

        let mut rng = Lcg(555);
        for _ in 0..2000 {
            let origin = Vector3::new(
                rng.range(-10.0, 10.0),
                rng.range(-10.0, 10.0),
                rng.range(-10.0, 10.0),
            );
            let direction = Vector3::new(
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
            );
            if direction.length() < 0.1 {
                continue;
            }
            let direction = direction.normalized();
            let max_distance = rng.range(0.5, 20.0);

            let expected =
                brute_force(&objects, &origin, &direction).is_some_and(|d| d < max_distance);

            assert_eq!(
                expected,
                bvh.any_hit(&origin, &direction, max_distance, &objects)
            );
        }
    }

    /// Si el agua contara como tapa, el fondo de la laguna quedaria negro.
    #[test]
    fn transparent_bodies_do_not_block_the_light() {
        let tint = Color::new(200, 220, 255, 255);
        let origin = Vector3::zero();
        let direction = Vector3::new(0.0, 0.0, -1.0);

        let mut clear = vec![Object::cube(
            Vector3::new(0.0, 0.0, -5.0),
            2.0,
            Material::water(tint),
        )];
        let bvh = Bvh::build(&mut clear);
        assert!(!bvh.any_hit(&origin, &direction, 10.0, &clear));

        // el mismo cubo, opaco, si tapa
        let mut solid = vec![Object::cube(
            Vector3::new(0.0, 0.0, -5.0),
            2.0,
            Material::matte(tint),
        )];
        let bvh = Bvh::build(&mut solid);
        assert!(bvh.any_hit(&origin, &direction, 10.0, &solid));
    }

    /// Lo que queda mas lejos que el limite no tapa nada.
    #[test]
    fn any_hit_ignores_blockers_past_the_limit() {
        let mut objects = vec![Object::cube(
            Vector3::new(0.0, 0.0, -5.0),
            2.0,
            Material::matte(Color::new(1, 2, 3, 255)),
        )];
        let bvh = Bvh::build(&mut objects);

        let origin = Vector3::zero();
        let direction = Vector3::new(0.0, 0.0, -1.0);

        assert!(bvh.any_hit(&origin, &direction, 10.0, &objects));
        assert!(!bvh.any_hit(&origin, &direction, 3.0, &objects));
    }
}
