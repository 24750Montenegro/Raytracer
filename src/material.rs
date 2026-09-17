use raylib::prelude::*;

/// Albedo tipico de un material mate. Nada real devuelve toda la luz que le
/// llega: la nieve fresca ronda 0.9 y el asfalto 0.1, asi que quedarse un
/// poco por debajo de 1 ya se ve mas creible que el color plano de antes.
pub const DEFAULT_ALBEDO: f32 = 0.85;

/// Como reparte el material la luz que recibe.
/// Los dos pesos juntos no deberian pasar de 1: lo que sale por un lado no
/// puede volver a salir por el otro sin inventar energia.
#[derive(Debug, Clone, Copy)]
pub struct Albedo {
    /// Fraccion que rebota parejo en todas las direcciones (color del cuerpo).
    pub diffuse: f32,
    /// Fraccion que rebota como espejo y produce el brillo puntual.
    pub specular: f32,
}

impl Albedo {
    pub fn new(diffuse: f32, specular: f32) -> Self {
        Albedo {
            diffuse: diffuse.clamp(0.0, 1.0),
            specular: specular.clamp(0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Material {
    /// Color base de la superficie.
    pub diffuse: Color,
    /// Fraccion de la luz incidente que la superficie devuelve.
    /// Lo que no devuelve, lo absorbe.
    pub albedo: Albedo,
    /// Exponente de Phong: mas alto = brillo mas chico y concentrado,
    /// que es como se ve una superficie mas pulida.
    pub shininess: f32,
}

impl Material {
    pub fn new(diffuse: Color, albedo: Albedo, shininess: f32) -> Self {
        Material { diffuse, albedo, shininess: shininess.max(1.0) }
    }

    /// Material mate (tiza, yeso): rebota la luz parejo, sin brillo.
    pub fn matte(diffuse: Color) -> Self {
        Material::new(diffuse, Albedo::new(DEFAULT_ALBEDO, 0.0), 1.0)
    }

    /// Material tipo plastico: mayormente difuso pero con un brillo chico
    /// que delata la forma de la esfera.
    pub fn plastic(diffuse: Color) -> Self {
        Material::new(diffuse, Albedo::new(0.8, 0.25), 48.0)
    }
}
