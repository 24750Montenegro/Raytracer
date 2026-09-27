// terrain.rs
// Cuatro islas flotantes generadas con ruido, una por bioma.
//
// Antes era un plano macizo de 48x48 partido en cuadrantes. El problema de
// esa forma es que no tiene silueta: mirando de costado solo se ve un ladrillo
// y todo el relieve queda del lado de arriba. Una isla flotante, en cambio,
// se lee entera desde cualquier angulo, porque tiene borde, tiene panza y
// tiene lo que cuelga de la panza.
//
// Cada isla es un blob de contorno irregular con:
//   - una tapa con el relieve de su bioma,
//   - una quilla que se hunde hacia el centro, como un cono invertido,
//   - estalactitas de roca que caen de esa quilla,
//   - su propia altura base, para que las cuatro no queden alineadas.
//
// La tapa pasa por cuatro pasos, y cada uno arregla algo que el anterior no
// puede dar solo:
//
//   1. El dominio se deforma (`Noise::warp`). El ruido de Perlin a secas
//      tiene curvas de nivel redondas y parejas, y un terreno hecho asi se
//      lee enseguida como lomas de algodon.
//   2. Una curva (`Curve`) convierte ese campo en altura. Escalarlo derecho
//      reparte las alturas parejo y el mapa termina sin llanura y sin
//      montana: son lomas de media altura de punta a punta. La curva gasta
//      medio rango del ruido en el primer quinto de altura, y de ahi sube
//      en serio.
//   3. Se le suman crestas de ruido plegado (`Noise::ridged`), pero pesadas
//      por la altura ya decidida, asi la sierra se afila solo donde hay
//      sierra.
//   4. El campo se erosiona (`erode`). El ruido no tiene historia y deja
//      agujas de una columna que ningun derrumbe habria dejado en pie.
//
// Solo se generan los bloques que dan al aire, y de cada uno solo las
// cajitas del modelo 3D que corresponden a las caras destapadas: lo que no
// se ve no entra al BVH.
use raylib::prelude::*;

use crate::material::Material;
use crate::model::{BlockModel, Face, Faces};
use crate::noise::Noise;
use crate::object::Object;
use crate::texture::{Recipe, Texture};

/// Lado de la celda que ocupa cada isla, en bloques.
///
/// Una isla chica no da lugar a que convivan un llano y una montana: entre
/// que el terreno arranca en la orilla y termina en la orilla de enfrente
/// hay veinte columnas, y en veinte columnas o hay una cosa o hay la otra.
pub const ISLAND: usize = 44;
/// Cielo entre una celda y la de al lado.
pub const GAP: usize = 14;
/// Aire de sobra en el borde del mapa. El contorno de una isla lo ondula el
/// ruido, asi que puede pasarse de su celda; sin margen, ese pedazo saldria
/// cortado al ras contra el borde de la grilla y se veria como un tajo.
const MARGIN: usize = 5;
/// Lado del mapa entero: las islas van en una grilla de 2x2.
pub const SIZE: usize = ISLAND * 2 + GAP + MARGIN * 2;

/// Lado de cada bloque. Van pegados uno al lado del otro: con separacion se
/// colaria el cielo por las junturas, porque lo tapado no se genera.
pub const BLOCK: f32 = 1.0;

/// Radio de una isla antes de que el ruido le deforme el contorno.
const RADIUS: f32 = ISLAND as f32 * 0.5 - 0.5;
/// Cuanto le come o le agrega el ruido a ese radio, como fraccion.
const SHORE_WOBBLE: f32 = 0.22;
/// Cada cuantos bloques ondula el contorno. Va en proporcion a la isla: con
/// una escala fija, una isla mas grande no sale con lobulos mas grandes,
/// sale con el mismo borde crespo repetido mas veces.
const SHORE_SCALE: f32 = ISLAND as f32 / 4.5;
/// Pasadas de limpieza de la costa, ver `trim_shore`.
const SHORE_PASSES: usize = 2;

/// Pasadas de erosion sobre el campo de alturas, y cuanto pesa la columna
/// contra las ocho de alrededor en cada una. Ver `erode`.
const EROSION_PASSES: usize = 2;
const EROSION_CENTER: f32 = 3.0;

/// Espesor de la costra en el borde mismo de la isla.
const CRUST: f32 = 4.0;
/// Cuanto mas baja la quilla en el centro que en el borde.
const KEEL: f32 = 30.0;
/// Curva de esa quilla. Por encima de 1 mantiene el borde finito y hunde el
/// centro de golpe, que es la forma de cono invertido que se quiere.
const KEEL_CURVE: f32 = 1.7;

/// A partir de que valor del campo de goteo empieza a colgar una estalactita,
/// y cuantos bloques baja por cada punto que ese campo pasa el umbral.
///
/// El umbral va alto a proposito: el ruido se pasa de ahi en manchas chicas
/// y sueltas, que es lo que hace que de la panza cuelguen agujas y no que la
/// panza entera baje un escalon.
const DRIP_START: f32 = 0.3;
const DRIP_REACH: f32 = 110.0;
const DRIP_SCALE: f32 = 6.0;

/// Cuantos bloques por encima de la base de la isla llega su laguna.
const SEA_OFFSET: i32 = 3;
/// Cuantos bloques por encima del agua sigue habiendo arena.
const BEACH_BAND: i32 = 1;
/// Cuanto por encima de la base de la isla empieza la nieve.
const SNOW_OFFSET: i32 = 21;

/// Bloques de relleno debajo de la superficie antes de la roca.
const SUBSOIL: i32 = 3;
/// Los ultimos bloques de la quilla son roca profunda, que es lo que se ve
/// desde abajo.
const BEDROCK_BAND: i32 = 3;

const PATCH_SCALE: f32 = 9.0;

/// Que tan seguido aparece cada veta en la roca, acumulado. Las que valen
/// mas son mas raras, como corresponde.
const VEIN_DIAMOND: f32 = 0.004;
const VEIN_EMERALD: f32 = 0.008;
const VEIN_GOLD: f32 = 0.018;
const VEIN_LAPIS: f32 = 0.03;
const VEIN_REDSTONE: f32 = 0.05;
const VEIN_COPPER: f32 = 0.08;
const VEIN_IRON: f32 = 0.115;
const VEIN_COAL: f32 = 0.175;

const TREE_CHANCE: f32 = 0.12;
const TREE_SPACING: i32 = 3;
const TRUNK_HEIGHT: i32 = 4;

const CACTUS_CHANCE: f32 = 0.07;
const CACTUS_SPACING: i32 = 4;

const SPIKE_CHANCE: f32 = 0.035;
const SPIKE_SPACING: i32 = 8;

const BOULDER_CHANCE: f32 = 0.05;

/// Las lamparas marinas de la isla helada: pocas y bien separadas. Cada una
/// no es solo un bloque que brilla, es una luz mas que la escena tiene que
/// consultar en cada punto que sombrea, asi que conviene que se cuenten con
/// los dedos.
const LAMP_CHANCE: f32 = 0.02;
const LAMP_SPACING: i32 = 15;
/// Cuanta luz emite por su cuenta la cara del bloque.
const LAMP_GLOW: f32 = 2.0;
/// Cuanto se levanta la luz por encima de la cara de arriba del bloque. Si
/// quedara adentro, el propio bloque le taparia la luz a todo lo que tiene
/// alrededor.
pub const LAMP_LIFT: f32 = 0.35;
const BOULDER_SPACING: i32 = 5;

/// Que tan adentro del contorno tiene que estar una columna para que le
/// crezca algo. Lo que crece asoma un bloque para los costados y no puede
/// quedar colgando en el aire.
const INLAND: f32 = 0.18;

/// Cuanto varia el tono de un bloque al siguiente. En Minecraft esa variacion
/// la da la textura; sin ella una pradera se ve como una chapa de un color.
const COLOR_JITTER: f32 = 0.07;

/// Semillas de los campos derivados: cualquier numero distinto sirve.
const PATCH_SALT: u64 = 0x1ce;
const GRAIN_SALT: u64 = 0x9ea1;
const SHORE_SALT: u64 = 0xb0d3;
const DRIP_SALT: u64 = 0xd819;
/// Las texturas no dependen de la semilla del terreno: son siempre estas.
const TEXTURE_SEED: u64 = 0x7e_7c_00;

/// Cuanto se exagera el relieve de los mapas normales.
const RELIEF_BOOST: f32 = 1.6;

/// Cuanto se hunde el relieve dibujado de cada bloque, como fraccion del
/// lado de la cara.
///
/// Ahora que el pack pone geometria de verdad, el paralaje pasa a ser el
/// detalle fino que queda entre cajita y cajita, y va mucho mas bajo que
/// antes: los dos relieves sumados se peleaban y el dibujo nadaba al girar.
const DEPTH_FLAT: f32 = 0.0;
const DEPTH_ROCK: f32 = 0.07;
const DEPTH_COARSE: f32 = 0.1;

/// El pasto y las hojas vienen en gris en el pack, como en el juego, que les
/// pone el color del bioma al dibujarlas.
const GRASS_TINT: Color = Color { r: 145, g: 189, b: 89, a: 255 };
const FOLIAGE_TINT: Color = Color { r: 119, g: 171, b: 47, a: 255 };
const PINE_TINT: Color = Color { r: 97, g: 140, b: 96, a: 255 };
const PLAIN: Color = Color { r: 255, g: 255, b: 255, a: 255 };

/// Alturas de sorteo para separar los hashes que comparten el campo `grain`.
const DRAW_TREE: i32 = -1;
const DRAW_CACTUS: i32 = -2;
const DRAW_SPIKE: i32 = -3;
const DRAW_BOULDER: i32 = -4;
const DRAW_LAMP: i32 = -5;
const DRAW_VEIN: i32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Biome {
    /// Pasto, bosque y cimas nevadas.
    Forest,
    Desert,
    Ice,
    /// Roca partida con vetas a la vista, como una mina a cielo abierto.
    Mines,
}

/// Una isla: donde cae, a que altura flota y de que bioma es.
#[derive(Debug, Clone, Copy)]
pub struct Island {
    pub biome: Biome,
    /// Centro en la grilla de columnas.
    center: (f32, f32),
    /// Cuantos niveles por encima del cero flota. Las cuatro van a alturas
    /// distintas para que el conjunto no se lea como un piso partido.
    base: i32,
}

/// Donde cae el centro de la celda de la izquierda y de la de la derecha.
const NEAR: f32 = MARGIN as f32 + ISLAND as f32 * 0.5;
const FAR: f32 = NEAR + (ISLAND + GAP) as f32;

pub const ISLANDS: [Island; 4] = [
    Island {
        biome: Biome::Forest,
        center: (NEAR, NEAR),
        base: 4,
    },
    Island {
        biome: Biome::Desert,
        center: (FAR, NEAR),
        base: 7,
    },
    Island {
        biome: Biome::Ice,
        center: (NEAR, FAR),
        base: -3,
    },
    Island {
        biome: Biome::Mines,
        center: (FAR, FAR),
        base: 0,
    },
];

impl Island {
    /// Hasta donde llega su laguna.
    fn sea_level(self) -> i32 {
        self.base + SEA_OFFSET
    }

    fn snow_line(self) -> i32 {
        self.base + SNOW_OFFSET
    }

    /// El bloque que corona su monumento: el material con el que cada isla
    /// muestra lo que sabe hacer el trazador.
    fn landmark(self) -> BlockKind {
        match self.biome {
            Biome::Forest => BlockKind::EmeraldBlock,
            Biome::Desert => BlockKind::GoldBlock,
            Biome::Ice => BlockKind::DiamondBlock,
            Biome::Mines => BlockKind::CopperBlock,
        }
    }
}

