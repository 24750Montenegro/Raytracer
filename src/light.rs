// light.rs
// Luces puntuales de la escena.
use raylib::prelude::*;

use crate::color::Rgb;

pub struct Light {
    pub position: Vector3,
    /// Color de la luz; multiplica al color del material canal por canal.
    pub color: Rgb,
    /// Cuanto aporta esta luz.
    pub intensity: f32,
    /// Hasta donde llega. Infinito para las que hacen de sol: esas estan tan
    /// lejos que iluminan la escena entera parejo, y una caida cuadratica
    /// sobre ellas dejaria los bordes del mapa casi negros.
    ///
    /// Las lamparas de la escena si tienen alcance. Una lampara marina no
    /// ilumina la isla de enfrente, y ademas es lo que hace que salgan
    /// baratas: un punto que le queda lejos se descarta con una resta, sin
    /// tirar el rayo de sombra.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Las que hacen de sol no tienen caida: estan tan lejos que alumbran la
    /// escena entera parejo, y una caida cuadratica sobre ellas dejaria los
    /// bordes del mapa casi negros.
    #[test]
    fn a_distant_light_reaches_everywhere_the_same() {
        let sun = Light::new(Vector3::zero(), Rgb::new(1.0, 1.0, 1.0), 1.0);

        for distance in [0.0, 1.0, 50.0, 10_000.0] {
            assert_eq!(sun.falloff(distance), 1.0, "a {distance}");
        }
    }

    /// Una lampara si: entera encima, nada en el borde de su alcance, y de
    /// ahi para afuera nada tampoco.
    #[test]
    fn a_lamp_fades_out_within_its_reach() {
        let lamp = Light::lamp(Vector3::zero(), Rgb::new(1.0, 1.0, 1.0), 1.0, 10.0);

        assert_eq!(lamp.falloff(0.0), 1.0);
        assert_eq!(lamp.falloff(10.0), 0.0, "en el borde tiene que apagarse");
        assert_eq!(lamp.falloff(25.0), 0.0, "y mas alla tambien");

        // y entre medio baja sin saltos
        let mut previous = 1.0;
        for step in 1..=20 {
            let reach = lamp.falloff(step as f32 * 0.5);
            assert!(reach <= previous, "subio en el paso {step}");
            assert!((0.0..=1.0).contains(&reach));
            previous = reach;
        }
    }

    /// Lo que justifica que llegue a cero justo en el alcance en vez de
    /// cortar una caida cuadratica: un corte se ve como un circulo de luz
    /// con el borde marcado alrededor de la lampara.
    #[test]
    fn the_lamp_goes_out_without_an_edge() {
        let lamp = Light::lamp(Vector3::zero(), Rgb::new(1.0, 1.0, 1.0), 1.0, 10.0);

        assert!(lamp.falloff(9.9) < 0.001, "{}", lamp.falloff(9.9));
    }

    #[test]
    fn the_direction_points_from_the_surface_towards_the_light() {
        let light = Light::new(Vector3::new(0.0, -5.0, 0.0), Rgb::new(1.0, 1.0, 1.0), 1.0);
        let (to_light, distance) = light.direction_from(Vector3::zero());

        assert!((distance - 5.0).abs() < 1e-5);
        assert!((to_light - Vector3::new(0.0, -1.0, 0.0)).length() < 1e-5);
    }
}
