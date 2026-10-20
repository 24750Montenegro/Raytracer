// light.rs
// Luces puntuales de la escena.
use raylib::prelude::*;

use crate::color::Rgb;

#[derive(Debug, Clone, Copy)]
pub struct Light {
    pub position: Vector3,
    /// Color de la luz; multiplica al color del material canal por canal.
    pub color: Rgb,
    /// Cuanto aporta esta luz.
    pub intensity: f32,
    /// Hasta donde llega. Infinito para las que hacen de sol: esas estan tan
    /// lejos que iluminan la escena entera parejo, y una caida cuadratica
    /// sobre ellas dejaria los bordes de la escena casi negros.
    ///
    /// Las lamparas si tienen alcance: iluminan solo lo que tienen cerca, y
    /// ademas es lo que hace que salgan baratas, porque un punto que les
    /// queda lejos se descarta con una resta, sin hacer la prueba de sombra.
    pub range: f32,
}

impl Light {
    pub fn new(position: Vector3, color: Rgb, intensity: f32) -> Self {
        Light {
            position,
            color,
            intensity,
            range: f32::INFINITY,
        }
    }

    /// Una lampara: ilumina lo que tiene alrededor y nada mas.
    pub fn lamp(position: Vector3, color: Rgb, intensity: f32, range: f32) -> Self {
        Light {
            range: range.max(f32::EPSILON),
            ..Light::new(position, color, intensity)
        }
    }

    /// Cuanta de su intensidad llega a esa distancia, de 1 pegado a la
    /// lampara a 0 en el borde de su alcance.
    ///
    /// No es la caida cuadratica de la fisica y a proposito. La cuadratica no
    /// llega a cero nunca, asi que habria que cortarla en algun lado, y ese
    /// corte se ve: aparece un circulo de luz con el borde marcado alrededor
    /// de cada lampara. Esta llega a cero justo en el alcance y ademas vale 1
    /// en el centro en vez de irse al infinito, que es lo que quema una
    /// superficie pegada a la lampara.
    pub fn falloff(&self, distance: f32) -> f32 {
        if self.range.is_infinite() {
            return 1.0;
        }

        let reach = 1.0 - distance / self.range;
        reach.max(0.0) * reach.max(0.0)
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
