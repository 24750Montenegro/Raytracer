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

/// Como alumbra una lampara marina: el blanco verdoso de su propia textura,
/// fuerte de cerca y hasta donde llega un puñado de bloques.
const LAMP_COLOR: Rgb = Rgb::new(0.72, 0.94, 0.92);
const LAMP_INTENSITY: f32 = 2.4;
const LAMP_RANGE: f32 = 11.0;

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
        let models = terrain::block_models();
        let terrain = Terrain::generate(seed);
        let mut objects = terrain.blocks(&models);
        // la escena no cambia nunca, asi que el BVH se arma una sola vez
        let bvh = Bvh::build(&mut objects);
        let (center, radius) = bounding_sphere(&objects);

        let mut lights = default_lights(center, radius);
        lights.extend(lamp_lights(&terrain.lamps()));

        Scene {
            objects,
            bvh,
            lights,
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

/// Tres luces, como en fotografia: una principal, una de relleno mas debil
/// del lado opuesto para que las sombras no queden planas, y una tercera por
/// debajo. Van en funcion del tamano de la escena, y como aca -Y es arriba,
/// las alturas de las de arriba son negativas.
///
/// La de abajo es la que pide la escena nueva. Con islas flotantes, la panza
/// de cada una queda de espaldas a todo lo que ilumina desde arriba, y sin
/// nada que la levante se ve como una mancha negra recortada contra el
/// cielo. Es la misma luz que en el mundo real sube del suelo y de las
/// nubes, asi que va debil y tirando a azul.
fn default_lights(center: Vector3, radius: f32) -> Vec<Light> {
    vec![
        Light::new(
            center + Vector3::new(-1.2 * radius, -2.2 * radius, 1.4 * radius),
            Rgb::new(1.0, 0.96, 0.88),
            0.9,
        ),
        Light::new(
            center + Vector3::new(1.8 * radius, -0.6 * radius, -1.1 * radius),
            Rgb::new(0.68, 0.80, 1.0),
            0.3,
        ),
        Light::new(
            center + Vector3::new(0.7 * radius, 2.0 * radius, 0.9 * radius),
            Rgb::new(0.74, 0.84, 1.0),
            0.34,
        ),
    ]
}

/// Una luz por cada lampara marina de la isla helada.
///
/// Van aparte de las tres de arriba porque son otra cosa: aquellas hacen de
/// sol y de cielo, estan lejisimos y alumbran la escena entera; estas estan
/// metidas adentro del mapa y alumbran los bloques que tienen al lado. De ahi
/// el alcance: una lampara marina no ilumina la isla de enfrente, y ademas
/// ese alcance es lo que las hace baratas, porque un punto que les queda
/// lejos se descarta con una resta en vez de con un rayo de sombra.
///
/// El color es el de la propia lampara: un blanco tirando a verde de agua.
fn lamp_lights(places: &[Vector3]) -> Vec<Light> {
    places
        .iter()
        .map(|place| Light::lamp(*place, LAMP_COLOR, LAMP_INTENSITY, LAMP_RANGE))
        .collect()
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
    use crate::terrain::{ISLAND, SIZE};

    /// Pegadas al origen quedarian adentro de una escena mas grande.
    #[test]
    fn lights_scale_with_the_scene() {
        let center = Vector3::new(1.0, 2.0, -3.0);
        let lights = default_lights(center, 4.0);

        assert_eq!(lights.len(), 3);
        for light in &lights {
            assert!((light.position - center).length() > 4.0);
        }
        for fill in &lights[1..] {
            assert!(lights[0].intensity > fill.intensity, "la principal manda");
        }
    }

    /// La principal viene de arriba y la tercera de abajo: es la que levanta
    /// la panza de las islas, que si no queda de espaldas a todo.
    #[test]
    fn the_key_light_comes_from_above_and_one_fill_from_below() {
        let lights = default_lights(Vector3::zero(), 10.0);

        assert!(lights[0].position.y < 0.0, "arriba es -Y");
        assert!(lights[2].position.y > 0.0, "la de relleno va por debajo");
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

    /// La escena llega de una isla a la de enfrente, pero no llena el mapa:
    /// entre una y otra hay cielo, y ese hueco es la mitad del punto de
    /// haberlas separado.
    #[test]
    fn the_scene_spans_the_islands_and_the_sky_between_them() {
        let scene = Scene::from_seed(42);
        let (min, max) = scene
            .objects
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), object| {
                let (low, high) = object.bounds();
                (min.min(low.x), max.max(high.x))
            });

        let span = max - min;
        assert!(span > ISLAND as f32 * 2.0, "la escena mide {span} de ancho");
        assert!(span < SIZE as f32, "la escena llena el mapa entero");

        // y en el medio, donde va el cielo, no hay ni un bloque
        let middle = scene.objects.iter().any(|object| {
            let (low, high) = object.bounds();
            low.x > min + span * 0.48 && high.x < min + span * 0.52
        });
        assert!(!middle, "el pasillo entre las islas quedo tapado");
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
