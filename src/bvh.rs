// bvh.rs
// Arbol de volumenes envolventes (BVH) sobre las esferas de la escena.
//
// Sin esto cada rayo prueba TODAS las esferas: 1000x1000 pixeles por 173
// esferas son ~173 millones de tests por frame, y con la camara moviendose
// hay que rehacerlos en cada cuadro. El BVH deja cada rayo en unos pocos
// tests. Como la escena es estatica, se construye una sola vez: orbitar la
// camara no lo invalida.
use raylib::prelude::*;

use crate::sphere::Sphere;

/// Maximo de esferas por hoja. Con pocas esferas ya no compensa seguir
/// partiendo: probarlas de corrido es mas barato que bajar otro nivel.
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

    fn from_sphere(sphere: &Sphere) -> Self {
        let r = Vector3::new(sphere.radius, sphere.radius, sphere.radius);
        Aabb {
            min: sphere.center - r,
            max: sphere.center + r,
        }
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
/// la primera esfera y count cuantas van seguidas.
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
    /// Construye el arbol y reordena las esferas para que cada hoja quede en
    /// posiciones contiguas, asi el recorrido lee memoria seguida.
    pub fn build(objects: &mut Vec<Sphere>) -> Self {
        let mut bvh = Bvh { nodes: Vec::new() };
        if objects.is_empty() {
            return bvh;
        }

        let mut order: Vec<u32> = (0..objects.len() as u32).collect();
        bvh.nodes.reserve(2 * objects.len());
        bvh.build_node(objects, &mut order, 0, objects.len());

        let sorted: Vec<Sphere> = order.iter().map(|&i| objects[i as usize]).collect();
        *objects = sorted;
        bvh
    }

    /// Arma el nodo que cubre order[start..end] y devuelve su indice.
    fn build_node(
        &mut self,
        objects: &[Sphere],
        order: &mut [u32],
        start: usize,
        end: usize,
    ) -> u32 {
        let mut bounds = Aabb::empty();
        let mut centroids = Aabb::empty();
        for &index in &order[start..end] {
            let sphere = &objects[index as usize];
            bounds.grow(&Aabb::from_sphere(sphere));
            centroids.grow_point(sphere.center);
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
        // esferas apiladas en el mismo centro: partir a la mitad y seguir
        if mid == start || mid == end {
            mid = (start + end) / 2;
        }

        self.build_node(objects, order, start, mid);
        let right = self.build_node(objects, order, mid, end);

        self.nodes[node_index as usize].count = 0; // pasa a ser nodo interno
        self.nodes[node_index as usize].payload = right;
        node_index
    }

    /// Esfera mas cercana que toca el rayo, con la distancia al impacto.
    /// El indice es sobre la lista ya reordenada por build. Devolver la
    /// distancia aca evita repetir el test de la esfera ganadora al sombrear.
    pub fn nearest_hit(
        &self,
        origin: &Vector3,
        direction: &Vector3,
        objects: &[Sphere],
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
                    for (offset, sphere) in objects[first..last].iter().enumerate() {
                        if let Some(distance) = sphere.hit_distance(origin, direction) {
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

    /// Hay algo tapando el tramo entre origin y una distancia dada?
    ///
    /// A diferencia de nearest_hit no interesa cual es la esfera mas cercana
    /// ni a que distancia: con encontrar una sola que estorbe alcanza, asi
    /// que corta apenas la encuentra. Es lo que necesitan los rayos de sombra.
    pub fn any_hit(
        &self,
        origin: &Vector3,
        direction: &Vector3,
        max_distance: f32,
        objects: &[Sphere],
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
                    for sphere in &objects[first..last] {
                        if let Some(distance) = sphere.hit_distance(origin, direction) {
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

/// Deja a la izquierda las esferas con centro por debajo de split y devuelve
/// donde quedo el corte.
fn partition(
    objects: &[Sphere],
    order: &mut [u32],
    start: usize,
    end: usize,
    axis: usize,
    split: f32,
) -> usize {
    let mut mid = start;
    for i in start..end {
        let center = objects[order[i] as usize].center;
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

    fn random_scene(count: usize, seed: u32) -> Vec<Sphere> {
        let mut rng = Lcg(seed);
        (0..count)
            .map(|_| Sphere {
                center: Vector3::new(
                    rng.range(-5.0, 5.0),
                    rng.range(-5.0, 5.0),
                    rng.range(-5.0, 5.0),
                ),
                radius: rng.range(0.05, 0.6),
                material: Material::matte(Color::new(10, 20, 30, 255)),
            })
            .collect()
    }

    /// La version lenta: probar todas las esferas. Es la referencia.
    fn brute_force(objects: &[Sphere], origin: &Vector3, direction: &Vector3) -> Option<f32> {
        objects
            .iter()
            .filter_map(|s| s.hit_distance(origin, direction))
            .fold(None, |best: Option<f32>, d| Some(best.map_or(d, |b| b.min(d))))
    }

    fn bvh_distance(
        bvh: &Bvh,
        objects: &[Sphere],
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
        let mut objects = vec![Sphere {
            center: Vector3::new(0.0, 0.0, -5.0),
            radius: 1.0,
            material: Material::matte(Color::new(1, 2, 3, 255)),
        }];
        let bvh = Bvh::build(&mut objects);

        let origin = Vector3::zero();
        let direction = Vector3::new(0.0, 0.0, -1.0);
        let hit = bvh_distance(&bvh, &objects, &origin, &direction).expect("tenia que pegar");
        assert!((hit - 4.0).abs() < 1e-4);

        // mirando al otro lado no hay nada
        assert!(bvh_distance(&bvh, &objects, &origin, &Vector3::new(0.0, 0.0, 1.0)).is_none());
    }

    #[test]
    fn empty_scene_never_hits() {
        let mut objects: Vec<Sphere> = Vec::new();
        let bvh = Bvh::build(&mut objects);
        assert!(bvh
            .nearest_hit(&Vector3::zero(), &Vector3::new(0.0, 0.0, -1.0), &objects)
            .is_none());
    }

    /// Esferas todas en el mismo centro: la particion por punto medio no
    /// separa nada y tiene que caer en el corte a la mitad sin colgarse.
    #[test]
    fn degenerate_split_terminates() {
        let mut objects: Vec<Sphere> = (0..32)
            .map(|i| Sphere {
                center: Vector3::new(0.0, 0.0, -5.0),
                radius: 0.1 + i as f32 * 0.01,
                material: Material::matte(Color::new(0, 0, 0, 255)),
            })
            .collect();
        let bvh = Bvh::build(&mut objects);

        let hit = bvh_distance(&bvh, &objects, &Vector3::zero(), &Vector3::new(0.0, 0.0, -1.0));
        assert!(hit.is_some());
    }

    /// any_hit tiene que coincidir con revisar todas las esferas a mano.
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

    /// Lo que queda mas lejos que el limite no tapa nada.
    #[test]
    fn any_hit_ignores_blockers_past_the_limit() {
        let mut objects = vec![Sphere {
            center: Vector3::new(0.0, 0.0, -5.0),
            radius: 1.0,
            material: Material::matte(Color::new(1, 2, 3, 255)),
        }];
        let bvh = Bvh::build(&mut objects);

        let origin = Vector3::zero();
        let direction = Vector3::new(0.0, 0.0, -1.0);

        assert!(bvh.any_hit(&origin, &direction, 10.0, &objects));
        assert!(!bvh.any_hit(&origin, &direction, 3.0, &objects));
    }
}
