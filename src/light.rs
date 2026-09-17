// light.rs
// Luces puntuales de la escena.
use raylib::prelude::*;

use crate::color::Rgb;

pub struct Light {
    pub position: Vector3,
    /// Color de la luz; multiplica al color del material canal por canal.
    pub color: Rgb,
    /// Cuanto aporta esta luz. No hay caida por distancia: la escena es
    /// chica y una caida cuadratica la dejaria casi negra en los bordes.
    pub intensity: f32,
}

impl Light {
    pub fn new(position: Vector3, color: Rgb, intensity: f32) -> Self {
        Light { position, color, intensity }
    }

    /// Direccion normalizada del punto hacia la luz, y a que distancia esta.
    /// La distancia la usa el rayo de sombra para no contar como bloqueo algo
    /// que esta mas lejos que la luz misma.
    pub fn direction_from(&self, point: Vector3) -> (Vector3, f32) {
        let to_light = self.position - point;
        let distance = to_light.length();
        if distance <= f32::EPSILON {
            return (Vector3::new(0.0, 0.0, 0.0), 0.0);
        }
        (to_light * (1.0 / distance), distance)
    }
}