/// Curva que convierte el campo de ruido en altura.
///
/// Es la pieza que da llanuras y cordilleras a la vez, y con un solo campo
/// de ruido. Escalar el ruido derecho reparte las alturas parejo, y un mapa
/// asi no tiene ni llano ni montana: tiene lomas de media altura de punta a
/// punta, que es como se veia el terreno antes.
///
/// La salida es la fraccion del `span` del bioma, de 0 a 1. Los puntos de
/// control se eligieron contra como se reparte de verdad el fbm, que no es
/// parejo: mide -0.31 en el percentil 5, 0 en la mediana y 0.31 en el 95, y
/// casi nunca pasa de 0.7. Asi que gastar el tramo de -0.70 a 0.06 (dos
/// tercios de las columnas) en el primer 13% de altura deja dos tercios del
/// mapa hecho llanura, y meter el resto de la altura entre 0.06 y 0.45 deja
/// el tercio que queda subiendo en serio.
///
/// Es la misma idea con la que Minecraft separa pradera de cordillera desde
/// la 1.18.
struct Curve(&'static [(f32, f32)]);

impl Curve {
    /// Interpola la curva en ese valor del campo. Fuera de los extremos se
    /// queda en el ultimo punto: el ruido no llega ahi casi nunca, y cuando
    /// llega lo que corresponde es meseta, no extrapolar al infinito.
    fn at(&self, value: f32) -> f32 {
        let points = self.0;

        for pair in points.windows(2) {
            let ((from_x, from_y), (to_x, to_y)) = (pair[0], pair[1]);
            if value <= to_x {
                if value <= from_x {
                    return from_y;
                }
                let t = (value - from_x) / (to_x - from_x);
                return from_y + (to_y - from_y) * t;
            }
        }

        points[points.len() - 1].1
    }
}

/// Como lee cada bioma el campo de relieve.
struct Profile {
    /// Cuantos bloques mide un accidente del terreno.
    scale: f32,
    octaves: u32,
    /// Cuanto se deforma el dominio antes de leer el campo, en celdas del
    /// ruido. Es lo que dobla las curvas de nivel y les saca la cara de
    /// burbuja.
    warp: f32,
    /// De campo de ruido a altura: donde estan el llano y la montana.
    curve: Curve,
    /// Altura minima del bioma sobre la base de su isla, y cuanto puede
    /// subir por encima de ella.
    floor: f32,
    span: f32,
    /// Cuanto levantan las crestas por encima de la montana ya formada, en
    /// bloques. Van solo arriba: una sierra tiene filos, una llanura no.
    peaks: f32,
    /// Cada cuantos bloques se repite una cresta.
    ridge_scale: f32,
}

/// Bosque: pradera ancha con alguna laguna, y una cordillera nevada al
/// fondo. El tramo de -0.05 a 0.07, que es casi la mitad de las columnas,
/// sube apenas cuatro centesimos: eso es la pradera.
const FOREST_CURVE: Curve = Curve(&[
    (-0.55, 0.00),
    (-0.28, 0.04),
    (-0.18, 0.12),
    (-0.05, 0.18),
    (0.06, 0.22),
    (0.14, 0.38),
    (0.24, 0.62),
    (0.36, 0.88),
    (0.50, 1.00),
]);

/// Desierto: llano casi entero y casi todo seco, con mesetas. El tramo de
/// 0.16 a 0.28 es el faldon que sube a la meseta, y el de 0.28 a 0.36, la
/// meseta misma.
const DESERT_CURVE: Curve = Curve(&[
    (-0.60, 0.00),
    (-0.30, 0.05),
    (-0.14, 0.14),
    (0.06, 0.20),
    (0.12, 0.24),
    (0.22, 0.46),
    (0.28, 0.54),
    (0.38, 0.80),
    (0.48, 1.00),
]);

/// Hielo: llanura helada, lagunas congeladas y de golpe un macizo.
const ICE_CURVE: Curve = Curve(&[
    (-0.55, 0.00),
    (-0.28, 0.03),
    (-0.17, 0.09),
    (-0.02, 0.16),
    (0.08, 0.22),
    (0.21, 0.50),
    (0.34, 0.80),
    (0.50, 1.00),
]);

/// Cantera: el suelo de la mina es plano, y lo que sobra son los cerros que
/// no se excavaron.
const MINES_CURVE: Curve = Curve(&[
    (-0.55, 0.00),
    (-0.30, 0.04),
    (-0.19, 0.11),
    (-0.02, 0.16),
    (0.06, 0.24),
    (0.22, 0.54),
    (0.38, 0.84),
    (0.54, 1.00),
]);

impl Biome {
    fn profile(self) -> Profile {
        match self {
            Biome::Forest => Profile {
                scale: 17.0,
                octaves: 4,
                warp: 0.55,
                curve: FOREST_CURVE,
                floor: 0.0,
                span: 33.0,
                peaks: 4.0,
                ridge_scale: 17.0,
            },
            Biome::Desert => Profile {
                scale: 21.0,
                octaves: 3,
                warp: 0.7,
                curve: DESERT_CURVE,
                floor: 2.0,
                span: 33.0,
                peaks: 1.5,
                ridge_scale: 19.0,
            },
            Biome::Ice => Profile {
                scale: 18.0,
                octaves: 4,
                warp: 0.5,
                curve: ICE_CURVE,
                floor: 1.0,
                span: 26.0,
                peaks: 3.0,
                ridge_scale: 16.0,
            },
            Biome::Mines => Profile {
                scale: 15.0,
                octaves: 4,
                warp: 0.8,
                curve: MINES_CURVE,
                floor: 0.0,
                span: 30.0,
                peaks: 7.0,
                ridge_scale: 15.0,
            },
        }
    }

    /// Altura que le da este bioma a la columna por encima de la base de su
    /// isla, sin redondear.
    ///
    /// Son tres pasos. Primero se deforma el dominio, que es lo que le saca
    /// al ruido su aire de burbujas parejas. Despues la curva reparte ese
    /// campo en llanura y montana. Y por ultimo se le suman crestas, pero
    /// multiplicadas por la altura ya decidida: asi la sierra se afila solo
    /// donde hay sierra y el llano se queda llano.
    fn height_at(self, relief: &Noise, x: usize, z: usize) -> f32 {
        let profile = self.profile();
        let (x, z) = (x as f32 / profile.scale, z as f32 / profile.scale);
        let (x, z) = relief.warp(x, z, profile.warp);

        let land = profile.curve.at(relief.fbm(x, z, profile.octaves));

        // al cuadrado, y no derecho, para que las crestas no asomen ya en la
        // primera loma: a media altura pesan un cuarto, no la mitad
        let crest = relief.ridged(
            x * profile.scale / profile.ridge_scale,
            z * profile.scale / profile.ridge_scale,
            3,
        );

        profile.floor + land * profile.span + land * land * crest * profile.peaks
    }
}

/// Como reparte la luz una superficie, agrupado por lo que es en la vida
/// real. Es lo que decide si un bloque se ve como tiza, como roca mojada o
/// como un espejo, y de paso lo que hace que valga la pena trazar rayos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Finish {
    /// Tierra, arena, grava, musgo: rebotan casi todo parejo y no brillan.
    Soil,
    /// Hojas: lo mismo, pero mas claras y con un brillo de cera apenas.
    Foliage,
    /// Nieve, arcilla, caliza, cuarzo: claras y mates.
    Chalk,
    /// Piedra partida, adoquin, basalto: mate con un brillo muy chico.
    Rock,
    Wood,
    /// Piedra lisa, terracota, ladrillos: barnizadas, devuelven algo.
    Polished,
    /// Prismarina, lapis, amatista pulida: laqueadas.
    Lacquer,
    /// Hierro, oro, cobre, netherita. El unico grupo que tine su propio
    /// reflejo: un espejo de oro devuelve el mundo dorado, y eso no sale de
    /// un especular blanco.
    Metal,
    /// El metal que perdio el espejo: cobre oxidado.
    Patina,
    /// Diamante y esmeralda: reflejan y dejan pasar algo de luz.
    Gem,
    Glass,
    Ice,
    Water,
    /// Vidrio volcanico: negro y muy pulido, casi todo reflejo.
    Obsidian,
    /// El unico que no espera a que le llegue luz: la pone el.
    Lamp,
}

impl Finish {
    /// El material con el que se sombrea, ya tenido.
    ///
    /// El tinte no es el color del bloque: ese vive en la textura. Es un gris
    /// que lo oscurece un poco, para que dos bloques pegados del mismo tipo
    /// no se lean como una sola chapa.
    fn material(self, tint: Color) -> Material {
        match self {
            Finish::Soil => Material::matte(tint),
            Finish::Foliage => Material::new(tint, crate::material::Albedo::new(0.9, 0.05, 0.0, 0.0), 12.0),
            Finish::Chalk => Material::new(tint, crate::material::Albedo::new(0.9, 0.04, 0.0, 0.0), 8.0),
            Finish::Rock => Material::new(tint, crate::material::Albedo::new(0.84, 0.06, 0.02, 0.0), 18.0),
            Finish::Wood => Material::new(tint, crate::material::Albedo::new(0.84, 0.1, 0.02, 0.0), 26.0),
            Finish::Polished => Material::polished(tint),
            Finish::Lacquer => Material::lacquer(tint),
            Finish::Lamp => Material::lamp(tint, LAMP_GLOW),
            Finish::Metal => Material::metal(tint),
            Finish::Patina => Material::new(tint, crate::material::Albedo::new(0.78, 0.1, 0.06, 0.0), 40.0),
            Finish::Gem => Material::gem(tint),
            Finish::Glass => Material::glass(tint),
            Finish::Ice => Material::ice(tint),
            Finish::Water => Material::water(tint),
            Finish::Obsidian => Material::obsidian(tint),
        }
    }

    /// Los que devuelven una imagen no llevan relieve: el mapa normal
    /// rompe el reflejo en pedazos que apuntan cada uno para su lado.
    fn reflective(self) -> bool {
        matches!(
            self,
            Finish::Metal | Finish::Gem | Finish::Glass | Finish::Ice | Finish::Water | Finish::Obsidian
        )
    }
}

