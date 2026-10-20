// shading.rs
// Modelo de iluminacion: luz ambiente, difuso de Lambert, brillo especular
// de Phong, emision y sombras.
//
// Con Lambert el brillo de cada punto depende del angulo entre la normal y
// la luz, que es lo que da la sensacion de volumen; el especular agrega el
// reflejo puntual de la luz. El albedo reparte cuanta luz sale por cada uno
// de esos dos caminos.
//
// Viene del raytracer, pero ya no depende de como se encuentra la geometria:
// recibe el punto, su normal y desde donde se mira, y la prueba de sombra
// llega como una funcion. Asi lo puede llamar un trazador de rayos (que
// prueba la sombra tirando un rayo hacia la luz) o el fragment shader de un
// rasterizador (que puede pasar `no_shadows` o, mas adelante, consultar un
// shadow map).
use raylib::prelude::*;

use crate::color::Rgb;
use crate::light::Light;
use crate::material::Material;

/// Cuanto se despega de la superficie el origen de un rayo de sombra. Sin
/// este empujon el cuerpo se toca a si mismo por error de redondeo y sale
/// el moteado clasico.
pub const SURFACE_BIAS: f32 = 1e-3;

/// Donde se evalua la luz.
pub struct SurfacePoint {
    pub point: Vector3,
    /// Normal unitaria, del lado desde el que se mira.
    pub normal: Vector3,
    /// Del punto hacia el observador, ya normalizado.
    pub view_dir: Vector3,
}

/// Prueba de sombra que dice que nada tapa nunca ninguna luz. Es la que se
/// usa mientras no haya geometria contra la cual probar.
pub fn no_shadows(_origin: Vector3, _to_light: Vector3, _distance: f32) -> bool {
    false
}

/// Luz que sale de un punto de la superficie: ambiente, difuso, especular y
/// lo que el material emite por su cuenta.
///
/// base es el color que ya trae la superficie (el del material, o el de una
/// textura). in_shadow(origen, direccion_a_la_luz, distancia) responde si hay
/// algo entre el punto y esa luz; el origen ya viene despegado de la
/// superficie.
pub fn shade(
    lights: &[Light],
    ambient: Rgb,
    material: &Material,
    base: Rgb,
    surface: &SurfacePoint,
    in_shadow: impl Fn(Vector3, Vector3, f32) -> bool,
) -> Rgb {
    let SurfacePoint { point, normal, view_dir } = *surface;

    // la ambiente es el piso: sin ella, lo que no ve ninguna luz queda negro
    let mut diffuse = ambient;
    let mut specular = Rgb::new(0.0, 0.0, 0.0);

    for light in lights {
        let (to_light, distance) = light.direction_from(point);

        // lo primero es lo mas barato: una lampara que no llega hasta aca se
        // descarta con una resta, antes de tocar la geometria
        let reach = light.falloff(distance);
        if reach <= 0.0 {
            continue;
        }

        // ley del coseno: la superficie recibe menos luz cuanto mas de canto
        // le llega. De espaldas a la luz (producto punto <= 0) no recibe nada.
        let lambert = normal.dot(to_light);
        if lambert <= 0.0 {
            continue;
        }

        // el orden importa: el coseno es una multiplicacion y la prueba de
        // sombra puede recorrer toda la geometria, asi que va despues
        if in_shadow(point + normal * SURFACE_BIAS, to_light, distance) {
            continue;
        }

        diffuse = diffuse + light.color * (light.intensity * reach * lambert);

        if material.albedo.specular <= 0.0 {
            continue;
        }

        // Phong: el brillo aparece donde el rebote de la luz apunta al ojo.
        // El exponente controla que tan cerrado queda ese brillo.
        let bounce = reflect(-to_light, normal).dot(view_dir);
        if bounce > 0.0 {
            let highlight = bounce.powf(material.shininess) * light.intensity * reach;
            specular = specular + light.color * highlight;
        }
    }

    // albedo: cuanta luz sale por cada camino en vez de absorberse
    let mut color = base * diffuse * material.albedo.diffuse
        + specular * material.albedo.specular * mirror_tint(material, base);

    // lo que el cuerpo emite va aparte: no depende de que le llegue ninguna
    // luz, que es lo que hace que una lampara se vea encendida en la sombra
    if material.emission > 0.0 {
        color = color + base * material.emission;
    }

    color
}

/// Con que color sale lo que la superficie refleja: el suyo si es un metal,
/// blanco si no.
fn mirror_tint(material: &Material, base: Rgb) -> Rgb {
    if material.metallic {
        base
    } else {
        Rgb::new(1.0, 1.0, 1.0)
    }
}

/// Rebote de un vector contra la superficie, espejado sobre la normal.
#[inline(always)]
pub fn reflect(incident: Vector3, normal: Vector3) -> Vector3 {
    incident - normal * (2.0 * incident.dot(normal))
}
