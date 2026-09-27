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