/// Todo lo que define un bloque, en una fila. Agregar un bloque es agregar
/// una fila: antes habia que tocar cinco `match` distintos y era cuestion de
/// tiempo que uno quedara desfasado.
pub struct Spec {
    /// Archivo de `assets/blocks`, y con que color se tine al cargarlo. El
    /// juego guarda el pasto y las hojas en gris y les pone el color del
    /// bioma al dibujarlos; el resto ya viene pintado.
    ///
    /// None es para lo que el pack de texturas no trae, como el agua: esa se
    /// dibuja aca con la receta.
    pub texture: Option<(&'static str, Color)>,
    /// Modelo 3D del pack Classic 3D, en `assets/models`. None deja el cubo
    /// liso, que es lo que el pack hace con la mayoria de los bloques
    /// naturales.
    pub model: Option<&'static str>,
    pub finish: Finish,
    /// Color de respaldo, para la receta.
    pub color: Color,
    /// Como se dibuja si hay que dibujarla.
    pub recipe: Recipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum BlockKind {
    Water,
    Ice,
    PackedIce,
    Snow,
    Sand,
    RedSand,
    Sandstone,
    RedSandstone,
    CutSandstone,
    Terracotta,
    Grass,
    Dirt,
    CoarseDirt,
    Mud,
    Clay,
    Moss,
    Gravel,
    Stone,
    Cobblestone,
    MossyCobblestone,
    StoneBricks,
    SmoothStone,
    Andesite,
    Granite,
    Diorite,
    Deepslate,
    DeepslateBricks,
    Tuff,
    Basalt,
    Calcite,
    Quartz,
    CoalOre,
    IronOre,
    GoldOre,
    CopperOre,
    RedstoneOre,
    LapisOre,
    DiamondOre,
    EmeraldOre,
    IronBlock,
    GoldBlock,
    CopperBlock,
    OxidizedCopper,
    NetheriteBlock,
    DiamondBlock,
    EmeraldBlock,
    AmethystBlock,
    Prismarine,
    SeaLantern,
    Obsidian,
    Glass,
    Magma,
    Wood,
    SpruceLog,
    Planks,
    Leaves,
    SpruceLeaves,
    Cactus,
}

impl BlockKind {
    /// Todos, en el mismo orden en el que estan declarados: ese orden es su
    /// lugar en las tablas de texturas y de modelos de la escena.
    pub const ALL: [BlockKind; 58] = [
        BlockKind::Water,
        BlockKind::Ice,
        BlockKind::PackedIce,
        BlockKind::Snow,
        BlockKind::Sand,
        BlockKind::RedSand,
        BlockKind::Sandstone,
        BlockKind::RedSandstone,
        BlockKind::CutSandstone,
        BlockKind::Terracotta,
        BlockKind::Grass,
        BlockKind::Dirt,
        BlockKind::CoarseDirt,
        BlockKind::Mud,
        BlockKind::Clay,
        BlockKind::Moss,
        BlockKind::Gravel,
        BlockKind::Stone,
        BlockKind::Cobblestone,
        BlockKind::MossyCobblestone,
        BlockKind::StoneBricks,
        BlockKind::SmoothStone,
        BlockKind::Andesite,
        BlockKind::Granite,
        BlockKind::Diorite,
        BlockKind::Deepslate,
        BlockKind::DeepslateBricks,
        BlockKind::Tuff,
        BlockKind::Basalt,
        BlockKind::Calcite,
        BlockKind::Quartz,
        BlockKind::CoalOre,
        BlockKind::IronOre,
        BlockKind::GoldOre,
        BlockKind::CopperOre,
        BlockKind::RedstoneOre,
        BlockKind::LapisOre,
        BlockKind::DiamondOre,
        BlockKind::EmeraldOre,
        BlockKind::IronBlock,
        BlockKind::GoldBlock,
        BlockKind::CopperBlock,
        BlockKind::OxidizedCopper,
        BlockKind::NetheriteBlock,
        BlockKind::DiamondBlock,
        BlockKind::EmeraldBlock,
        BlockKind::AmethystBlock,
        BlockKind::Prismarine,
        BlockKind::SeaLantern,
        BlockKind::Obsidian,
        BlockKind::Glass,
        BlockKind::Magma,
        BlockKind::Wood,
        BlockKind::SpruceLog,
        BlockKind::Planks,
        BlockKind::Leaves,
        BlockKind::SpruceLeaves,
        BlockKind::Cactus,
    ];

    /// Su lugar en las tablas de la escena. La textura lisa ocupa la 0, asi
    /// que todo se corre uno.
    pub fn slot(self) -> u16 {
        self as u16 + 1
    }

    /// La fila de este bloque.
    ///
    /// El grano de la receta solo se usa cuando la textura del pack no esta:
    /// es el plan de emergencia para no quedarse sin escena por un PNG que
    /// falta.
    pub fn spec(self) -> Spec {
        use BlockKind::*;


        let rock = || Recipe::new(1.5, 0.16, 0.6, 0.4);
        let row = |texture: Option<(&'static str, Color)>,
                   model: Option<&'static str>,
                   finish: Finish,
                   color: Color,
                   recipe: Recipe| Spec {
            texture,
            model,
            finish,
            color,
            recipe,
        };
        let plain = |file: &'static str| Some((file, PLAIN));

        match self {
            // -- agua, hielo y nieve ------------------------------------
            Water => row(None, None, Finish::Water, rgb(58, 116, 184), Recipe::new(5.0, 0.05, 0.35, 0.0)),
            Ice => row(plain("ice"), None, Finish::Ice, rgb(151, 192, 221), Recipe::new(4.5, 0.08, 0.3, 0.15)),
            PackedIce => row(plain("packed_ice"), None, Finish::Ice, rgb(141, 180, 214), Recipe::new(4.0, 0.08, 0.3, 0.2)),
            Snow => row(plain("snow"), None, Finish::Chalk, rgb(236, 242, 248), Recipe::new(3.0, 0.05, 0.25, 0.5)),

            // -- arenas y arcillas -------------------------------------
            Sand => row(plain("sand"), None, Finish::Soil, rgb(214, 198, 142), Recipe::new(1.3, 0.12, 0.28, 0.55)),
            RedSand => row(plain("red_sand"), None, Finish::Soil, rgb(190, 102, 33), Recipe::new(1.3, 0.12, 0.28, 0.55)),
            Sandstone => row(plain("sandstone"), Some("sandstone"), Finish::Rock, rgb(198, 176, 118), Recipe::new(1.2, 0.14, 0.45, 0.3).stretched(7.0, 1.1)),
            RedSandstone => row(plain("red_sandstone"), Some("red_sandstone"), Finish::Rock, rgb(178, 99, 40), Recipe::new(1.2, 0.14, 0.45, 0.3).stretched(7.0, 1.1)),
            CutSandstone => row(plain("cut_sandstone"), Some("cut_sandstone"), Finish::Polished, rgb(202, 182, 126), Recipe::new(1.2, 0.1, 0.4, 0.2)),
            Terracotta => row(plain("terracotta"), None, Finish::Polished, rgb(152, 94, 67), Recipe::new(2.0, 0.1, 0.3, 0.3)),

            // -- suelos vivos ------------------------------------------
            Grass => row(Some(("grass_block_top", GRASS_TINT)), None, Finish::Soil, rgb(96, 154, 66), Recipe::new(2.0, 0.22, 0.6, 0.4)),
            Dirt => row(plain("dirt"), None, Finish::Soil, rgb(133, 96, 66), Recipe::new(1.8, 0.26, 0.75, 0.45)),
            CoarseDirt => row(plain("coarse_dirt"), None, Finish::Soil, rgb(119, 85, 59), Recipe::new(1.8, 0.3, 0.85, 0.5)),
            Mud => row(plain("mud"), None, Finish::Polished, rgb(60, 50, 48), Recipe::new(2.2, 0.14, 0.4, 0.3)),
            Clay => row(plain("clay"), None, Finish::Chalk, rgb(160, 166, 179), Recipe::new(2.4, 0.08, 0.3, 0.4)),
            Moss => row(plain("moss_block"), None, Finish::Soil, rgb(89, 109, 45), Recipe::new(2.2, 0.28, 0.8, 0.5)),

            // -- roca ---------------------------------------------------
            Gravel => row(plain("gravel"), None, Finish::Soil, rgb(150, 144, 136), Recipe::new(2.2, 0.3, 1.1, 0.5)),
            Stone => row(plain("stone"), None, Finish::Rock, rgb(140, 142, 146), rock()),
            Cobblestone => row(plain("cobblestone"), Some("cobblestone"), Finish::Rock, rgb(134, 134, 134), Recipe::new(2.4, 0.3, 1.2, 0.5)),
            MossyCobblestone => row(plain("mossy_cobblestone"), Some("mossy_cobblestone"), Finish::Rock, rgb(114, 128, 104), Recipe::new(2.4, 0.3, 1.2, 0.5)),
            StoneBricks => row(plain("stone_bricks"), Some("stone_bricks"), Finish::Rock, rgb(122, 122, 122), Recipe::new(1.6, 0.14, 0.7, 0.3)),
            SmoothStone => row(plain("smooth_stone"), Some("smooth_stone"), Finish::Polished, rgb(158, 158, 158), Recipe::new(3.0, 0.06, 0.2, 0.2)),
            Andesite => row(plain("andesite"), None, Finish::Rock, rgb(136, 136, 138), rock()),
            Granite => row(plain("granite"), None, Finish::Rock, rgb(149, 103, 86), rock()),
            Diorite => row(plain("diorite"), None, Finish::Rock, rgb(188, 188, 190), rock()),
            Deepslate => row(plain("deepslate"), None, Finish::Rock, rgb(80, 80, 85), rock().stretched(1.2, 6.0)),
            DeepslateBricks => row(plain("deepslate_bricks"), Some("deepslate_bricks"), Finish::Rock, rgb(72, 72, 76), Recipe::new(1.6, 0.14, 0.7, 0.3)),
            Tuff => row(plain("tuff"), None, Finish::Rock, rgb(108, 109, 102), rock()),
            Basalt => row(plain("basalt_side"), None, Finish::Rock, rgb(80, 80, 85), rock().stretched(1.1, 6.0)),
            Calcite => row(plain("calcite"), None, Finish::Chalk, rgb(223, 223, 216), Recipe::new(2.6, 0.06, 0.3, 0.4)),
            Quartz => row(plain("quartz_block_side"), None, Finish::Polished, rgb(236, 230, 222), Recipe::new(3.0, 0.05, 0.2, 0.3)),

            // -- vetas --------------------------------------------------
            CoalOre => row(plain("coal_ore"), Some("coal_ore"), Finish::Rock, rgb(140, 142, 146), rock().flecked(rgb(62, 62, 66), 0.26, 0.5)),
            IronOre => row(plain("iron_ore"), Some("iron_ore"), Finish::Rock, rgb(140, 142, 146), rock().flecked(rgb(196, 150, 112), 0.26, 0.5)),
            GoldOre => row(plain("gold_ore"), Some("gold_ore"), Finish::Rock, rgb(140, 142, 146), rock().flecked(rgb(226, 184, 74), 0.26, 0.5)),
            CopperOre => row(plain("copper_ore"), Some("copper_ore"), Finish::Rock, rgb(140, 142, 146), rock().flecked(rgb(160, 120, 90), 0.26, 0.5)),
            RedstoneOre => row(plain("redstone_ore"), Some("redstone_ore"), Finish::Rock, rgb(140, 142, 146), rock().flecked(rgb(190, 40, 30), 0.24, 0.5)),
            LapisOre => row(plain("lapis_ore"), Some("lapis_ore"), Finish::Rock, rgb(140, 142, 146), rock().flecked(rgb(40, 76, 160), 0.24, 0.5)),
            DiamondOre => row(plain("diamond_ore"), Some("diamond_ore"), Finish::Rock, rgb(140, 142, 146), rock().flecked(rgb(110, 220, 220), 0.22, 0.5)),
            EmeraldOre => row(plain("emerald_ore"), Some("emerald_ore"), Finish::Rock, rgb(140, 142, 146), rock().flecked(rgb(60, 190, 90), 0.22, 0.5)),

            // -- lo que refleja -----------------------------------------
            IronBlock => row(plain("iron_block"), Some("iron_block"), Finish::Metal, rgb(220, 220, 220), Recipe::new(4.0, 0.05, 0.2, 0.2)),
            GoldBlock => row(plain("gold_block"), Some("gold_block"), Finish::Metal, rgb(247, 212, 76), Recipe::new(4.0, 0.05, 0.2, 0.2)),
            CopperBlock => row(plain("copper_block"), Some("copper_block"), Finish::Metal, rgb(192, 107, 79), Recipe::new(4.0, 0.06, 0.25, 0.2)),
            OxidizedCopper => row(plain("oxidized_copper"), Some("oxidized_copper"), Finish::Patina, rgb(82, 162, 132), Recipe::new(2.4, 0.16, 0.6, 0.4)),
            NetheriteBlock => row(plain("netherite_block"), Some("netherite_block"), Finish::Metal, rgb(66, 60, 62), Recipe::new(3.0, 0.1, 0.4, 0.3)),
            DiamondBlock => row(plain("diamond_block"), Some("diamond_block"), Finish::Gem, rgb(110, 224, 219), Recipe::new(3.0, 0.08, 0.3, 0.25)),
            EmeraldBlock => row(plain("emerald_block"), None, Finish::Gem, rgb(63, 203, 105), Recipe::new(3.0, 0.08, 0.3, 0.25)),
            AmethystBlock => row(plain("amethyst_block"), None, Finish::Lacquer, rgb(134, 96, 190), Recipe::new(2.6, 0.12, 0.4, 0.3)),
            Prismarine => row(plain("prismarine"), None, Finish::Lacquer, rgb(99, 156, 148), Recipe::new(2.6, 0.12, 0.4, 0.35)),
            SeaLantern => row(plain("sea_lantern"), None, Finish::Lamp, rgb(214, 228, 219), Recipe::new(3.0, 0.08, 0.3, 0.3)),
            Obsidian => row(plain("obsidian"), None, Finish::Obsidian, rgb(21, 17, 33), Recipe::new(2.4, 0.12, 0.4, 0.3)),
            Glass => row(plain("glass"), None, Finish::Glass, rgb(222, 238, 244), Recipe::new(6.0, 0.03, 0.1, 0.1)),
            Magma => row(plain("magma"), None, Finish::Rock, rgb(142, 71, 32), Recipe::new(2.2, 0.3, 0.8, 0.4)),

            // -- lo que crece -------------------------------------------
            Wood => row(plain("oak_log"), None, Finish::Wood, rgb(101, 76, 47), Recipe::new(1.0, 0.24, 0.8, 0.25).stretched(1.2, 7.0)),
            SpruceLog => row(plain("spruce_log"), None, Finish::Wood, rgb(70, 50, 28), Recipe::new(1.0, 0.24, 0.8, 0.25).stretched(1.2, 7.0)),
            Planks => row(plain("oak_planks"), Some("oak_planks"), Finish::Wood, rgb(162, 130, 78), Recipe::new(1.2, 0.18, 0.6, 0.2).stretched(1.2, 7.0)),
            Leaves => row(Some(("oak_leaves", FOLIAGE_TINT)), None, Finish::Foliage, rgb(64, 114, 50), Recipe::new(2.6, 0.34, 1.2, 0.45)),
            SpruceLeaves => row(Some(("spruce_leaves", PINE_TINT)), None, Finish::Foliage, rgb(52, 92, 52), Recipe::new(2.6, 0.34, 1.2, 0.45)),
            Cactus => row(plain("cactus_side"), None, Finish::Foliage, rgb(78, 132, 62), Recipe::new(1.0, 0.22, 0.9, 0.2).stretched(1.1, 6.0)),
        }
    }

    /// El material con el que se sombrea, ya apuntando a su textura.
    pub fn material(self, tint: Color) -> Material {
        self.spec().finish.material(tint).textured(self.slot())
    }
}

/// Las texturas de todos los bloques, en el orden de `BlockKind::ALL`. La
/// primera es la lisa, la de los materiales que no llevan textura.
///
/// Las del pack se leen de disco. Si alguna falta, ese bloque cae en la
/// receta procedural en vez de tumbar el programa: prefiero un bloque con
/// otra textura que una escena que no abre.
pub fn block_textures() -> Vec<Texture> {
    let noise = Noise::new(TEXTURE_SEED);
    let mut textures = vec![Texture::plain()];

    for (place, kind) in BlockKind::ALL.iter().enumerate() {
        let spec = kind.spec();
        let loaded = spec
            .texture
            .map(|(file, tint)| Texture::load(file, tint));

        let texture = match loaded {
            Some(Ok(texture)) => texture,
            Some(Err(problem)) => {
                eprintln!("textura de {kind:?}: {problem}");
                Texture::build(spec.color, &spec.recipe, &noise.variant(place as u64))
            }
            // el pack no la trae, va la receta y listo
            None => Texture::build(spec.color, &spec.recipe, &noise.variant(place as u64)),
        };

        let reflective = spec.finish.reflective();
        let relief = if reflective { 0.15 } else { RELIEF_BOOST };
        let depth = match (reflective, spec.model.is_some(), spec.finish) {
            // lo que refleja va liso: el relieve rompe la imagen que devuelve
            (true, _, _) => DEPTH_FLAT,
            // Y donde el pack puso geometria, tambien: el paralaje y las
            // cajitas del modelo son dos formas de contar el mismo relieve,
            // y sumarlas no lo duplica, lo pelea. Peor todavia en las
            // paredes de una cajita, que miden medio texel: ahi el paralaje
            // se corre mas de lo que mide la cara y deja rayas.
            (_, true, _) => DEPTH_FLAT,
            (_, _, Finish::Soil | Finish::Foliage) => DEPTH_COARSE,
            _ => DEPTH_ROCK,
        };

        textures.push(texture.with_relief(relief).with_depth(depth));
    }

    textures
}

/// Los modelos 3D de todos los bloques, en el mismo orden. Los que el pack
/// no trae quedan con el cubo liso.
pub fn block_models() -> Vec<BlockModel> {
    let mut models = vec![BlockModel::plain()];

    for kind in BlockKind::ALL {
        let model = match kind.spec().model {
            Some(name) => BlockModel::load(name).unwrap_or_else(|problem| {
                eprintln!("modelo de {kind:?}: {problem}");
                BlockModel::plain()
            }),
            None => BlockModel::plain(),
        };

        models.push(model);
    }

    models
}

pub struct Terrain {
    /// Nivel del bloque mas alto de cada columna, y del mas bajo. La columna
    /// ocupa bottoms..=tops, y fuera de una isla no ocupa nada.
    tops: Vec<i32>,
    bottoms: Vec<i32>,
    /// A que isla pertenece cada columna, si es que pertenece a alguna.
    home: Vec<Option<u8>>,
    /// Que tan adentro del contorno de su isla esta: 0 en la orilla, 1 en el
    /// centro. De el salen la quilla y donde se puede plantar algo.
    inland: Vec<f32>,
    /// Manchas de un segundo material adentro de cada bioma.
    patches: Noise,
    /// Sorteos sueltos: el tono de cada bloque, las vetas y las plantas.
    grain: Noise,
    /// Donde caen las lamparas marinas. Son las unicas columnas elegidas a
    /// dedo del mapa, porque cada una es tambien una luz de la escena y no
    /// se pueden sortear de a cientos.
    lamps: Vec<(usize, usize)>,
}

impl Terrain {
    pub fn generate(seed: u64) -> Self {
        let relief = Noise::new(seed);
        let shore = relief.variant(SHORE_SALT);
        let drip = relief.variant(DRIP_SALT);

        let mut home = vec![None; SIZE * SIZE];
        let mut inland = vec![0.0; SIZE * SIZE];
        // las alturas se llevan sin redondear hasta despues del suavizado:
        // redondear antes pierde justo lo que el promedio necesita
        let mut surface = vec![0.0; SIZE * SIZE];
        let mut keel = vec![0.0; SIZE * SIZE];

        for z in 0..SIZE {
            for x in 0..SIZE {
                let Some((which, deep)) = belongs_to(&shore, x, z) else {
                    continue;
                };
                let island = ISLANDS[which as usize];

                // la quilla: fina en la orilla y hundida en el centro
                let hull = CRUST + KEEL * deep.powf(KEEL_CURVE);
                // y de ella cuelgan estalactitas donde el campo de goteo se
                // pasa del umbral, que es lo que le da a la panza el aspecto
                // de raices colgando en vez de un cono liso
                let field = drip.fbm(x as f32 / DRIP_SCALE, z as f32 / DRIP_SCALE, 3);
                let hang = (field - DRIP_START).max(0.0) * DRIP_REACH * deep;

                let here = z * SIZE + x;
                surface[here] = island.base as f32 + island.biome.height_at(&relief, x, z);
                keel[here] = hull + hang;
                home[here] = Some(which);
                inland[here] = deep;
            }
        }

        trim_shore(&mut home);
        erode(&mut surface, &home);

        let tops: Vec<i32> = surface.iter().map(|top| top.round() as i32).collect();
        let bottoms: Vec<i32> = tops
            .iter()
            .zip(&keel)
            .map(|(top, keel)| top - keel.round() as i32)
            .collect();

        let mut terrain = Terrain {
            tops,
            bottoms,
            home,
            inland,
            patches: relief.variant(PATCH_SALT),
            grain: relief.variant(GRAIN_SALT),
            lamps: Vec::new(),
        };

        // van al final porque el sorteo necesita el mapa ya armado: mira el
        // bioma de cada columna y el de sus vecinas
        terrain.lamps =
            terrain.scattered(LAMP_CHANCE, LAMP_SPACING, DRAW_LAMP, Terrain::holds_lamp);

        terrain
    }

    /// A que isla pertenece la columna, si es que a alguna.
    pub fn island(&self, x: usize, z: usize) -> Option<Island> {
        self.home[z * SIZE + x].map(|which| ISLANDS[which as usize])
    }

    pub fn biome(&self, x: usize, z: usize) -> Option<Biome> {
        self.island(x, z).map(|island| island.biome)
    }

    /// Nivel del bloque de arriba de la columna.
    pub fn top(&self, x: usize, z: usize) -> i32 {
        self.tops[z * SIZE + x]
    }

    pub fn bottom(&self, x: usize, z: usize) -> i32 {
        self.bottoms[z * SIZE + x]
    }

    /// Hay roca en ese punto de la grilla?
    pub fn solid(&self, x: i32, level: i32, z: i32) -> bool {
        let Some((x, z)) = inside_map(x, z) else {
            return false;
        };
        let here = z * SIZE + x;

        self.home[here].is_some() && level >= self.bottoms[here] && level <= self.tops[here]
    }

    /// Y agua?
    pub fn flooded(&self, x: i32, level: i32, z: i32) -> bool {
        let Some((x, z)) = inside_map(x, z) else {
            return false;
        };
        let here = z * SIZE + x;
        let Some(which) = self.home[here] else {
            return false;
        };

        level > self.tops[here] && level <= ISLANDS[which as usize].sea_level()
    }

    pub fn blocks(&self, models: &[BlockModel]) -> Vec<Object> {
        let mut blocks = Vec::new();
        self.push_ground(&mut blocks, models);
        self.push_water(&mut blocks, models);
        self.push_plants(&mut blocks, models);
        self.push_landmarks(&mut blocks, models);
        blocks
    }

    /// De cada columna salen los bloques que tienen alguna cara al aire, y de
    /// cada uno solo las cajitas del modelo que dan a esas caras. Un bloque
    /// enterrado no se genera; uno destapado solo por arriba no genera la
    /// decoracion de los otros cinco lados.
    fn push_ground(&self, blocks: &mut Vec<Object>, models: &[BlockModel]) {
        for z in 0..SIZE {
            for x in 0..SIZE {
                if self.home[z * SIZE + x].is_none() {
                    continue;
                }

                for level in self.bottom(x, z)..=self.top(x, z) {
                    // el agua no tapa: el fondo de la laguna se ve a traves
                    let exposed = self.exposed(x, level, z, Terrain::solid);
                    if !exposed.any() {
                        continue;
                    }

                    let kind = self.kind_at(x, z, level);
                    self.push_block(blocks, models, x, level, z, kind, exposed);
                }
            }
        }
    }

    /// Llena hasta el nivel del mar de su isla toda columna que no llegue.
    /// Contra el agua misma si tapa, asi que de una laguna queda la lamina de
    /// arriba y el corte del borde.
    fn push_water(&self, blocks: &mut Vec<Object>, models: &[BlockModel]) {
        for z in 0..SIZE {
            for x in 0..SIZE {
                let Some(island) = self.island(x, z) else {
                    continue;
                };

                let frozen = island.biome == Biome::Ice;
                for level in self.top(x, z) + 1..=island.sea_level() {
                    let exposed = self.exposed(x, level, z, |terrain, x, level, z| {
                        terrain.solid(x, level, z) || terrain.flooded(x, level, z)
                    });
                    if !exposed.any() {
                        continue;
                    }

                    // en la isla helada la lamina de arriba amanece congelada
                    let kind = if frozen && level == island.sea_level() {
                        BlockKind::Ice
                    } else {
                        BlockKind::Water
                    };
                    self.push_block(blocks, models, x, level, z, kind, exposed);
                }
            }
        }
    }

    /// Que caras del bloque dan al aire, segun la regla de ocupacion que le
    /// pase quien pregunta.
    fn exposed(
        &self,
        x: usize,
        level: i32,
        z: usize,
        occupied: impl Fn(&Terrain, i32, i32, i32) -> bool,
    ) -> Faces {
        let mut faces = Faces::NONE;

        for face in Face::ALL {
            let (dx, dy, dz) = face.step();
            if !occupied(self, x as i32 + dx, level + dy, z as i32 + dz) {
                faces = faces.with(face);
            }
        }

        faces
    }

    /// Mete un bloque en la escena: el cuerpo del modelo y la decoracion de
    /// las caras destapadas.
    fn push_block(
        &self,
        blocks: &mut Vec<Object>,
        models: &[BlockModel],
        x: usize,
        level: i32,
        z: usize,
        kind: BlockKind,
        exposed: Faces,
    ) {
        let draw = self.grain.hash_unit(x as i32, level, z as i32);
        let material = kind.material(tinted(draw));
        let corner = block_corner(x, level, z);

        for element in models[kind.slot() as usize].visible(exposed) {
            blocks.push(Object::part(
                corner,
                BLOCK,
                element.min,
                element.max,
                material,
            ));
        }
    }

    /// Lo que crece sobre el suelo, distinto en cada isla.
    fn push_plants(&self, blocks: &mut Vec<Object>, models: &[BlockModel]) {
        for (x, z) in self.scattered(TREE_CHANCE, TREE_SPACING, DRAW_TREE, Terrain::holds_tree) {
            self.push_tree(x, z, blocks, models);
        }
        for (x, z) in self.scattered(
            CACTUS_CHANCE,
            CACTUS_SPACING,
            DRAW_CACTUS,
            Terrain::holds_cactus,
        ) {
            self.push_column(x, z, 2, 4, BlockKind::Cactus, blocks, models);
        }
        for (x, z) in self.scattered(SPIKE_CHANCE, SPIKE_SPACING, DRAW_SPIKE, Terrain::holds_spike) {
            self.push_column(x, z, 3, 8, BlockKind::PackedIce, blocks, models);
        }
        // en la cantera, bloques sueltos de adoquin musgoso: son el bloque
        // del pack con mas geometria propia y se leen desde lejos
        for (x, z) in self.scattered(
            BOULDER_CHANCE,
            BOULDER_SPACING,
            DRAW_BOULDER,
            Terrain::holds_boulder,
        ) {
            self.push_column(x, z, 1, 3, BlockKind::MossyCobblestone, blocks, models);
        }
    }

    /// Un monumento por isla: una base de ladrillos y encima una columna del
    /// material con el que esa isla muestra lo que hace el trazador. Son los
    /// unicos bloques puestos a mano, y estan justamente para que el oro, el
    /// diamante y el cobre tengan donde reflejar el cielo y el terreno.
    fn push_landmarks(&self, blocks: &mut Vec<Object>, models: &[BlockModel]) {
        for island in ISLANDS {
            let (cx, cz) = (island.center.0 as usize, island.center.1 as usize);
            if self.home[cz * SIZE + cx].is_none() {
                continue;
            }

            let base = self.top(cx, cz) + 1;
            let (plinth, step) = match island.biome {
                Biome::Forest => (BlockKind::StoneBricks, BlockKind::Planks),
                Biome::Desert => (BlockKind::CutSandstone, BlockKind::SmoothStone),
                Biome::Ice => (BlockKind::Quartz, BlockKind::PackedIce),
                Biome::Mines => (BlockKind::DeepslateBricks, BlockKind::NetheriteBlock),
            };

            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1), (0, 0)] {
                let (px, pz) = ((cx as i32 + dx) as usize, (cz as i32 + dz) as usize);
                self.push_block(blocks, models, px, base, pz, plinth, Faces::ALL);
            }

            // la obsidiana va abajo de todo: negra y pulida, es la que deja
            // ver de una sola vez que lo que se dibuja ahi no es su color
            // sino lo que refleja
            self.push_block(blocks, models, cx, base + 1, cz, step, Faces::ALL);
            self.push_block(blocks, models, cx, base + 2, cz, BlockKind::Obsidian, Faces::ALL);

            for level in base + 3..base + 6 {
                self.push_block(blocks, models, cx, level, cz, island.landmark(), Faces::ALL);
            }
            // remate de vidrio: lo unico de la escena que refracta ademas del
            // agua y el hielo, y arriba de todo donde se lo ve contra el cielo
            self.push_block(blocks, models, cx, base + 6, cz, BlockKind::Glass, Faces::ALL);
        }
    }

    /// Columnas sueltas repartidas por el mapa, con una separacion minima
    /// para que no se amontonen.
    fn scattered(
        &self,
        chance: f32,
        spacing: i32,
        draw: i32,
        suits: impl Fn(&Terrain, usize, usize) -> bool,
    ) -> Vec<(usize, usize)> {
        let mut planted: Vec<(usize, usize)> = Vec::new();

        for z in 1..SIZE - 1 {
            for x in 1..SIZE - 1 {
                if !suits(self, x, z) {
                    continue;
                }
                if self.grain.hash_unit(x as i32, draw, z as i32) >= chance {
                    continue;
                }
                if planted.iter().any(|&(px, pz)| {
                    let reach = (px as i32 - x as i32).abs().max((pz as i32 - z as i32).abs());
                    reach < spacing
                }) {
                    continue;
                }

                planted.push((x, z));
            }
        }

        planted
    }

    fn holds_tree(&self, x: usize, z: usize) -> bool {
        self.well_inside(x, z, Biome::Forest) && self.surface_kind(x, z) == BlockKind::Grass
    }

    fn holds_cactus(&self, x: usize, z: usize) -> bool {
        let Some(island) = self.island(x, z) else {
            return false;
        };
        self.well_inside(x, z, Biome::Desert) && self.top(x, z) > island.sea_level() + BEACH_BAND
    }

    fn holds_spike(&self, x: usize, z: usize) -> bool {
        let Some(island) = self.island(x, z) else {
            return false;
        };
        self.well_inside(x, z, Biome::Ice) && self.top(x, z) > island.sea_level()
    }

    fn holds_boulder(&self, x: usize, z: usize) -> bool {
        let Some(island) = self.island(x, z) else {
            return false;
        };
        self.well_inside(x, z, Biome::Mines) && self.top(x, z) > island.sea_level()
    }

    fn holds_lamp(&self, x: usize, z: usize) -> bool {
        self.well_inside(x, z, Biome::Ice)
    }

    /// Donde va cada luz de lampara marina, en coordenadas del mundo.
    ///
    /// La luz no va en el centro del bloque sino apenas por encima de su
    /// cara de arriba. Puesta adentro, el propio bloque se interpondria entre
    /// ella y todo lo que tiene alrededor, y el rayo de sombra daria que la
    /// lampara no ilumina nada: una lampara encerrada en su propia pantalla.
    pub fn lamps(&self) -> Vec<Vector3> {
        self.lamps
            .iter()
            .map(|&(x, z)| {
                // la cara de arriba del bloque es su Y minimo: aca -Y es
                // arriba
                block_corner(x, self.top(x, z), z)
                    + Vector3::new(BLOCK * 0.5, -LAMP_LIFT, BLOCK * 0.5)
            })
            .collect()
    }

    /// La columna esta bien adentro de su isla: lejos de la orilla y con sus
    /// cuatro vecinas del mismo bioma. Lo que crece asoma un bloque para los
    /// costados, asi que plantarlo en el borde lo dejaria colgando del aire.
    fn well_inside(&self, x: usize, z: usize, biome: Biome) -> bool {
        self.biome(x, z) == Some(biome)
            && self.inland[z * SIZE + x] > INLAND
            && [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().all(|&(dx, dz)| {
                let (nx, nz) = ((x as i32 + dx) as usize, (z as i32 + dz) as usize);
                self.biome(nx, nz) == Some(biome)
            })
    }

    /// Tronco recto y una copa de hojas alrededor de la punta.
    fn push_tree(&self, x: usize, z: usize, blocks: &mut Vec<Object>, models: &[BlockModel]) {
        // en la mitad alta del bosque los arboles son pinos: da un limite de
        // vegetacion sin tener que inventar otro bioma
        let island = self.island(x, z).expect("el arbol crece en una isla");
        let pine = self.top(x, z) >= island.base + SNOW_OFFSET - 5;
        let (log, leaf) = if pine {
            (BlockKind::SpruceLog, BlockKind::SpruceLeaves)
        } else {
            (BlockKind::Wood, BlockKind::Leaves)
        };

        let base = self.top(x, z) + 1;
        let crown = base + TRUNK_HEIGHT - 1;

        for level in base..=crown {
            self.push_block(blocks, models, x, level, z, log, Faces::ALL);
        }

        for level in [crown - 1, crown] {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dz == 0 {
                        continue; // ahi esta el tronco
                    }
                    let (lx, lz) = ((x as i32 + dx) as usize, (z as i32 + dz) as usize);
                    self.push_block(blocks, models, lx, level, lz, leaf, Faces::ALL);
                }
            }
        }

        // la tapa: una cruz, que es lo que le da la punta redondeada
        for (dx, dz) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (lx, lz) = ((x as i32 + dx) as usize, (z as i32 + dz) as usize);
            self.push_block(blocks, models, lx, crown + 1, lz, leaf, Faces::ALL);
        }
    }

    /// Una pila de un bloque de ancho, de alto sorteado.
    fn push_column(
        &self,
        x: usize,
        z: usize,
        shortest: i32,
        tallest: i32,
        kind: BlockKind,
        blocks: &mut Vec<Object>,
        models: &[BlockModel],
    ) {
        let draw = self.grain.hash_unit(x as i32, tallest, z as i32);
        let height = shortest + (draw * (tallest - shortest) as f32) as i32;
        let base = self.top(x, z) + 1;

        for level in base..base + height {
            self.push_block(blocks, models, x, level, z, kind, Faces::ALL);
        }
    }

    fn kind_at(&self, x: usize, z: usize, level: i32) -> BlockKind {
        let top = self.top(x, z);
        let bottom = self.bottom(x, z);

        if level == top {
            return self.surface_kind(x, z);
        }

        let depth = top - level;
        let island = self.island(x, z).expect("la columna esta en una isla");
        let from_bottom = level - bottom;

        // Geoda: en la panza de la isla helada aparece un nucleo de amatista
        // forrado en caliza. Esta en el juego, pero antes que ahi esta en la
        // realidad: una burbuja en la roca donde el mineral cristalizo de
        // adentro hacia afuera.
        if island.biome == Biome::Ice && from_bottom < BEDROCK_BAND + 5 {
            let geode = self
                .patches
                .sample(x as f32 / 4.0 + 77.0, (z as f32 + level as f32 * 0.7) / 4.0);

            if geode > 0.62 {
                return BlockKind::AmethystBlock;
            }
            if geode > 0.48 {
                return BlockKind::Calcite;
            }
        }

        // la panza de la isla: roca profunda, que es lo que se ve desde abajo
        if from_bottom < BEDROCK_BAND {
            // y en la cantera, lo que quedo del fondo: obsidiana donde la
            // lava toco el agua, y magma donde todavia no se apago
            if island.biome == Biome::Mines {
                match self.grain.hash_unit(x as i32, level - DRAW_VEIN, z as i32) {
                    draw if draw < 0.03 => return BlockKind::Obsidian,
                    draw if draw < 0.06 => return BlockKind::Magma,
                    _ => {}
                }
            }

            return self.with_vein(x, z, level, BlockKind::Deepslate);
        }

        let under = match island.biome {
            Biome::Forest if depth <= SUBSOIL => match self.surface_kind(x, z) {
                BlockKind::Sand => BlockKind::Sand,
                BlockKind::Snow | BlockKind::Ice => BlockKind::Stone,
                BlockKind::Moss => BlockKind::Moss,
                _ => BlockKind::Dirt,
            },
            Biome::Desert if depth <= 1 => BlockKind::Sand,
            Biome::Desert if depth <= SUBSOIL + 2 => {
                // la arenisca sale del color de la arena que tiene encima
                if self.surface_kind(x, z) == BlockKind::RedSand {
                    BlockKind::RedSandstone
                } else {
                    BlockKind::Sandstone
                }
            }
            Biome::Desert if depth <= SUBSOIL + 4 => BlockKind::Terracotta,
            Biome::Ice if depth <= 1 => BlockKind::PackedIce,
            Biome::Ice if depth <= SUBSOIL => BlockKind::Clay,
            Biome::Mines if depth <= 2 => BlockKind::Cobblestone,
            _ => self.rock_at(x, z, level),
        };

        self.with_vein(x, z, level, under)
    }

    /// De que esta hecha la roca de esa columna. Las tres variantes de piedra
    /// se reparten con el campo de manchas, asi que salen en vetas anchas y
    /// no en confeti.
    fn rock_at(&self, x: usize, z: usize, level: i32) -> BlockKind {
        let vein = self
            .patches
            .sample(x as f32 / 9.0 + 31.0, (z as f32 + level as f32 * 0.4) / 9.0);

        match vein {
            v if v < -0.58 => BlockKind::Basalt,
            v if v < -0.42 => BlockKind::Andesite,
            v if v < -0.2 => BlockKind::Tuff,
            v if v > 0.62 => BlockKind::Calcite,
            v if v > 0.5 => BlockKind::Granite,
            v if v > 0.34 => BlockKind::Diorite,
            _ => BlockKind::Stone,
        }
    }

    /// Con que se cubre la columna, segun su isla y a que altura termina.
    fn surface_kind(&self, x: usize, z: usize) -> BlockKind {
        let Some(island) = self.island(x, z) else {
            return BlockKind::Stone;
        };
        let top = self.top(x, z);
        let patch = self.patch(x, z);

        match island.biome {
            Biome::Forest => {
                if top < island.sea_level() {
                    // el fondo de la laguna, que se ve a traves del agua
                    if patch > 0.2 {
                        BlockKind::Prismarine
                    } else {
                        BlockKind::Clay
                    }
                } else if top <= island.sea_level() + BEACH_BAND {
                    BlockKind::Sand
                } else if top >= island.snow_line() {
                    if patch > 0.1 {
                        BlockKind::Ice
                    } else {
                        BlockKind::Snow
                    }
                } else if patch < -0.38 {
                    BlockKind::Moss
                } else if patch > 0.46 {
                    BlockKind::CoarseDirt
                } else {
                    BlockKind::Grass
                }
            }
            Biome::Desert => {
                if top <= island.sea_level() {
                    BlockKind::Clay
                } else if patch < -0.3 {
                    BlockKind::Sandstone
                } else if patch > 0.42 {
                    BlockKind::RedSand
                } else {
                    BlockKind::Sand
                }
            }
            Biome::Ice => {
                // las lamparas van al ras del suelo, y donde caen en una
                // laguna quedan debajo de la capa de hielo: el hielo deja
                // pasar la luz, asi que de noche el charco se prende entero
                if self.lamps.contains(&(x, z)) {
                    BlockKind::SeaLantern
                } else if top <= island.sea_level() {
                    BlockKind::Gravel // el fondo que se ve por debajo del hielo
                } else if patch > 0.35 {
                    BlockKind::Ice
                } else if patch < -0.35 {
                    BlockKind::PackedIce
                } else {
                    BlockKind::Snow
                }
            }
            Biome::Mines => {
                if top <= island.sea_level() {
                    BlockKind::Mud
                } else if patch < -0.3 {
                    BlockKind::Gravel
                } else if patch > 0.45 {
                    BlockKind::Cobblestone
                } else {
                    self.rock_at(x, z, top)
                }
            }
        }
    }

    /// La roca sale con vetas a la vista. Como solo se generan los bloques
    /// destapados, las vetas aparecen justo donde se las puede ver: en el
    /// corte del borde y, sobre todo, en la panza de la isla.
    fn with_vein(&self, x: usize, z: usize, level: i32, kind: BlockKind) -> BlockKind {
        if !matches!(
            kind,
            BlockKind::Stone
                | BlockKind::Deepslate
                | BlockKind::Andesite
                | BlockKind::Granite
                | BlockKind::Diorite
                | BlockKind::Tuff
        ) {
            return kind;
        }

        // en la cantera hay el doble de vetas: es lo que la hace una cantera
        let richness = if self.biome(x, z) == Some(Biome::Mines) {
            1.0
        } else {
            2.2
        };

        match self.grain.hash_unit(x as i32, level + DRAW_VEIN, z as i32) * richness {
            draw if draw < VEIN_DIAMOND => BlockKind::DiamondOre,
            draw if draw < VEIN_EMERALD => BlockKind::EmeraldOre,
            draw if draw < VEIN_GOLD => BlockKind::GoldOre,
            draw if draw < VEIN_LAPIS => BlockKind::LapisOre,
            draw if draw < VEIN_REDSTONE => BlockKind::RedstoneOre,
            draw if draw < VEIN_COPPER => BlockKind::CopperOre,
            draw if draw < VEIN_IRON => BlockKind::IronOre,
            draw if draw < VEIN_COAL => BlockKind::CoalOre,
            _ => kind,
        }
    }

    /// Campo suave que reparte el segundo material de cada bioma. Va aparte
    /// del relieve: si dependiera de la altura, todas las manchas caerian en
    /// las mismas partes del mapa.
    fn patch(&self, x: usize, z: usize) -> f32 {
        self.patches
            .sample(x as f32 / PATCH_SCALE, z as f32 / PATCH_SCALE)
    }
}

