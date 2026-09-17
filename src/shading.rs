// shading.rs
// Modelo de iluminacion difusa (Lambert).
//
// Antes cada esfera se pintaba con su color plano, asi que una esfera se veia
// como un circulo de un solo tono. Con Lambert el brillo de cada punto depende
// del angulo entre la normal y la luz, que es lo que da la sensacion de
// volumen; el albedo dice que fraccion de esa luz devuelve el material.
use raylib::prelude::*;

use crate::color::Rgb;
use crate::material::Material;
use crate::scene::Scene;

/// Color final de un punto de la superficie.
pub fn shade(scene: &Scene, material: &Material, point: Vector3, normal: Vector3) -> Color {
    let base = Rgb::from_color(material.diffuse);

    // la ambiente es el piso: sin ella, lo que no ve ninguna luz queda negro
    let mut incoming = scene.ambient;

    for light in &scene.lights {
        let (to_light, _distance) = light.direction_from(point);

        // ley del coseno: la superficie recibe menos luz cuanto mas de canto
        // le llega. De espaldas a la luz (producto punto <= 0) no recibe nada.
        let lambert = normal.dot(to_light);
        if lambert <= 0.0 {
            continue;
        }

        incoming = incoming + light.color * (light.intensity * lambert);
    }

    // albedo: cuanta de esa luz rebota en vez de absorberse
    (base * incoming * material.albedo).to_color()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::light::Light;
    use crate::scene::Scene;

    /// Escena minima: una luz blanca a un metro sobre el punto (0,0,0),
    /// mirando hacia abajo. Recordar que en esta escena -Y es arriba.
    fn single_light_scene(intensity: f32, ambient: f32) -> Scene {
        Scene::for_test(
            vec![Light::new(
                Vector3::new(0.0, -1.0, 0.0),
                Rgb::new(1.0, 1.0, 1.0),
                intensity,
            )],
            Rgb::new(ambient, ambient, ambient),
        )
    }

    #[test]
    fn surface_facing_the_light_is_brightest() {
        let scene = single_light_scene(1.0, 0.0);
        let material = Material::new(Color::new(255, 255, 255, 255), 1.0);

        let facing = shade(&scene, &material, Vector3::zero(), Vector3::new(0.0, -1.0, 0.0));
        let tilted = shade(&scene, &material, Vector3::zero(), Vector3::new(0.7, -0.7, 0.0));

        assert_eq!(facing.r, 255);
        assert!(tilted.r < facing.r, "de canto tiene que llegar menos luz");
    }

    #[test]
    fn surface_turned_away_only_gets_ambient() {
        let scene = single_light_scene(1.0, 0.25);
        let material = Material::new(Color::new(255, 255, 255, 255), 1.0);

        // normal apuntando al lado opuesto de la luz
        let away = shade(&scene, &material, Vector3::zero(), Vector3::new(0.0, 1.0, 0.0));

        assert_eq!(away.r, (0.25 * 255.0) as u8);
    }

    /// Dos materiales del mismo color pero distinto albedo tienen que
    /// verse distinto: eso es justamente lo que aporta el albedo.
    #[test]
    fn albedo_scales_the_reflected_light() {
        let scene = single_light_scene(1.0, 0.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);
        let color = Color::new(200, 200, 200, 255);

        let bright = shade(&scene, &Material::new(color, 0.9), Vector3::zero(), normal);
        let dark = shade(&scene, &Material::new(color, 0.2), Vector3::zero(), normal);

        assert!(dark.r < bright.r);
        assert!(bright.r > 0);
    }

    #[test]
    fn light_color_tints_the_material() {
        let mut scene = single_light_scene(1.0, 0.0);
        scene.lights[0].color = Rgb::new(1.0, 0.0, 0.0);

        let white = Material::new(Color::new(255, 255, 255, 255), 1.0);
        let lit = shade(&scene, &white, Vector3::zero(), Vector3::new(0.0, -1.0, 0.0));

        assert_eq!(lit.r, 255);
        assert_eq!(lit.g, 0);
        assert_eq!(lit.b, 0);
    }
}
