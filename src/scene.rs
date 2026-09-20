// scene.rs
// La escena que renderiza el raytracer: el terreno de bloques, las luces y la
// luz ambiente. La geometria sale entera del generador de terreno.
use raylib::prelude::*;

use crate::bvh::Bvh;
use crate::color::Rgb;
use crate::light::Light;
use crate::noise;
use crate::object::Object;
use crate::terrain::{self, Terrain};
use crate::texture::Texture;

pub const BACKGROUND_COLOR: Color = Color { r: 124, g: 176, b: 232, a: 255 };

/// Piso de iluminacion: sin ambiente, lo que no ve ninguna luz queda negro
/// puro. Tira a azul porque a cielo abierto las sombras las rellena el cielo.
const AMBIENT: Rgb = Rgb::new(0.24, 0.27, 0.33);

pub struct Scene {
    pub objects: Vec<Object>,
    pub bvh: Bvh,
    pub lights: Vec<Light>,
    pub ambient: Rgb,
    /// Lo que ve un rayo que se escapa de la escena.
    pub background: Rgb,
    /// Centro y radio, para plantar la camara y las luces.
    pub center: Vector3,
    pub radius: f32,
    pub seed: u64,
    /// Las texturas de los bloques. Cada material guarda en cual cae.
    pub textures: Vec<Texture>,
}

impl Scene {
    /// Terreno nuevo en cada corrida del programa.
    pub fn random() -> Self {
        Scene::from_seed(noise::random_seed())
    }

    pub fn from_seed(seed: u64) -> Self {
        let mut objects = Terrain::generate(seed).blocks();
        // la escena no cambia nunca, asi que el BVH se arma una sola vez
        let bvh = Bvh::build(&mut objects);
        let (center, radius) = bounding_sphere(&objects);

        Scene {
            objects,
            bvh,
            lights: default_lights(center, radius),
            ambient: AMBIENT,
            background: Rgb::from_color(BACKGROUND_COLOR),
            center,
            radius,
            seed,
            textures: terrain::block_textures(),
        }
    }

    /// Escena vacia con luces a medida, para los tests de shading.
    #[cfg(test)]
    pub fn for_test(lights: Vec<Light>, ambient: Rgb) -> Self {
        let mut objects = Vec::new();
        let bvh = Bvh::build(&mut objects);
        Scene {
            objects,
            bvh,
            lights,
            ambient,
            background: Rgb::from_color(BACKGROUND_COLOR),
            center: Vector3::zero(),
            radius: 1.0,
            seed: 0,
            textures: vec![Texture::plain()],
        }
    }
}

/// Dos luces, como en fotografia: una principal y una de relleno mas debil
/// del lado opuesto para que las sombras no queden planas. Van en funcion del
/// tamano de la escena, y como aca -Y es arriba, las alturas son negativas.
fn default_lights(center: Vector3, radius: f32) -> Vec<Light> {
    vec![
        Light::new(
            center + Vector3::new(-1.2 * radius, -1.8 * radius, 1.6 * radius),
            Rgb::new(1.0, 0.96, 0.88),
            0.9,
        ),
        Light::new(
            center + Vector3::new(1.8 * radius, -0.6 * radius, -1.1 * radius),
            Rgb::new(0.68, 0.80, 1.0),
            0.32,
        ),
    ]
}

/// Esfera envolvente de la escena, para encuadrar la camara sin hardcodear
/// numeros.
pub fn bounding_sphere(objects: &[Object]) -> (Vector3, f32) {
    if objects.is_empty() {
        return (Vector3::zero(), 1.0);
    }

    let mut min = Vector3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vector3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

    for object in objects {
        let (low, high) = object.bounds();
        min.x = min.x.min(low.x);
        min.y = min.y.min(low.y);
        min.z = min.z.min(low.z);
        max.x = max.x.max(high.x);
        max.y = max.y.max(high.y);
        max.z = max.z.max(high.z);
    }

    let center = (min + max) * 0.5;
    let radius = (max - center).length().max(0.001);
    (center, radius)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::SIZE;

    /// Pegadas al origen quedarian adentro de una escena mas grande.
    #[test]
    fn lights_scale_with_the_scene() {
        let center = Vector3::new(1.0, 2.0, -3.0);
        let lights = default_lights(center, 4.0);

        assert_eq!(lights.len(), 2);
        for light in &lights {
            assert!((light.position - center).length() > 4.0);
        }
        assert!(lights[0].intensity > lights[1].intensity, "la principal manda");
    }

    #[test]
    fn the_key_light_comes_from_above() {
        let lights = default_lights(Vector3::zero(), 10.0);
        assert!(lights[0].position.y < 0.0);
    }

    /// Tiene que cubrir hasta el borde de cada cuerpo, no solo su centro: en
    /// el cubo el punto mas lejano es una esquina.
    #[test]
    fn bounding_sphere_covers_every_object() {
        let scene = Scene::from_seed(42);
        let (center, radius) = bounding_sphere(&scene.objects);

        for object in &scene.objects {
            let (min, max) = object.bounds();
            for corner in [
                Vector3::new(min.x, min.y, min.z),
                Vector3::new(min.x, min.y, max.z),
                Vector3::new(min.x, max.y, min.z),
                Vector3::new(min.x, max.y, max.z),
                Vector3::new(max.x, min.y, min.z),
                Vector3::new(max.x, min.y, max.z),
                Vector3::new(max.x, max.y, min.z),
                Vector3::new(max.x, max.y, max.z),
            ] {
                let reach = (corner - center).length();
                assert!(reach <= radius + 1e-3, "{reach} pasa de {radius}");
            }
        }
    }

    #[test]
    fn the_scene_is_made_of_blocks() {
        let scene = Scene::from_seed(42);

        assert!(!scene.objects.is_empty());
        assert!(scene.objects.iter().all(|o| matches!(o, Object::Cube(_))));
    }

    #[test]
    fn the_scene_spans_the_whole_plane() {
        let scene = Scene::from_seed(42);
        let (min, max) = scene
            .objects
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), object| {
                let (low, high) = object.bounds();
                (min.min(low.x), max.max(high.x))
            });

        assert!((max - min - SIZE as f32).abs() < 1e-3, "el plano mide {}", max - min);
    }

    #[test]
    fn each_run_gets_its_own_terrain() {
        let a = Scene::random();
        let b = Scene::random();

        assert_ne!(a.seed, b.seed);
        let same_shape = a.objects.len() == b.objects.len()
            && a.objects
                .iter()
                .zip(&b.objects)
                .all(|(x, y)| (x.center() - y.center()).length() < 1e-6);
        assert!(!same_shape, "dos corridas seguidas dieron el mismo terreno");
    }

    #[test]
    fn a_seed_rebuilds_its_scene() {
        let a = Scene::from_seed(20240519);
        let b = Scene::from_seed(20240519);

        assert_eq!(a.objects.len(), b.objects.len());
        for (x, y) in a.objects.iter().zip(&b.objects) {
            assert!((x.center() - y.center()).length() < 1e-6);
        }
    }
}