/// A que isla cae una columna, y que tan adentro de su contorno: 0 en la
/// orilla y 1 en el centro.
///
/// El contorno no es un circulo: el radio se ondula con ruido, asi que cada
/// isla sale con su forma y no cuatro moldes iguales.
fn belongs_to(shore: &Noise, x: usize, z: usize) -> Option<(u8, f32)> {
    for (which, island) in ISLANDS.iter().enumerate() {
        let (dx, dz) = (x as f32 - island.center.0, z as f32 - island.center.1);
        let distance = (dx * dx + dz * dz).sqrt();

        let wobble = shore.sample(x as f32 / SHORE_SCALE, z as f32 / SHORE_SCALE);
        let radius = RADIUS * (1.0 + wobble * SHORE_WOBBLE);

        if distance < radius {
            return Some((which as u8, 1.0 - distance / radius));
        }
    }

    None
}

/// Centro de cada isla en coordenadas del mundo, a la altura de su laguna.
/// Sirve para apuntarle la camara a una isla en vez de al hueco que queda
/// entre las cuatro.
pub fn island_centers() -> [Vector3; ISLANDS.len()] {
    ISLANDS.map(|island| {
        block_corner(
            island.center.0 as usize,
            island.sea_level(),
            island.center.1 as usize,
        ) + Vector3::new(BLOCK * 0.5, BLOCK * 0.5, BLOCK * 0.5)
    })
}

