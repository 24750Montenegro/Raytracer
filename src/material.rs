use raylib::prelude::*;

/// Albedo tipico de un material mate. Nada real devuelve toda la luz que le
/// llega: la nieve fresca ronda 0.9 y el asfalto 0.1, asi que quedarse un
/// poco por debajo de 1 ya se ve mas creible que el color plano de antes.
pub const DEFAULT_ALBEDO: f32 = 0.85;

/// Indice de refraccion del aire: la luz no se dobla.
pub const AIR_INDEX: f32 = 1.0;
/// El del agua.
pub const WATER_INDEX: f32 = 1.33;

/// La textura lisa y blanca, que deja al material con su color plano. Va
/// primera en la tabla de la escena para que sea el valor por omision.
pub const PLAIN_TEXTURE: u16 = 0;

/// Como reparte el material la luz que recibe.
/// Los pesos juntos no deberian pasar de 1: lo que sale por un camino no
/// puede volver a salir por otro sin inventar energia.
#[derive(Debug, Clone, Copy)]
pub struct Albedo {
    /// Fraccion que rebota parejo en todas las direcciones (color del cuerpo).
    pub diffuse: f32,
    /// Fraccion que rebota como espejo y produce el brillo puntual.
    pub specular: f32,
    /// Fraccion que devuelve la imagen de lo que tiene enfrente.
    pub reflection: f32,
    /// Fraccion que deja pasar la luz a traves del cuerpo.
    pub refraction: f32,
}

impl Albedo {
    pub fn new(diffuse: f32, specular: f32, reflection: f32, refraction: f32) -> Self {
        Albedo {
            diffuse: diffuse.clamp(0.0, 1.0),
            specular: specular.clamp(0.0, 1.0),
            reflection: reflection.clamp(0.0, 1.0),
            refraction: refraction.clamp(0.0, 1.0),
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
    /// Indice de refraccion: cuanto se dobla la luz al entrar. El aire vale
    /// 1.0, el vidrio comun 1.5 y el agua 1.33. Solo importa si el material
    /// deja pasar algo de luz.
    pub refractive_index: f32,
    /// Que textura lo dibuja, por posicion en la tabla de la escena.
    pub texture: u16,
}

impl Material {
    pub fn new(diffuse: Color, albedo: Albedo, shininess: f32) -> Self {
        Material {
            diffuse,
            albedo,
            shininess: shininess.max(1.0),
            refractive_index: AIR_INDEX,
            texture: PLAIN_TEXTURE,
        }
    }

    /// Le cuelga una textura: el color del material pasa a ser un tinte que
    /// multiplica al de cada texel.
    pub fn textured(self, texture: u16) -> Self {
        Material { texture, ..self }
    }

    /// Igual que new pero para cuerpos que dejan pasar la luz.
    pub fn transparent(
        diffuse: Color,
        albedo: Albedo,
        shininess: f32,
        refractive_index: f32,
    ) -> Self {
        Material {
            refractive_index: refractive_index.max(AIR_INDEX),
            ..Material::new(diffuse, albedo, shininess)
        }
    }

    /// Material mate (tiza, yeso): rebota la luz parejo, sin brillo.
    pub fn matte(diffuse: Color) -> Self {
        Material::new(diffuse, Albedo::new(DEFAULT_ALBEDO, 0.0, 0.0, 0.0), 1.0)
    }

    /// Material tipo plastico: mayormente difuso pero con un brillo chico
    /// que delata la forma de la esfera.
    pub fn plastic(diffuse: Color) -> Self {
        Material::new(diffuse, Albedo::new(0.8, 0.25, 0.0, 0.0), 48.0)
    }

    /// Metal pulido: poco color propio y mucha imagen del entorno.
    pub fn mirror(diffuse: Color) -> Self {
        Material::new(diffuse, Albedo::new(0.15, 0.4, 0.6, 0.0), 256.0)
    }

    /// Agua: el unico material de la escena que deja pasar la luz.
    ///
    /// Lo que mas pesa es el reflejo, porque es lo que se ve de una lamina de
    /// agua quieta. El color propio queda bajo, pero no en cero: es lo que
    /// evita que una laguna con el fondo en sombra salga negra.
    pub fn water(diffuse: Color) -> Self {
        Material::transparent(
            diffuse,
            Albedo::new(0.22, 0.4, 0.5, 0.3),
            160.0,
            WATER_INDEX,
        )
    }
}
