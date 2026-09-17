// shading.rs
// Modelo de iluminacion: difuso de Lambert mas brillo especular de Phong.
//
// Antes cada esfera se pintaba con su color plano, asi que una esfera se veia
// como un circulo de un solo tono. Con Lambert el brillo de cada punto depende
// del angulo entre la normal y la luz, que es lo que da la sensacion de
// volumen; el especular agrega el reflejo puntual de la luz. El albedo
// reparte cuanta luz sale por cada uno de esos dos caminos.
use raylib::prelude::*;

use crate::color::Rgb;
use crate::material::Material;
use crate::scene::Scene;

/// Color final de un punto de la superficie.
/// view_dir va del punto hacia la camara, ya normalizado.
pub fn shade(
    scene: &Scene,
    material: &Material,
    point: Vector3,
    normal: Vector3,
    view_dir: Vector3,
) -> Color {
    let base = Rgb::from_color(material.diffuse);

    // la ambiente es el piso: sin ella, lo que no ve ninguna luz queda negro
    let mut diffuse = scene.ambient;
    let mut specular = Rgb::new(0.0, 0.0, 0.0);

    for light in &scene.lights {
        let (to_light, _distance) = light.direction_from(point);

        // ley del coseno: la superficie recibe menos luz cuanto mas de canto
        // le llega. De espaldas a la luz (producto punto <= 0) no recibe nada.
        let lambert = normal.dot(to_light);
        if lambert <= 0.0 {
            continue;
        }

        diffuse = diffuse + light.color * (light.intensity * lambert);

        if material.albedo.specular <= 0.0 {
            continue;
        }

        // Phong: el brillo aparece donde el rebote de la luz apunta al ojo.
        // El exponente controla que tan cerrado queda ese brillo.
        let bounce = reflect(-to_light, normal).dot(view_dir);
        if bounce > 0.0 {
            let highlight = bounce.powf(material.shininess) * light.intensity;
            specular = specular + light.color * highlight;
        }
    }

    // albedo: cuanta luz sale por cada camino en vez de absorberse
    let color = base * diffuse * material.albedo.diffuse + specular * material.albedo.specular;
    color.to_color()
}

/// Rebote de un vector contra la superficie, espejado sobre la normal.
#[inline(always)]
fn reflect(incident: Vector3, normal: Vector3) -> Vector3 {
    incident - normal * (2.0 * incident.dot(normal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::light::Light;
    use crate::material::Albedo;
    use crate::scene::Scene;

    /// Camara imaginaria justo encima del punto, mirando hacia abajo.
    const VIEW: Vector3 = Vector3 { x: 0.0, y: -1.0, z: 0.0 };

    fn white_matte(albedo: f32) -> Material {
        Material::new(Color::new(255, 255, 255, 255), Albedo::new(albedo, 0.0), 1.0)
    }

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
        let material = white_matte(1.0);

        let facing = shade(&scene, &material, Vector3::zero(), Vector3::new(0.0, -1.0, 0.0), VIEW);
        let tilted = shade(&scene, &material, Vector3::zero(), Vector3::new(0.7, -0.7, 0.0), VIEW);

        assert_eq!(facing.r, 255);
        assert!(tilted.r < facing.r, "de canto tiene que llegar menos luz");
    }

    #[test]
    fn surface_turned_away_only_gets_ambient() {
        let scene = single_light_scene(1.0, 0.25);
        let material = white_matte(1.0);

        // normal apuntando al lado opuesto de la luz
        let away = shade(&scene, &material, Vector3::zero(), Vector3::new(0.0, 1.0, 0.0), VIEW);

        assert_eq!(away.r, (0.25 * 255.0) as u8);
    }

    /// Dos materiales del mismo color pero distinto albedo tienen que
    /// verse distinto: eso es justamente lo que aporta el albedo.
    #[test]
    fn albedo_scales_the_reflected_light() {
        let scene = single_light_scene(1.0, 0.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);
        let color = Color::new(200, 200, 200, 255);
        let bright_material = Material::new(color, Albedo::new(0.9, 0.0), 1.0);
        let dark_material = Material::new(color, Albedo::new(0.2, 0.0), 1.0);

        let bright = shade(&scene, &bright_material, Vector3::zero(), normal, VIEW);
        let dark = shade(&scene, &dark_material, Vector3::zero(), normal, VIEW);

        assert!(dark.r < bright.r);
        assert!(bright.r > 0);
    }

    #[test]
    fn light_color_tints_the_material() {
        let mut scene = single_light_scene(1.0, 0.0);
        scene.lights[0].color = Rgb::new(1.0, 0.0, 0.0);

        let white = white_matte(1.0);
        let lit = shade(&scene, &white, Vector3::zero(), Vector3::new(0.0, -1.0, 0.0), VIEW);

        assert_eq!(lit.r, 255);
        assert_eq!(lit.g, 0);
        assert_eq!(lit.b, 0);
    }

    /// El brillo especular aparece donde el rebote de la luz apunta al ojo.
    #[test]
    fn specular_highlight_shows_up_head_on() {
        let scene = single_light_scene(1.0, 0.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);
        let shiny = Material::new(Color::new(30, 30, 30, 255), Albedo::new(0.2, 0.8), 32.0);

        // luz justo arriba y ojo justo arriba: el rebote va derecho a la camara
        let head_on = shade(&scene, &shiny, Vector3::zero(), normal, VIEW);
        // mirando de costado, el brillo se apaga
        let off_axis = shade(
            &scene,
            &shiny,
            Vector3::zero(),
            normal,
            Vector3::new(0.9, -0.44, 0.0).normalized(),
        );

        assert!(head_on.r > off_axis.r, "{} vs {}", head_on.r, off_axis.r);
    }

    /// Mismo material pero mas pulido: el brillo se cierra, asi que fuera del
    /// eje cae mas rapido.
    #[test]
    fn higher_shininess_narrows_the_highlight() {
        let scene = single_light_scene(1.0, 0.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);
        let off_axis = Vector3::new(0.5, -0.86, 0.0).normalized();
        let color = Color::new(30, 30, 30, 255);

        let soft = Material::new(color, Albedo::new(0.2, 0.8), 8.0);
        let sharp = Material::new(color, Albedo::new(0.2, 0.8), 128.0);

        let soft_edge = shade(&scene, &soft, Vector3::zero(), normal, off_axis);
        let sharp_edge = shade(&scene, &sharp, Vector3::zero(), normal, off_axis);

        assert!(sharp_edge.r < soft_edge.r);
    }

    /// Un material mate no tiene brillo por mas que la luz le pegue de frente.
    #[test]
    fn matte_material_has_no_highlight() {
        let scene = single_light_scene(1.0, 0.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);
        let matte = Material::matte(Color::new(0, 0, 0, 255));

        let lit = shade(&scene, &matte, Vector3::zero(), normal, VIEW);

        assert_eq!(lit.r, 0, "el negro mate tiene que seguir negro");
    }
}