/// Le pasa una mano de erosion al campo de alturas.
///
/// Un campo de ruido no tiene historia: la altura de cada columna se sortea
/// sin mirar a las de al lado, y eso deja agujas de una columna de ancho y
/// escalones que ningun derrumbe habria dejado en pie. Un cerro de verdad
/// paso miles de anos tirando abajo justamente eso.
///
/// El promedio con las ocho columnas de alrededor es la version barata de ese
/// proceso. El peso del centro es alto a proposito: con eso una aguja suelta
/// se cae, porque todas sus vecinas la contradicen, pero un acantilado se
/// queda, porque de un lado tiene media docena de vecinas que le dan la
/// razon. Borrar los acantilados seria peor que tener agujas: la isla
/// volveria a ser una loma.
///
/// El cielo no promedia. Si promediara, cada columna de la orilla se llevaria
/// puesta la altura del aire que tiene al lado y las islas terminarian con el
/// borde en pendiente hacia abajo, como un platillo.
fn erode(surface: &mut [f32], home: &[Option<u8>]) {
    for _ in 0..EROSION_PASSES {
        let before = surface.to_vec();

        for z in 0..SIZE {
            for x in 0..SIZE {
                let here = z * SIZE + x;
                if home[here].is_none() {
                    continue;
                }

                let mut total = before[here] * EROSION_CENTER;
                let mut weight = EROSION_CENTER;

                // las diagonales pesan la mitad porque estan mas lejos, pero
                // tienen que estar: con solo las cuatro de al lado, una cresta
                // en diagonal no tiene quien la contradiga y sobrevive entera,
                // y lo que queda es una aleta de un bloque de ancho cruzando
                // la isla
                for (dx, dz, share) in [
                    (-1i32, 0i32, 1.0),
                    (1, 0, 1.0),
                    (0, -1, 1.0),
                    (0, 1, 1.0),
                    (-1, -1, 0.5),
                    (1, -1, 0.5),
                    (-1, 1, 0.5),
                    (1, 1, 0.5),
                ] {
                    let Some((nx, nz)) = inside_map(x as i32 + dx, z as i32 + dz) else {
                        continue;
                    };
                    if home[nz * SIZE + nx].is_some() {
                        total += before[nz * SIZE + nx] * share;
                        weight += share;
                    }
                }

                surface[here] = total / weight;
            }
        }
    }
}

