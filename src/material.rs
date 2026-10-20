// material.rs
// Como responde una superficie a la luz. Lo usa shading.rs.
use raylib::prelude::*;

/// Albedo tipico de un material mate. Nada real devuelve toda la luz que le
/// llega: la nieve fresca ronda 0.9 y el asfalto 0.1, asi que quedarse un
/// poco por debajo de 1 ya se ve mas creible que el color plano.
pub const DEFAULT_ALBEDO: f32 = 0.85;

/// Como reparte el material la luz que recibe.
/// Los pesos juntos no deberian pasar de 1: lo que sale por un camino no
/// puede volver a salir por otro sin inventar energia.
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
    /// Cuanta luz emite por su cuenta, en las mismas unidades que la que
    /// recibe: 0 es un cuerpo comun y 1 se ve como si le diera el sol de
    /// lleno aunque este en la sombra.
    pub emission: f32,
    /// Si es un metal.
    ///
    /// Es la diferencia fisica entre el oro y un plastico amarillo: en un
    /// dielectrico la luz que rebota en la superficie sale con el color de
    /// la luz, y el color propio lo pone la que se metio adentro y volvio.
    /// En un metal no hay adentro: la luz rebota en el mar de electrones y
    /// sale ya tenida. Por eso el brillo de una chapa de oro es dorado y no
    /// blanco.
    pub metallic: bool,
}

impl Material {
    pub fn new(diffuse: Color, albedo: Albedo, shininess: f32) -> Self {
        Material {
            diffuse,
            albedo,
            shininess: shininess.max(1.0),
            emission: 0.0,
            metallic: false,
        }
    }

    /// Un cuerpo que ademas de reflejar la luz que le llega, emite la suya.
    ///
    /// El difuso va bajo a proposito: una lampara encendida no se ve mas
    /// clara todavia porque le de el sol, se ve encendida y punto. Dejarle el
    /// difuso alto sumaria las dos cosas y de dia saldria blanca quemada.
    pub fn lamp(diffuse: Color, emission: f32) -> Self {
        Material {
            emission: emission.max(0.0),
            ..Material::new(diffuse, Albedo::new(0.35, 0.12), 60.0)
        }
    }

    /// Material mate (tiza, yeso): rebota la luz parejo, sin brillo.
    pub fn matte(diffuse: Color) -> Self {
        Material::new(diffuse, Albedo::new(DEFAULT_ALBEDO, 0.0), 1.0)
    }

    /// Superficie barnizada: casi todo difuso y un brillo moderado.
    pub fn polished(diffuse: Color) -> Self {
        Material::new(diffuse, Albedo::new(0.7, 0.18), 110.0)
    }

    /// Laqueado: mas brillo que el barniz y mas concentrado.
    pub fn lacquer(diffuse: Color) -> Self {
        Material::new(diffuse, Albedo::new(0.64, 0.22), 150.0)
    }

    /// Metal: poco color difuso y un brillo fuerte tenido con su color.
    pub fn metal(diffuse: Color) -> Self {
        Material {
            metallic: true,
            ..Material::new(diffuse, Albedo::new(0.25, 0.6), 320.0)
        }
    }
}
