use raylib::prelude::*;

/// Albedo tipico de un material mate. Nada real devuelve toda la luz que le
/// llega: la nieve fresca ronda 0.9 y el asfalto 0.1, asi que quedarse un
/// poco por debajo de 1 ya se ve mas creible que el color plano de antes.
pub const DEFAULT_ALBEDO: f32 = 0.85;

/// Indice de refraccion del aire: la luz no se dobla.
pub const AIR_INDEX: f32 = 1.0;
/// El del agua.
pub const WATER_INDEX: f32 = 1.33;
/// El del hielo, apenas por debajo del agua.
pub const ICE_INDEX: f32 = 1.31;
/// El del vidrio comun.
pub const GLASS_INDEX: f32 = 1.52;
/// El del diamante. Es el mas alto de la escena y por eso el que mas dobla
/// lo que se ve a traves suyo.
pub const GEM_INDEX: f32 = 2.42;

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
    /// Cuanta luz emite por su cuenta, en las mismas unidades que la que
    /// recibe: 0 es un cuerpo comun y 1 se ve como si le diera el sol de
    /// lleno aunque este en la sombra.
    ///
    /// Lo que emite sale con su propio color y su propia textura, asi que en
    /// una lampara marina se prenden los cristales y no el marco.
    pub emission: f32,
    /// Si es un metal.
    ///
    /// Es la diferencia fisica entre el oro y un plastico amarillo: en un
    /// dielectrico la luz que rebota en la superficie sale con el color de
    /// la luz, y el color propio lo pone la que se metio adentro y volvio.
    /// En un metal no hay adentro: la luz rebota en el mar de electrones y
    /// sale ya tenida. Por eso el brillo de una chapa de oro es dorado y no
    /// blanco, y por eso un espejo de oro devuelve el mundo dorado.
    pub metallic: bool,
}

impl Material {
    pub fn new(diffuse: Color, albedo: Albedo, shininess: f32) -> Self {
        Material {
            diffuse,
            albedo,
            shininess: shininess.max(1.0),
            refractive_index: AIR_INDEX,
            texture: PLAIN_TEXTURE,
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
            ..Material::new(diffuse, Albedo::new(0.35, 0.12, 0.06, 0.0), 60.0)
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

    /// Metal pulido: poco color propio, mucha imagen del entorno, y esa
    /// imagen y su brillo tenidos con su propio color.
    pub fn metal(diffuse: Color) -> Self {
        Material {
            metallic: true,
            ..Material::new(diffuse, Albedo::new(0.08, 0.35, 0.55, 0.0), 320.0)
        }
    }

    /// Piedra lisa, terracota, cuarzo: un barniz que devuelve algo del
    /// entorno sin llegar a espejo.
    pub fn polished(diffuse: Color) -> Self {
        Material::new(diffuse, Albedo::new(0.7, 0.18, 0.12, 0.0), 110.0)
    }

    /// Laqueados: prismarina, lapislazuli, amatista. Mas brillo que el
    /// barniz y un reflejo apenas mas nitido.
    pub fn lacquer(diffuse: Color) -> Self {
        Material::new(diffuse, Albedo::new(0.64, 0.22, 0.14, 0.0), 150.0)
    }

    /// Gema tallada: refleja en las caras y deja pasar el resto, doblandolo
    /// mucho. Es el indice de refraccion alto lo que hace que un diamante se
    /// vea como un diamante y no como un vidrio celeste.
    pub fn gem(diffuse: Color) -> Self {
        Material::transparent(
            diffuse,
            Albedo::new(0.22, 0.3, 0.28, 0.2),
            220.0,
            GEM_INDEX,
        )
    }

    /// Vidrio: casi todo lo que le llega lo deja pasar.
    pub fn glass(diffuse: Color) -> Self {
        Material::transparent(
            diffuse,
            Albedo::new(0.05, 0.25, 0.1, 0.6),
            300.0,
            GLASS_INDEX,
        )
    }

    /// Hielo: entre el agua y el vidrio, con mas color propio que los dos
    /// porque no es transparente sino translucido.
    pub fn ice(diffuse: Color) -> Self {
        Material::transparent(
            diffuse,
            Albedo::new(0.3, 0.22, 0.28, 0.2),
            180.0,
            ICE_INDEX,
        )
    }

    /// Obsidiana: vidrio volcanico. Negro casi puro, y justamente por eso lo
    /// unico que se le ve es lo que refleja.
    pub fn obsidian(diffuse: Color) -> Self {
        Material::new(diffuse, Albedo::new(0.3, 0.25, 0.4, 0.0), 260.0)
    }

    /// Agua: el unico material de la escena que deja pasar la luz.
    ///
    /// Lo que mas pesa es el reflejo, porque es lo que se ve de una lamina de
    /// agua quieta. El color propio queda bajo, pero no en cero: es lo que
    /// evita que una laguna con el fondo en sombra salga negra.
    ///
    /// Los cuatro pesos juntos rondan 1 a proposito. Antes sumaban casi 1.5,
    /// o sea que el agua devolvia mas luz de la que le llegaba, y a pleno sol
    /// salia una chapa blanca en vez de una laguna.
    pub fn water(diffuse: Color) -> Self {
        Material::transparent(
            diffuse,
            Albedo::new(0.15, 0.25, 0.4, 0.25),
            160.0,
            WATER_INDEX,
        )
    }
}