/// Le saca a la costa las columnas sueltas y las lenguas de una columna de
/// ancho.
///
/// El contorno lo dibuja ruido, y donde el ruido apenas roza el radio deja
/// columnas que no tocan a ninguna otra. En un mapa plano eso seria una
/// piedrita en el agua y no se notaria, pero aca cada columna arrastra su
/// costra, su quilla y lo que le cuelgue: una columna suelta es una aguja de
/// treinta bloques flotando al lado de la isla. Visto de costado, el borde
/// entero parecia un peine.
///
/// La regla es la de siempre para limpiar un mapa de celdas: la columna que
/// no tiene al menos dos vecinas se va. Dos pasadas, que es lo que tarda en
/// irse una lengua de dos columnas de largo; con mas, la isla empieza a
/// perder los entrantes de la costa, que son justamente lo que la hace ver
/// dibujada por el agua y no con un compas.
fn trim_shore(home: &mut [Option<u8>]) {
    for _ in 0..SHORE_PASSES {
        let before = home.to_vec();

        for z in 0..SIZE {
            for x in 0..SIZE {
                let here = z * SIZE + x;
                if before[here].is_none() {
                    continue;
                }

                let neighbours = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .filter(|(dx, dz)| {
                        inside_map(x as i32 + dx, z as i32 + dz)
                            .is_some_and(|(nx, nz)| before[nz * SIZE + nx].is_some())
                    })
                    .count();

                if neighbours < 2 {
                    home[here] = None;
                }
            }
        }
    }
}

/// Lleva una coordenada de la grilla a su indice, o None si se fue del mapa.
fn inside_map(x: i32, z: i32) -> Option<(usize, usize)> {
    if x < 0 || z < 0 || x >= SIZE as i32 || z >= SIZE as i32 {
        return None;
    }

    Some((x as usize, z as usize))
}

/// Esquina minima de un bloque de la grilla. El mapa queda centrado en el
/// origen y crece hacia -Y, que aca es arriba.
fn block_corner(x: usize, level: i32, z: usize) -> Vector3 {
    let half = (SIZE as f32 - 1.0) * 0.5;

    Vector3::new(
        (x as f32 - half - 0.5) * BLOCK,
        -(level as f32 + 1.0) * BLOCK,
        (z as f32 - half - 0.5) * BLOCK,
    )
}

