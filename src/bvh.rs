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