/// Gris por el que se multiplica la textura del bloque. Solo oscurece: si
/// tambien aclarara, el tope de 255 recortaria la mitad de los sorteos y esos
/// bloques saldrian todos iguales.
fn tinted(draw: f32) -> Color {
    let level = (255.0 * (1.0 - (1.0 - draw) * COLOR_JITTER)) as u8;

    Color::new(level, level, level, 255)
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::PLAIN_TEXTURE;
    use std::collections::HashSet;

    /// Semillas fijas para que los tests no dependan del sorteo del dia.
    const SEEDS: [u64; 6] = [1, 7, 42, 20240519, 777777, u64::MAX];

    fn columns() -> impl Iterator<Item = (usize, usize)> {
        (0..SIZE).flat_map(|z| (0..SIZE).map(move |x| (x, z)))
    }

    fn center_of(island: Island) -> (usize, usize) {
        (island.center.0 as usize, island.center.1 as usize)
    }

    fn white() -> Color {
        Color::new(255, 255, 255, 255)
    }

    // -- la forma del mapa -------------------------------------------------

    #[test]
    fn the_map_holds_four_islands_with_sky_around_them() {
        let terrain = Terrain::generate(42);

        assert_eq!(ISLANDS.len(), 4);
        for island in ISLANDS {
            let (x, z) = center_of(island);
            assert_eq!(
                terrain.biome(x, z),
                Some(island.biome),
                "el centro de {:?} tendria que ser suyo",
                island.biome
            );
        }

        for (x, z) in [(0, 0), (SIZE - 1, 0), (0, SIZE - 1), (SIZE - 1, SIZE - 1)] {
            assert!(terrain.island(x, z).is_none(), "({x}, {z}) tendria que ser cielo");
        }
        assert!(terrain.island(SIZE / 2, SIZE / 2).is_none(), "el medio es cielo");
    }

    /// Lo que las separa: dos islas no pueden quedar pegadas, o serian un
    /// continente con una grieta.
    #[test]
    fn the_islands_never_touch_each_other() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            for (x, z) in columns() {
                let Some(here) = terrain.biome(x, z) else {
                    continue;
                };

                for (dx, dz) in [(1, 0), (0, 1)] {
                    let (nx, nz) = (x + dx, z + dz);
                    if nx >= SIZE || nz >= SIZE {
                        continue;
                    }
                    if let Some(there) = terrain.biome(nx, nz) {
                        assert_eq!(here, there, "semilla {seed}: ({x}, {z}) pegado a otro bioma");
                    }
                }
            }
        }
    }

    /// Ninguna isla llega al borde del mapa: si llegara, el contorno que
    /// ondula el ruido quedaria cortado al ras y se veria el tajo.
    #[test]
    fn no_island_reaches_the_edge_of_the_map() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            for i in 0..SIZE {
                for (x, z) in [(i, 0), (i, SIZE - 1), (0, i), (SIZE - 1, i)] {
                    assert!(terrain.island(x, z).is_none(), "semilla {seed}: ({x}, {z})");
                }
            }
        }
    }

    #[test]
    fn the_islands_float_at_different_heights() {
        let bases: HashSet<i32> = ISLANDS.iter().map(|island| island.base).collect();
        assert_eq!(bases.len(), 4, "dos islas comparten altura");

        let terrain = Terrain::generate(42);
        let tops: Vec<i32> = ISLANDS
            .iter()
            .map(|island| {
                let (x, z) = center_of(*island);
                terrain.top(x, z)
            })
            .collect();

        assert!(
            tops.windows(2).any(|pair| pair[0] != pair[1]),
            "las cuatro terminan a la misma altura: {tops:?}"
        );
    }

    // -- la panza ----------------------------------------------------------

    /// Una isla flotante no es un plano: toda columna tiene espesor, y el
    /// centro cuelga mucho mas que la orilla.
    #[test]
    fn every_island_hangs_deeper_at_its_heart_than_at_its_shore() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            for island in ISLANDS {
                let (cx, cz) = center_of(island);
                let heart = terrain.top(cx, cz) - terrain.bottom(cx, cz);

                let shore = columns()
                    .filter(|&(x, z)| {
                        terrain.biome(x, z) == Some(island.biome)
                            && terrain.inland[z * SIZE + x] < 0.08
                    })
                    .map(|(x, z)| terrain.top(x, z) - terrain.bottom(x, z))
                    .max()
                    .expect("la isla tiene orilla");

                assert!(
                    heart > shore,
                    "semilla {seed}: {:?} cuelga {heart} en el centro y {shore} en la orilla",
                    island.biome
                );
            }
        }
    }

    #[test]
    fn every_column_of_an_island_has_some_rock() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            for (x, z) in columns() {
                if terrain.island(x, z).is_none() {
                    continue;
                }
                let thickness = terrain.top(x, z) - terrain.bottom(x, z) + 1;
                assert!(
                    thickness >= CRUST as i32,
                    "semilla {seed}: ({x}, {z}) mide {thickness} de espesor"
                );
            }
        }
    }

    /// Las estalactitas: en alguna parte de la panza tiene que colgar algo
    /// mas largo que la quilla sola, o la isla seria un cono liso.
    #[test]
    fn stalactites_hang_off_the_belly() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);
            let longest = columns()
                .filter(|&(x, z)| terrain.island(x, z).is_some())
                .map(|(x, z)| terrain.top(x, z) - terrain.bottom(x, z))
                .max()
                .expect("hay islas");

            let keel = (CRUST + KEEL) as i32;
            assert!(
                longest > keel,
                "semilla {seed}: lo mas largo mide {longest} y la quilla sola {keel}"
            );
        }
    }

    // -- el relieve de arriba ----------------------------------------------

    /// Terreno, no ruido blanco: dos columnas vecinas de la misma isla no
    /// pueden pegar saltos de acantilado en promedio.
    #[test]
    fn neighbouring_columns_stay_close() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);
            let (mut total, mut steps) = (0, 0);

            for (x, z) in columns() {
                if x == 0 || terrain.biome(x, z).is_none() || terrain.biome(x - 1, z).is_none() {
                    continue;
                }
                total += (terrain.top(x, z) - terrain.top(x - 1, z)).abs();
                steps += 1;
            }

            let average = total as f32 / steps as f32;
            assert!(average < 2.5, "semilla {seed}: escalon promedio de {average}");
        }
    }

    /// Y aun asi el relieve tiene que ser marcado: es lo que separa un
    /// paisaje de una mesa.
    #[test]
    fn the_relief_is_dramatic() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            for island in ISLANDS {
                let tops: Vec<i32> = columns()
                    .filter(|&(x, z)| terrain.biome(x, z) == Some(island.biome))
                    .map(|(x, z)| terrain.top(x, z))
                    .collect();

                let range = tops.iter().max().unwrap() - tops.iter().min().unwrap();
                assert!(
                    range >= 8,
                    "semilla {seed}: {:?} va de punta a punta {range} bloques",
                    island.biome
                );
            }
        }
    }

    #[test]
    fn the_same_seed_rebuilds_the_same_map() {
        let a = Terrain::generate(20240519);
        let b = Terrain::generate(20240519);

        assert_eq!(a.tops, b.tops);
        assert_eq!(a.bottoms, b.bottoms);
    }

    #[test]
    fn different_seeds_build_different_maps() {
        for pair in SEEDS.windows(2) {
            let a = Terrain::generate(pair[0]);
            let b = Terrain::generate(pair[1]);
            assert_ne!(a.tops, b.tops, "semillas {} y {}", pair[0], pair[1]);
        }
    }

    // -- que bloques salen -------------------------------------------------

    #[test]
    fn the_water_stops_at_each_islands_sea_level() {
        let terrain = Terrain::generate(42);

        for (x, z) in columns() {
            let Some(island) = terrain.island(x, z) else {
                continue;
            };

            assert!(!terrain.flooded(x as i32, island.sea_level() + 1, z as i32));
            if terrain.top(x, z) < island.sea_level() {
                assert!(
                    terrain.flooded(x as i32, island.sea_level(), z as i32),
                    "({x}, {z}) tendria que estar inundada"
                );
            }
        }
    }

    /// Cada isla se cubre con lo suyo: si no, el mapa entero seria el mismo
    /// paisaje pintado de cuatro colores.
    #[test]
    fn each_island_has_its_own_ground() {
        let terrain = Terrain::generate(42);
        let mut seen: Vec<HashSet<BlockKind>> = vec![HashSet::new(); ISLANDS.len()];

        for (x, z) in columns() {
            let Some(island) = terrain.island(x, z) else {
                continue;
            };
            let which = ISLANDS.iter().position(|i| i.biome == island.biome).unwrap();
            seen[which].insert(terrain.surface_kind(x, z));
        }

        for (which, island) in ISLANDS.iter().enumerate() {
            let expected = match island.biome {
                Biome::Forest => BlockKind::Grass,
                Biome::Desert => BlockKind::Sand,
                Biome::Ice => BlockKind::Snow,
                Biome::Mines => BlockKind::Stone,
            };

            assert!(
                seen[which].contains(&expected),
                "{:?} no tiene {expected:?} en la superficie",
                island.biome
            );
        }

        assert!(!seen[1].contains(&BlockKind::Grass), "el desierto tiene pasto");
        assert!(!seen[0].contains(&BlockKind::Sandstone), "el bosque tiene arenisca");
    }

    /// Lo que justifica no generar el relleno: un mapa macizo serian cientos
    /// de miles de bloques, y eso lo paga cada rayo de cada frame.
    #[test]
    fn only_the_shell_of_the_map_gets_traced() {
        let terrain = Terrain::generate(42);
        let mut solid = 0;
        let mut shown = 0;

        for (x, z) in columns() {
            if terrain.island(x, z).is_none() {
                continue;
            }
            for level in terrain.bottom(x, z)..=terrain.top(x, z) {
                solid += 1;
                if terrain.exposed(x, level, z, Terrain::solid).any() {
                    shown += 1;
                }
            }
        }

        assert!(shown * 4 < solid * 3, "{shown} de {solid} bloques quedaron a la vista");
    }

    #[test]
    fn plants_rest_on_the_ground() {
        let terrain = Terrain::generate(42);
        let planted =
            terrain.scattered(TREE_CHANCE, TREE_SPACING, DRAW_TREE, Terrain::holds_tree);

        assert!(!planted.is_empty(), "el bosque quedo pelado");
        for (x, z) in planted {
            assert_eq!(terrain.biome(x, z), Some(Biome::Forest));
            assert_eq!(terrain.surface_kind(x, z), BlockKind::Grass);

            // la copa asoma un bloque para los costados: sus vecinas tambien
            // tienen que ser suelo de la misma isla
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (nx, nz) = ((x as i32 + dx) as usize, (z as i32 + dz) as usize);
                assert_eq!(terrain.biome(nx, nz), Some(Biome::Forest), "({nx}, {nz})");
            }
        }
    }

    // -- la tabla de bloques -----------------------------------------------

    /// El orden de ALL es lo que ata cada bloque con su textura y su modelo:
    /// si se desfasa, el mapa entero sale pintado con la textura del vecino.
    #[test]
    fn every_kind_sits_where_its_slot_says() {
        for (place, kind) in BlockKind::ALL.iter().enumerate() {
            assert_eq!(kind.slot() as usize, place + 1, "{kind:?}");
        }

        let slots: HashSet<u16> = BlockKind::ALL.iter().map(|kind| kind.slot()).collect();
        assert_eq!(slots.len(), BlockKind::ALL.len());
        assert!(!slots.contains(&PLAIN_TEXTURE), "alguno pisa la textura lisa");
    }

    /// Y cada uno tiene que encontrar de verdad su textura y su modelo en
    /// disco: si falta un PNG el bloque cae en la receta y se ve distinto al
    /// resto, que es justo lo que este test esta para avisar.
    #[test]
    fn every_kind_finds_its_assets() {
        for kind in BlockKind::ALL {
            let spec = kind.spec();

            if let Some((file, _)) = spec.texture {
                let path = format!("assets/blocks/{file}.png");
                assert!(
                    std::path::Path::new(&path).exists(),
                    "falta la textura de {kind:?}: {path}"
                );
            }
            if let Some(name) = spec.model {
                BlockModel::load(name).unwrap_or_else(|problem| panic!("{kind:?}: {problem}"));
            }
        }

        assert_eq!(block_textures().len(), BlockKind::ALL.len() + 1);
        assert_eq!(block_models().len(), BlockKind::ALL.len() + 1);
    }

    /// La escena tiene que mostrar de todo: si un acabado esta en la tabla
    /// pero nunca se genera, es peso muerto.
    #[test]
    fn the_scene_shows_a_wide_range_of_materials() {
        let terrain = Terrain::generate(42);
        let mut finishes: HashSet<Finish> = HashSet::new();

        for (x, z) in columns() {
            if terrain.island(x, z).is_none() {
                continue;
            }
            for level in terrain.bottom(x, z)..=terrain.top(x, z) {
                finishes.insert(terrain.kind_at(x, z, level).spec().finish);
            }
        }

        for wanted in [Finish::Soil, Finish::Rock, Finish::Chalk, Finish::Polished] {
            assert!(finishes.contains(&wanted), "no aparece ningun {wanted:?}");
        }
    }

    /// El tinte del bloque solo oscurece un poco: si se pasara, el color de
    /// la textura se perderia debajo.
    #[test]
    fn the_tint_only_nudges_the_block() {
        for draw in [0.0, 0.5, 1.0] {
            let tint = tinted(draw);
            assert_eq!(tint.r, tint.g, "el tinte no puede virar el color");
            assert_eq!(tint.g, tint.b);
            assert!(tint.r as f32 >= 255.0 * (1.0 - COLOR_JITTER) - 1.0, "{}", tint.r);
        }
    }

    /// Los niveles de reflectividad: un metal devuelve mucho mas que una
    /// roca, y ademas lo devuelve tenido. Se mide uno contra otro y no
    /// contra un numero fijo, porque lo que importa es la diferencia.
    #[test]
    fn the_materials_span_real_levels_of_reflectivity() {
        let metal = BlockKind::GoldBlock.material(white());
        let polished = BlockKind::Quartz.material(white());
        let rock = BlockKind::Stone.material(white());
        let soil = BlockKind::Dirt.material(white());

        assert!(metal.albedo.reflection > polished.albedo.reflection);
        assert!(polished.albedo.reflection > rock.albedo.reflection);
        assert!(rock.albedo.reflection >= soil.albedo.reflection);
        assert_eq!(soil.albedo.reflection, 0.0, "la tierra no es un espejo");

        assert!(metal.metallic, "el oro tine lo que refleja");
        assert!(!polished.metallic, "el cuarzo no");

        // y lo que deja pasar la luz la dobla, cada uno con su indice
        let glass = BlockKind::Glass.material(white());
        let water = BlockKind::Water.material(white());
        let gem = BlockKind::DiamondBlock.material(white());

        assert!(water.refractive_index < glass.refractive_index);
        assert!(glass.refractive_index < gem.refractive_index);
        for clear in [glass, water, gem] {
            assert!(clear.albedo.refraction > 0.0);
        }
    }

    /// Dos bloques no llevan relieve hundido: el que refleja, porque el
    /// paralaje le rompe la imagen que devuelve, y el que el pack ya decoro
    /// con geometria, porque ahi el relieve lo ponen las cajitas.
    #[test]
    fn only_the_plain_blocks_carry_the_carved_relief() {
        let textures = block_textures();

        for kind in BlockKind::ALL {
            let spec = kind.spec();
            let flat = spec.finish.reflective() || spec.model.is_some();

            assert_eq!(
                textures[kind.slot() as usize].carved(),
                !flat,
                "{kind:?} quedo del lado equivocado"
            );
        }

        // y aun asi la mayoria de los bloques lo lleva: si no, el paralaje
        // no estaria haciendo nada en la escena
        let carved = BlockKind::ALL
            .iter()
            .filter(|kind| textures[kind.slot() as usize].carved())
            .count();
        assert!(carved * 3 > BlockKind::ALL.len(), "solo {carved} bloques tallados");
    }

    /// Y el material del bloque arrastra su textura: si no, la escena se
    /// dibujaria con los colores planos otra vez.
    #[test]
    fn the_block_material_carries_its_texture() {
        for kind in BlockKind::ALL {
            let material = kind.material(white());
            assert_eq!(material.texture, kind.slot(), "{kind:?}");
            assert_ne!(material.texture, PLAIN_TEXTURE, "{kind:?}");
        }
    }

    // -- la geometria del pack ---------------------------------------------

    /// Lo que cambia con el pack: un bloque que el pack decora deja de ser un
    /// cubo y pasa a ser decenas de cajitas, y uno que no, sigue siendo uno.
    #[test]
    fn the_pack_turns_decorated_blocks_into_real_geometry() {
        let models = block_models();
        let terrain = Terrain::generate(42);
        let mut blocks = Vec::new();
        let top = Faces::NONE.with(Face::Up);

        terrain.push_block(&mut blocks, &models, 20, 0, 20, BlockKind::Cobblestone, top);
        let carved = blocks.len();
        assert!(carved > 20, "el adoquin salio con {carved} cajitas");

        blocks.clear();
        terrain.push_block(&mut blocks, &models, 20, 0, 20, BlockKind::Dirt, top);
        assert_eq!(blocks.len(), 1, "la tierra no la decora el pack");

        // y solo se paga la decoracion de las caras destapadas
        blocks.clear();
        terrain.push_block(
            &mut blocks,
            &models,
            20,
            0,
            20,
            BlockKind::Cobblestone,
            Faces::NONE,
        );
        assert_eq!(blocks.len(), 1, "tapado por todos lados queda el cuerpo solo");
    }

    /// La decoracion tiene que salirse del bloque: eso es lo que le rompe la
    /// silueta, y es exactamente lo que el mapa de alturas no podia dar.
    #[test]
    fn the_decoration_breaks_the_silhouette_of_the_block() {
        let models = block_models();
        let terrain = Terrain::generate(42);
        let mut blocks = Vec::new();
        terrain.push_block(
            &mut blocks,
            &models,
            20,
            0,
            20,
            BlockKind::Cobblestone,
            Faces::ALL,
        );

        let corner = block_corner(20, 0, 20);
        let mut outside = 0;

        for block in &blocks {
            let (min, max) = block.bounds();
            // nada se va mas de un texel: si se fuera, la decoracion de dos
            // bloques vecinos se atravesaria
            assert!(min.x >= corner.x - 0.1 && max.x <= corner.x + BLOCK + 0.1);
            assert!(min.y >= corner.y - 0.1 && max.y <= corner.y + BLOCK + 0.1);
            assert!(min.z >= corner.z - 0.1 && max.z <= corner.z + BLOCK + 0.1);

            if min.x < corner.x || min.y < corner.y || min.z < corner.z {
                outside += 1;
            }
        }

        assert!(outside > 0, "ninguna cajita sobresale: la silueta quedaria recta");
    }

    /// Y la cuenta final: la escena entera tiene que caber en algo que el
    /// BVH pueda recorrer a tiempo.
    #[test]
    fn the_whole_scene_fits_in_a_workable_number_of_bodies() {
        let models = block_models();
        let blocks = Terrain::generate(42).blocks(&models);

        println!("cuerpos de la escena: {}", blocks.len());
        assert!(!blocks.is_empty());
        assert!(
            blocks.len() < 600_000,
            "la escena salio con {} cuerpos",
            blocks.len()
        );
    }

    // -- la forma del relieve ----------------------------------------------

    /// Lo que se le pidio al terreno: llanuras de verdad y montanas de
    /// verdad, no lomas de media altura de punta a punta.
    ///
    /// Se mide contra el piso de la propia isla y no contra un numero fijo:
    /// lo que importa es como se reparten las columnas entre el piso y la
    /// cima, y eso no cambia si manana un bioma sube o baja entero.
    #[test]
    fn every_island_has_both_flatland_and_mountains() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            for island in ISLANDS {
                let mut tops: Vec<i32> = columns()
                    .filter(|&(x, z)| terrain.biome(x, z) == Some(island.biome))
                    .map(|(x, z)| terrain.top(x, z))
                    .collect();
                tops.sort_unstable();

                // el percentil 5 y no el minimo: una sola columna hundida no
                // define el piso de una isla
                let floor = tops[tops.len() / 20];
                let relief = tops[tops.len() - 1] - floor;

                let flat = tops.iter().filter(|top| **top - floor <= 4).count();
                let high = tops.iter().filter(|top| **top - floor > relief / 2).count();

                assert!(
                    relief >= 18,
                    "semilla {seed}: {:?} tiene {relief} bloques de relieve",
                    island.biome
                );
                assert!(
                    flat * 3 > tops.len(),
                    "semilla {seed}: {:?} tiene {flat} columnas de llano sobre {}",
                    island.biome,
                    tops.len()
                );
                assert!(
                    high * 40 > tops.len(),
                    "semilla {seed}: {:?} tiene {high} columnas de montana sobre {}",
                    island.biome,
                    tops.len()
                );
            }
        }
    }

    /// Y ninguna columna puede quedar parada sola como una aguja.
    ///
    /// Es lo que pasa si se lee el ruido derecho: sale una columna veinte
    /// bloques por encima de sus cuatro vecinas, que en un mapa de bloques
    /// no es un cerro, es un poste. Contra eso esta `erode`. Un acantilado,
    /// en cambio, tiene que sobrevivir, asi que el limite no es cero: es que
    /// ninguna columna se despegue de TODAS sus vecinas a la vez.
    ///
    /// El cielo no cuenta como vecina: el corte del borde de la isla no es
    /// un accidente del relieve, es donde se termina la isla.
    #[test]
    fn no_column_stands_alone_like_a_needle() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            for (x, z) in columns() {
                if x == 0 || z == 0 || x + 1 == SIZE || z + 1 == SIZE {
                    continue;
                }
                let Some(island) = terrain.island(x, z) else {
                    continue;
                };

                let lowest = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .map(|(dx, dz)| ((x as i32 + dx) as usize, (z as i32 + dz) as usize))
                    .filter(|&(nx, nz)| terrain.island(nx, nz).is_some())
                    .map(|(nx, nz)| terrain.top(nx, nz))
                    .min();

                let Some(lowest) = lowest else {
                    continue;
                };
                let stands = terrain.top(x, z) - lowest;

                assert!(
                    stands <= 12,
                    "semilla {seed}: ({x}, {z}) de {:?} sobresale {stands} bloques de todas sus vecinas",
                    island.biome
                );
            }
        }
    }

    /// La costa no puede dejar columnas sueltas. En un mapa plano seria una
    /// piedrita en el agua; aca cada columna se lleva su quilla entera y
    /// queda una aguja de treinta bloques flotando al lado de la isla.
    #[test]
    fn the_shore_leaves_no_loose_columns() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            for (x, z) in columns() {
                if x == 0 || z == 0 || x + 1 == SIZE || z + 1 == SIZE {
                    continue;
                }
                if terrain.island(x, z).is_none() {
                    continue;
                }

                let neighbours = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .filter(|(dx, dz)| {
                        terrain
                            .island((x as i32 + dx) as usize, (z as i32 + dz) as usize)
                            .is_some()
                    })
                    .count();

                assert!(neighbours >= 2, "semilla {seed}: ({x}, {z}) quedo suelta");
            }
        }
    }

    /// La erosion no puede comerse el borde de la isla. Si promediara contra
    /// el cielo, cada columna de la orilla se llevaria puesta la altura del
    /// aire de al lado y las islas terminarian con forma de platillo.
    #[test]
    fn erosion_leaves_the_shore_where_it_was() {
        let mut surface = vec![0.0f32; SIZE * SIZE];
        let mut home = vec![None; SIZE * SIZE];

        // una isla cuadrada de 6x6 a la misma altura, en el medio del mapa
        for z in 20..26 {
            for x in 20..26 {
                surface[z * SIZE + x] = 10.0;
                home[z * SIZE + x] = Some(0);
            }
        }

        erode(&mut surface, &home);

        for z in 20..26 {
            for x in 20..26 {
                let height = surface[z * SIZE + x];
                assert!(
                    (height - 10.0).abs() < 1e-3,
                    "({x}, {z}) bajo a {height} sin que hubiera pendiente"
                );
            }
        }
    }

    // -- las lamparas marinas ----------------------------------------------

    /// Pocas y solo en la isla helada. Pocas no es un capricho: cada lampara
    /// es ademas una luz mas que la escena consulta en cada punto que
    /// sombrea.
    #[test]
    fn a_handful_of_sea_lanterns_light_the_ice_island() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);
            let places = &terrain.lamps;

            assert!(
                (2..=8).contains(&places.len()),
                "semilla {seed}: salieron {} lamparas",
                places.len()
            );

            for &(x, z) in places {
                assert_eq!(
                    terrain.biome(x, z),
                    Some(Biome::Ice),
                    "semilla {seed}: la lampara de ({x}, {z}) cayo fuera de la isla helada"
                );
                assert_eq!(
                    terrain.surface_kind(x, z),
                    BlockKind::SeaLantern,
                    "semilla {seed}: ({x}, {z}) no quedo cubierta por la lampara"
                );
            }

            // y repartidas, no amontonadas en un rincon
            for (first, &(x, z)) in places.iter().enumerate() {
                for &(ox, oz) in &places[first + 1..] {
                    let apart = (ox as i32 - x as i32).abs().max((oz as i32 - z as i32).abs());
                    assert!(apart >= LAMP_SPACING, "semilla {seed}: dos a {apart} bloques");
                }
            }
        }
    }

    /// La luz de cada lampara va por fuera del bloque, no en su centro.
    ///
    /// Puesta adentro, el propio bloque se interpone entre ella y todo lo que
    /// tiene alrededor: el rayo de sombra pega contra la lampara misma y la
    /// escena queda con una lampara encendida que no ilumina nada.
    #[test]
    fn every_lamp_light_sits_outside_its_own_block() {
        let terrain = Terrain::generate(42);
        let lights = terrain.lamps();

        assert_eq!(lights.len(), terrain.lamps.len());

        for (light, &(x, z)) in lights.iter().zip(&terrain.lamps) {
            let corner = block_corner(x, terrain.top(x, z), z);

            // aca -Y es arriba, asi que la cara de arriba del bloque es su Y
            // minimo y la luz tiene que quedar por encima
            assert!(
                light.y < corner.y,
                "la luz de ({x}, {z}) quedo adentro del bloque: {} contra {}",
                light.y,
                corner.y
            );
            assert!(
                corner.y - light.y < BLOCK,
                "y no puede irse un bloque entero para arriba"
            );

            // pero sigue centrada sobre la columna
            assert!((light.x - (corner.x + BLOCK * 0.5)).abs() < 1e-3);
            assert!((light.z - (corner.z + BLOCK * 0.5)).abs() < 1e-3);
        }
    }

    /// Y el bloque emite luz por su cuenta, que es lo que lo hace una lampara
    /// y no una piedra blanca.
    #[test]
    fn the_sea_lantern_is_the_only_block_that_emits() {
        let white = Color::new(255, 255, 255, 255);

        assert!(BlockKind::SeaLantern.material(white).emission > 0.0);

        for kind in BlockKind::ALL {
            if kind == BlockKind::SeaLantern {
                continue;
            }
            assert_eq!(
                kind.material(white).emission,
                0.0,
                "{kind:?} no tendria que emitir"
            );
        }
    }
}
