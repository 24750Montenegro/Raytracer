// terrain.rs
// Terreno de bloques generado con ruido: cuatro biomas de 24x24 repartidos en
// un plano de 48x48.
//
// Los cuatro comparten el mismo campo de relieve y solo cambian como lo leen,
// asi que en la frontera los pesos se mezclan y el terreno cruza de un bioma
// al otro sin escalones.
//
// Solo se generan los bloques que dan al aire: los de adentro del cerro no se
// ven nunca y meterlos en el BVH solo haria mas lento cada rayo.
use raylib::prelude::*;

use crate::material::Material;
use crate::noise::Noise;
use crate::object::Object;

/// Lado de cada bioma, en bloques.
pub const BIOME_SIZE: usize = 24;
/// Lado del plano entero: los biomas van en una grilla de 2x2.
pub const SIZE: usize = BIOME_SIZE * 2;

/// Lado de cada bloque. Van pegados uno al lado del otro: con separacion se
/// colaria el cielo por las junturas, porque lo tapado no se genera.
pub const BLOCK: f32 = 1.0;

const MAX_HEIGHT: i32 = 16;
/// Ninguna columna queda vacia: hasta el fondo del mar tiene suelo.
const MIN_HEIGHT: i32 = 1;

/// Hasta que nivel llega el agua. Lo que quede por debajo se inunda.
const SEA_LEVEL: i32 = 4;
/// Cuantos bloques por encima del agua sigue habiendo arena.
const BEACH_BAND: i32 = 1;
const SNOW_LINE: i32 = 9;

/// Ancho de la franja donde se mezclan dos biomas vecinos.
const BLEND: f32 = 7.0;
/// Cuanto se ondula la frontera, en bloques: sin esto el corte entre biomas
/// seria una linea recta y se veria como el borde de una baldosa.
const BORDER_WOBBLE: f32 = 3.5;
const BORDER_SCALE: f32 = 8.0;

/// Estira el ruido para que cada bioma use todo su rango de alturas.
const RELIEF_GAIN: f32 = 0.78;

/// Bloques de relleno debajo de la superficie antes de la roca.
const SUBSOIL: i32 = 3;

const PATCH_SCALE: f32 = 6.0;
/// Umbrales del campo de manchas: de el salen el hielo entre la nieve y el
/// ripio entre la piedra.
const PATCH_PEAK_ICE: f32 = 0.1;
const PATCH_FLOE_ICE: f32 = 0.35;
const PATCH_GRAVEL: f32 = -0.25;
const PATCH_SANDSTONE: f32 = -0.3;

/// Que tan seguido aparece cada veta en la piedra de las minas, acumulado.
const VEIN_GOLD: f32 = 0.015;
const VEIN_IRON: f32 = 0.05;
const VEIN_COAL: f32 = 0.13;

const TREE_CHANCE: f32 = 0.06;
const TREE_SPACING: i32 = 4;
const TRUNK_HEIGHT: i32 = 4;

const CACTUS_CHANCE: f32 = 0.05;
const CACTUS_SPACING: i32 = 4;

const SPIKE_CHANCE: f32 = 0.05;
const SPIKE_SPACING: i32 = 5;

/// Cuanto varia el tono de un bloque al siguiente. En Minecraft esa variacion
/// la da la textura; sin ella una pradera se ve como una chapa de un color.
const COLOR_JITTER: f32 = 0.07;

/// Semillas de los campos derivados: cualquier numero distinto sirve.
const PATCH_SALT: u64 = 0x1ce;
const GRAIN_SALT: u64 = 0x9ea1;
const BORDER_SALT: u64 = 0xb0d3;

/// Alturas de sorteo para separar los hashes que comparten el campo `grain`.
const DRAW_TREE: i32 = -1;
const DRAW_CACTUS: i32 = -2;
const DRAW_SPIKE: i32 = -3;
const DRAW_VEIN: i32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Biome {
    /// El de siempre: pasto, playa, bosque y cimas nevadas.
    Forest,
    Desert,
    Ice,
    /// Roca partida con vetas a la vista, como una mina a cielo abierto.
    Mines,
}

/// Como lee cada bioma el campo de relieve.
struct Profile {
    /// Cuantos bloques mide un accidente del terreno.
    scale: f32,
    octaves: u32,
    /// Por encima de 1 hunde los valles y afila las cimas.
    curve: f32,
    /// Altura minima del bioma y cuanto puede subir por encima de ella.
    base: f32,
    span: f32,
    /// Pliega el ruido sobre si mismo: deja filos en vez de lomas, que es lo
    /// que distingue a la roca partida de una colina.
    ridged: bool,
}

impl Biome {
    fn profile(self) -> Profile {
        match self {
            Biome::Forest => Profile {
                scale: 9.0,
                octaves: 4,
                curve: 1.6,
                base: 0.0,
                span: 14.0,
                ridged: false,
            },
            Biome::Desert => Profile {
                scale: 13.0,
                octaves: 2,
                curve: 1.2,
                base: 3.0,
                span: 9.0,
                ridged: false,
            },
            Biome::Ice => Profile {
                scale: 10.0,
                octaves: 3,
                curve: 1.7,
                base: 1.0,
                span: 11.0,
                ridged: false,
            },
            Biome::Mines => Profile {
                scale: 8.0,
                octaves: 3,
                curve: 1.5,
                base: 1.0,
                span: 10.0,
                ridged: true,
            },
        }
    }

    /// Altura que le daria este bioma a la columna, sin redondear.
    fn height_at(self, relief: &Noise, x: usize, z: usize) -> f32 {
        let profile = self.profile();
        let field = relief.fbm(
            x as f32 / profile.scale,
            z as f32 / profile.scale,
            profile.octaves,
        );

        let shaped = if profile.ridged {
            1.0 - field.abs()
        } else {
            field * RELIEF_GAIN + 0.5
        };

        profile.base + shaped.clamp(0.0, 1.0).powf(profile.curve) * profile.span
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Water,
    Sand,
    Sandstone,
    Cactus,
    Grass,
    Dirt,
    Stone,
    Gravel,
    CoalOre,
    IronOre,
    GoldOre,
    Snow,
    Ice,
    Wood,
    Leaves,
}

impl BlockKind {
    pub fn color(self) -> Color {
        match self {
            BlockKind::Water => Color::new(58, 116, 184, 255),
            BlockKind::Sand => Color::new(214, 198, 142, 255),
            BlockKind::Sandstone => Color::new(198, 176, 118, 255),
            BlockKind::Cactus => Color::new(78, 132, 62, 255),
            BlockKind::Grass => Color::new(96, 154, 66, 255),
            BlockKind::Dirt => Color::new(133, 96, 66, 255),
            BlockKind::Stone => Color::new(140, 142, 146, 255),
            BlockKind::Gravel => Color::new(150, 144, 136, 255),
            BlockKind::CoalOre => Color::new(74, 74, 78, 255),
            BlockKind::IronOre => Color::new(174, 140, 110, 255),
            BlockKind::GoldOre => Color::new(214, 176, 72, 255),
            BlockKind::Snow => Color::new(236, 242, 248, 255),
            BlockKind::Ice => Color::new(151, 192, 221, 255),
            BlockKind::Wood => Color::new(101, 76, 47, 255),
            BlockKind::Leaves => Color::new(64, 114, 50, 255),
        }
    }

    /// La tierra y la piedra son mates; los unicos que valen el trazado son
    /// el agua, el hielo, el oro de las vetas y la nieve.
    pub fn material(self, diffuse: Color) -> Material {
        match self {
            BlockKind::Water => Material::water(diffuse),
            BlockKind::Ice | BlockKind::GoldOre => Material::mirror(diffuse),
            BlockKind::Snow => Material::plastic(diffuse),
            _ => Material::matte(diffuse),
        }
    }
}

pub struct Terrain {
    /// Cuantos bloques mide cada columna: la de (x, z) ocupa los niveles
    /// 0..height, o sea que su bloque de arriba esta en height - 1.
    heights: Vec<i32>,
    /// A que bioma pertenece cada columna, ya resuelto el empate del borde.
    biomes: Vec<Biome>,
    /// Manchas de un segundo material adentro de cada bioma.
    patches: Noise,
    /// Sorteos sueltos: el tono de cada bloque, las vetas y las plantas.
    grain: Noise,
}

impl Terrain {
    pub fn generate(seed: u64) -> Self {
        let relief = Noise::new(seed);
        let border = relief.variant(BORDER_SALT);

        let mut heights = Vec::with_capacity(SIZE * SIZE);
        let mut biomes = Vec::with_capacity(SIZE * SIZE);

        for z in 0..SIZE {
            for x in 0..SIZE {
                let weights = biome_weights(&border, x, z);

                let height: f32 = weights
                    .iter()
                    .filter(|(_, weight)| *weight > 0.0)
                    .map(|(biome, weight)| weight * biome.height_at(&relief, x, z))
                    .sum();

                heights.push((height.round() as i32).clamp(MIN_HEIGHT, MAX_HEIGHT));
                biomes.push(dominant(&weights));
            }
        }

        Terrain {
            heights,
            biomes,
            patches: relief.variant(PATCH_SALT),
            grain: relief.variant(GRAIN_SALT),
        }
    }

    pub fn height(&self, x: usize, z: usize) -> i32 {
        self.heights[z * SIZE + x]
    }

    pub fn biome(&self, x: usize, z: usize) -> Biome {
        self.biomes[z * SIZE + x]
    }

    pub fn blocks(&self) -> Vec<Object> {
        let mut blocks = Vec::new();
        self.push_ground(&mut blocks);
        self.push_water(&mut blocks);
        self.push_plants(&mut blocks);
        blocks
    }

    /// De cada columna sale el bloque de arriba y los de los costados que
    /// alguna columna vecina mas baja deja destapados. El relleno no.
    fn push_ground(&self, blocks: &mut Vec<Object>) {
        for z in 0..SIZE {
            for x in 0..SIZE {
                let height = self.height(x, z);
                let top = height - 1;
                let buried_up_to = self.lowest_neighbour(x, z, |terrain, x, z| terrain.height(x, z));

                for level in 0..height {
                    if level != top && level < buried_up_to {
                        continue;
                    }
                    blocks.push(self.block(x, level, z, self.kind_at(x, z, level)));
                }
            }
        }
    }

    /// Llena hasta SEA_LEVEL toda columna que no llegue a esa altura. Con el
    /// mismo recorte que el terreno, de una laguna queda la lamina de arriba
    /// y, en el borde del mapa, el corte lateral.
    fn push_water(&self, blocks: &mut Vec<Object>) {
        for z in 0..SIZE {
            for x in 0..SIZE {
                let height = self.height(x, z);
                if height > SEA_LEVEL {
                    continue;
                }

                let frozen = self.biome(x, z) == Biome::Ice;
                let buried_up_to = self.lowest_neighbour(x, z, Terrain::flooded_height);

                for level in height..=SEA_LEVEL {
                    if level != SEA_LEVEL && level < buried_up_to {
                        continue;
                    }
                    // en el bioma helado la lamina de arriba amanece congelada
                    let kind = if frozen && level == SEA_LEVEL {
                        BlockKind::Ice
                    } else {
                        BlockKind::Water
                    };
                    blocks.push(self.block(x, level, z, kind));
                }
            }
        }
    }

    /// Lo que crece sobre el suelo, distinto en cada bioma.
    fn push_plants(&self, blocks: &mut Vec<Object>) {
        for (x, z) in self.scattered(TREE_CHANCE, TREE_SPACING, DRAW_TREE, Terrain::holds_tree) {
            self.push_tree(x, z, blocks);
        }
        for (x, z) in self.scattered(
            CACTUS_CHANCE,
            CACTUS_SPACING,
            DRAW_CACTUS,
            Terrain::holds_cactus,
        ) {
            self.push_column(x, z, 2, 3, BlockKind::Cactus, blocks);
        }
        for (x, z) in self.scattered(SPIKE_CHANCE, SPIKE_SPACING, DRAW_SPIKE, Terrain::holds_spike)
        {
            self.push_column(x, z, 3, 7, BlockKind::Ice, blocks);
        }
    }

    /// Columnas sueltas repartidas por el mapa, con una separacion minima
    /// para que no se amontonen. El margen del borde deja adentro del plano
    /// lo que sobresalga un bloque.
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
        self.well_inside(x, z, Biome::Desert) && self.height(x, z) - 1 > SEA_LEVEL + BEACH_BAND
    }

    fn holds_spike(&self, x: usize, z: usize) -> bool {
        self.well_inside(x, z, Biome::Ice) && self.height(x, z) - 1 > SEA_LEVEL
    }

    /// La columna y sus cuatro vecinas son del mismo bioma. Lo que crece
    /// asoma un bloque para los costados, asi que plantarlo justo sobre la
    /// frontera lo dejaria pisando el bioma de al lado.
    fn well_inside(&self, x: usize, z: usize, biome: Biome) -> bool {
        self.biome(x, z) == biome
            && [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().all(|&(dx, dz)| {
                let (nx, nz) = (x as i32 + dx, z as i32 + dz);
                self.biome(nx as usize, nz as usize) == biome
            })
    }

    /// Tronco recto y una copa de hojas alrededor de la punta.
    fn push_tree(&self, x: usize, z: usize, blocks: &mut Vec<Object>) {
        let base = self.height(x, z);
        let crown = base + TRUNK_HEIGHT - 1;

        for level in base..=crown {
            blocks.push(self.block(x, level, z, BlockKind::Wood));
        }

        for level in [crown - 1, crown] {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dz == 0 {
                        continue; // ahi esta el tronco
                    }
                    let (lx, lz) = ((x as i32 + dx) as usize, (z as i32 + dz) as usize);
                    blocks.push(self.block(lx, level, lz, BlockKind::Leaves));
                }
            }
        }

        // la tapa: una cruz, que es lo que le da la punta redondeada
        for (dx, dz) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (lx, lz) = ((x as i32 + dx) as usize, (z as i32 + dz) as usize);
            blocks.push(self.block(lx, crown + 1, lz, BlockKind::Leaves));
        }
    }

    /// Una pila de un bloque de ancho, de alto sorteado: el cactus del
    /// desierto y la aguja de hielo del bioma helado.
    fn push_column(
        &self,
        x: usize,
        z: usize,
        shortest: i32,
        tallest: i32,
        kind: BlockKind,
        blocks: &mut Vec<Object>,
    ) {
        let draw = self.grain.hash_unit(x as i32, tallest, z as i32);
        let height = shortest + (draw * (tallest - shortest) as f32) as i32;
        let base = self.height(x, z);

        for level in base..base + height {
            blocks.push(self.block(x, level, z, kind));
        }
    }

    /// Altura de la columna vecina mas baja, medida con la regla que le pase
    /// quien pregunta. Sobre el borde del mapa devuelve 0: ahi no hay vecino
    /// que tape nada, asi que el contorno se genera entero y el plano no
    /// queda como una cascara hueca.
    fn lowest_neighbour(
        &self,
        x: usize,
        z: usize,
        height_of: impl Fn(&Terrain, usize, usize) -> i32,
    ) -> i32 {
        let mut lowest = i32::MAX;

        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let (nx, nz) = (x as i32 + dx, z as i32 + dz);
            if nx < 0 || nz < 0 || nx >= SIZE as i32 || nz >= SIZE as i32 {
                return 0;
            }
            lowest = lowest.min(height_of(self, nx as usize, nz as usize));
        }

        lowest
    }

    /// Niveles ocupados de la columna contando el agua.
    fn flooded_height(&self, x: usize, z: usize) -> i32 {
        self.height(x, z).max(SEA_LEVEL + 1)
    }

    fn kind_at(&self, x: usize, z: usize, level: i32) -> BlockKind {
        let top = self.height(x, z) - 1;

        if level == top {
            return self.with_vein(x, z, level, self.surface_kind(x, z));
        }

        let depth = top - level;
        let under = match self.biome(x, z) {
            Biome::Forest if depth <= SUBSOIL => match self.surface_kind(x, z) {
                BlockKind::Sand => BlockKind::Sand,
                BlockKind::Snow | BlockKind::Ice => BlockKind::Stone,
                _ => BlockKind::Dirt,
            },
            Biome::Desert if depth <= 1 => BlockKind::Sand,
            Biome::Desert if depth <= SUBSOIL + 2 => BlockKind::Sandstone,
            _ => BlockKind::Stone,
        };

        self.with_vein(x, z, level, under)
    }

    /// Con que se cubre la columna, segun su bioma y a que altura termina.
    fn surface_kind(&self, x: usize, z: usize) -> BlockKind {
        let top = self.height(x, z) - 1;
        let patch = self.patch(x, z);

        match self.biome(x, z) {
            Biome::Forest => {
                if top <= SEA_LEVEL + BEACH_BAND {
                    BlockKind::Sand
                } else if top >= SNOW_LINE {
                    self.snow_or_ice(patch, PATCH_PEAK_ICE)
                } else {
                    BlockKind::Grass
                }
            }
            Biome::Desert => {
                if patch < PATCH_SANDSTONE {
                    BlockKind::Sandstone
                } else {
                    BlockKind::Sand
                }
            }
            Biome::Ice => {
                if top <= SEA_LEVEL {
                    BlockKind::Gravel // el fondo que se ve por debajo del hielo
                } else {
                    self.snow_or_ice(patch, PATCH_FLOE_ICE)
                }
            }
            Biome::Mines => {
                if patch < PATCH_GRAVEL {
                    BlockKind::Gravel
                } else {
                    BlockKind::Stone
                }
            }
        }
    }

    fn snow_or_ice(&self, patch: f32, threshold: f32) -> BlockKind {
        if patch > threshold {
            BlockKind::Ice
        } else {
            BlockKind::Snow
        }
    }

    /// En las minas la roca sale con vetas a la vista. Como solo se generan
    /// los bloques destapados, las vetas aparecen justo donde se las puede
    /// ver, que es lo que hace que el bioma se lea como una cantera.
    fn with_vein(&self, x: usize, z: usize, level: i32, kind: BlockKind) -> BlockKind {
        if kind != BlockKind::Stone || self.biome(x, z) != Biome::Mines {
            return kind;
        }

        match self.grain.hash_unit(x as i32, level + DRAW_VEIN, z as i32) {
            draw if draw < VEIN_GOLD => BlockKind::GoldOre,
            draw if draw < VEIN_IRON => BlockKind::IronOre,
            draw if draw < VEIN_COAL => BlockKind::CoalOre,
            _ => BlockKind::Stone,
        }
    }

    /// Campo suave que reparte el segundo material de cada bioma. Va aparte
    /// del relieve: si dependiera de la altura, todas las manchas caerian en
    /// las mismas partes del mapa.
    fn patch(&self, x: usize, z: usize) -> f32 {
        self.patches
            .sample(x as f32 / PATCH_SCALE, z as f32 / PATCH_SCALE)
    }

    fn block(&self, x: usize, level: i32, z: usize, kind: BlockKind) -> Object {
        let draw = self.grain.hash_unit(x as i32, level, z as i32);
        let diffuse = tinted(kind.color(), draw);
        Object::cube(block_center(x, level, z), BLOCK, kind.material(diffuse))
    }
}

/// Cuanto pesa cada bioma en una columna. En el interior de un cuadrante da 1
/// para el suyo y 0 para el resto; cerca de la frontera se reparte, y esa
/// mezcla es la que empalma las alturas de los dos vecinos.
fn biome_weights(border: &Noise, x: usize, z: usize) -> [(Biome, f32); 4] {
    let tx = axis_blend(x as f32 + wobble(border, x, z, 0.0));
    let tz = axis_blend(z as f32 + wobble(border, x, z, 53.0));

    [
        (Biome::Forest, (1.0 - tx) * (1.0 - tz)),
        (Biome::Desert, tx * (1.0 - tz)),
        (Biome::Ice, (1.0 - tx) * tz),
        (Biome::Mines, tx * tz),
    ]
}

/// De que lado de la frontera cae una coordenada: 0 de un lado, 1 del otro y
/// una rampa suave en el medio.
fn axis_blend(value: f32) -> f32 {
    let border = BIOME_SIZE as f32 - 0.5;
    let t = ((value - border) / BLEND + 0.5).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn wobble(border: &Noise, x: usize, z: usize, shift: f32) -> f32 {
    border.sample(
        (x as f32 + shift) / BORDER_SCALE,
        (z as f32 - shift) / BORDER_SCALE,
    ) * BORDER_WOBBLE
}

fn dominant(weights: &[(Biome, f32); 4]) -> Biome {
    let mut best = weights[0];
    for &candidate in &weights[1..] {
        if candidate.1 > best.1 {
            best = candidate;
        }
    }
    best.0
}

/// El plano queda centrado en el origen y crece hacia -Y, que aca es arriba.
fn block_center(x: usize, level: i32, z: usize) -> Vector3 {
    let half = (SIZE as f32 - 1.0) * 0.5;

    Vector3::new(
        (x as f32 - half) * BLOCK,
        -(level as f32 + 0.5) * BLOCK,
        (z as f32 - half) * BLOCK,
    )
}

fn tinted(color: Color, draw: f32) -> Color {
    let factor = 1.0 + (draw - 0.5) * 2.0 * COLOR_JITTER;
    let channel = |value: u8| (value as f32 * factor).clamp(0.0, 255.0) as u8;

    Color::new(channel(color.r), channel(color.g), channel(color.b), 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_BIOMES: [Biome; 4] = [Biome::Forest, Biome::Desert, Biome::Ice, Biome::Mines];

    /// Semillas fijas para que los tests no dependan del sorteo del dia.
    const SEEDS: [u64; 8] = [1, 7, 42, 1234, 20240519, 777777, 8_675_309, u64::MAX];

    /// Una columna bien adentro de cada cuadrante.
    const CORES: [(usize, usize, Biome); 4] = [
        (6, 6, Biome::Forest),
        (41, 6, Biome::Desert),
        (6, 41, Biome::Ice),
        (41, 41, Biome::Mines),
    ];

    #[test]
    fn the_plane_holds_four_biomes_of_twenty_four() {
        let terrain = Terrain::generate(42);

        assert_eq!(BIOME_SIZE, 24);
        assert_eq!(SIZE, 48);
        assert_eq!(terrain.heights.len(), 48 * 48);
        assert_eq!(terrain.biomes.len(), 48 * 48);
    }

    /// Cada cuadrante es de su bioma, y los cuatro estan.
    #[test]
    fn each_quadrant_keeps_its_own_biome() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            for (x, z, biome) in CORES {
                assert_eq!(
                    terrain.biome(x, z),
                    biome,
                    "semilla {seed}: ({x}, {z}) tendria que ser {biome:?}"
                );
            }

            for biome in ALL_BIOMES {
                let columns = (0..SIZE)
                    .flat_map(|z| (0..SIZE).map(move |x| (x, z)))
                    .filter(|&(x, z)| terrain.biome(x, z) == biome)
                    .count();

                // la frontera ondula, asi que ningun cuadrante mide exacto
                // 24x24, pero tampoco se puede comer al vecino
                assert!(
                    (400..800).contains(&columns),
                    "semilla {seed}: {biome:?} ocupa {columns} columnas"
                );
            }
        }
    }

    #[test]
    fn every_column_has_a_sane_height() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);
            for z in 0..SIZE {
                for x in 0..SIZE {
                    let height = terrain.height(x, z);
                    assert!(
                        (MIN_HEIGHT..=MAX_HEIGHT).contains(&height),
                        "semilla {seed}: la columna ({x}, {z}) mide {height}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_same_seed_rebuilds_the_same_map() {
        let a = Terrain::generate(20240519);
        let b = Terrain::generate(20240519);

        assert_eq!(a.heights, b.heights);
        assert_eq!(a.blocks().len(), b.blocks().len());
    }

    #[test]
    fn different_seeds_build_different_maps() {
        for pair in SEEDS.windows(2) {
            let a = Terrain::generate(pair[0]);
            let b = Terrain::generate(pair[1]);
            assert_ne!(a.heights, b.heights, "semillas {} y {}", pair[0], pair[1]);
        }
    }

    /// Terreno, no ruido blanco. Sorteando cada columna por separado este
    /// promedio daria arriba de 4.
    #[test]
    fn neighbouring_columns_stay_close() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);
            let mut steps = 0;
            let mut total = 0;

            for z in 0..SIZE {
                for x in 1..SIZE {
                    total += (terrain.height(x, z) - terrain.height(x - 1, z)).abs();
                    steps += 1;
                }
            }

            let average = total as f32 / steps as f32;
            assert!(average < 1.6, "semilla {seed}: escalon promedio de {average}");
        }
    }

    /// Lo que justifica mezclar los pesos en la frontera: cruzarla no puede
    /// costar mas que caminar por adentro de un bioma. Contra un numero fijo
    /// no sirve medirlo, porque la roca partida de las minas ya salta sola.
    #[test]
    fn the_seam_between_biomes_is_not_a_cliff() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);
            let border = BIOME_SIZE;

            let mut seam = 0;
            let mut inland = 0;

            for other in 0..SIZE {
                seam += (terrain.height(border, other) - terrain.height(border - 1, other)).abs();
                seam += (terrain.height(other, border) - terrain.height(other, border - 1)).abs();
            }

            for z in 0..SIZE {
                for x in 1..SIZE {
                    inland += (terrain.height(x, z) - terrain.height(x - 1, z)).abs();
                }
            }

            let seam = seam as f32 / (2 * SIZE) as f32;
            let inland = inland as f32 / (SIZE * (SIZE - 1)) as f32;

            assert!(
                seam < inland * 2.0 + 0.5,
                "semilla {seed}: la costura salta {seam} contra {inland} tierra adentro"
            );
        }
    }

    #[test]
    fn the_map_has_relief() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);
            let low = *terrain.heights.iter().min().unwrap();
            let high = *terrain.heights.iter().max().unwrap();

            assert!(high - low >= 4, "semilla {seed}: el mapa va de {low} a {high}");
        }
    }

    #[test]
    fn the_maps_hold_water_and_dry_land() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            assert!(
                terrain.heights.iter().any(|height| *height <= SEA_LEVEL),
                "semilla {seed}: no hay una sola columna bajo el agua"
            );
            assert!(
                terrain.heights.iter().any(|height| *height > SEA_LEVEL + 1),
                "semilla {seed}: esta todo inundado"
            );
        }
    }

    /// Incluidas las hojas, que sobresalen un bloque del tronco.
    #[test]
    fn nothing_grows_outside_the_plane() {
        let limit = SIZE as f32 * BLOCK * 0.5;

        for seed in SEEDS {
            for block in Terrain::generate(seed).blocks() {
                let (min, max) = block.bounds();
                assert!(
                    min.x >= -limit - 1e-4 && max.x <= limit + 1e-4,
                    "semilla {seed}: un bloque se fue en x a {}..{}",
                    min.x,
                    max.x
                );
                assert!(
                    min.z >= -limit - 1e-4 && max.z <= limit + 1e-4,
                    "semilla {seed}: un bloque se fue en z a {}..{}",
                    min.z,
                    max.z
                );
            }
        }
    }

    #[test]
    fn every_column_shows_its_top_block() {
        let terrain = Terrain::generate(42);
        let blocks = terrain.blocks();

        for z in 0..SIZE {
            for x in 0..SIZE {
                let top = block_center(x, terrain.height(x, z) - 1, z);
                assert!(
                    blocks
                        .iter()
                        .any(|block| (block.center() - top).length() < 1e-4),
                    "falta la tapa de la columna ({x}, {z})"
                );
            }
        }
    }

    #[test]
    fn blocks_walled_in_on_every_side_are_left_out() {
        let terrain = Terrain::generate(42);
        let blocks = terrain.blocks();
        let mut buried = 0;

        for z in 1..SIZE - 1 {
            for x in 1..SIZE - 1 {
                let top = terrain.height(x, z) - 1;
                let lowest = terrain.lowest_neighbour(x, z, |t, x, z| t.height(x, z));

                for level in 0..top {
                    if level < lowest {
                        buried += 1;
                        let center = block_center(x, level, z);
                        assert!(
                            !blocks
                                .iter()
                                .any(|block| (block.center() - center).length() < 1e-4),
                            "el bloque ({x}, {level}, {z}) esta enterrado y se genero igual"
                        );
                    }
                }
            }
        }

        assert!(buried > 0, "el mapa de prueba no tenia nada enterrado");
    }

    #[test]
    fn the_water_stops_at_sea_level() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);
            let blocks = terrain.blocks();
            let mut surfaces = 0;

            for z in 0..SIZE {
                for x in 0..SIZE {
                    if terrain.height(x, z) > SEA_LEVEL {
                        continue;
                    }
                    surfaces += 1;

                    let top = block_center(x, SEA_LEVEL, z);
                    assert!(
                        blocks
                            .iter()
                            .any(|block| (block.center() - top).length() < 1e-4),
                        "semilla {seed}: la columna ({x}, {z}) quedo sin superficie"
                    );
                }
            }

            assert!(surfaces > 0, "semilla {seed}: no se genero nada de agua");
        }
    }

    /// En el bioma helado el agua amanece congelada, y el hielo es opaco: lo
    /// que se ve es el cielo reflejado, no el fondo.
    #[test]
    fn the_lakes_of_the_ice_biome_are_frozen_over(){
        let mut frozen = 0;

        for seed in SEEDS {
            let terrain = Terrain::generate(seed);
            let mut water = Vec::new();
            terrain.push_water(&mut water);

            for z in 0..SIZE {
                for x in 0..SIZE {
                    if terrain.height(x, z) > SEA_LEVEL || terrain.biome(x, z) != Biome::Ice {
                        continue;
                    }
                    frozen += 1;

                    let lid = block_center(x, SEA_LEVEL, z);
                    let block = water
                        .iter()
                        .find(|block| (block.center() - lid).length() < 1e-4)
                        .expect("la laguna helada quedo sin tapa");

                    assert_eq!(block.material().albedo.refraction, 0.0);
                    assert!(block.material().albedo.reflection > 0.0);
                }
            }
        }

        assert!(frozen > 0, "ninguna semilla dio una laguna en el hielo");
    }

    /// Cada bioma se cubre con lo suyo: si no, el mapa entero seria el mismo
    /// paisaje pintado de cuatro colores.
    #[test]
    fn each_biome_has_its_own_ground() {
        let terrain = Terrain::generate(42);

        for z in 0..SIZE {
            for x in 0..SIZE {
                let surface = terrain.surface_kind(x, z);
                let top = terrain.height(x, z) - 1;

                match terrain.biome(x, z) {
                    Biome::Forest => assert!(
                        matches!(
                            surface,
                            BlockKind::Grass | BlockKind::Sand | BlockKind::Snow | BlockKind::Ice
                        ),
                        "bosque con {surface:?} en ({x}, {z})"
                    ),
                    Biome::Desert => assert!(
                        matches!(surface, BlockKind::Sand | BlockKind::Sandstone),
                        "desierto con {surface:?} en ({x}, {z})"
                    ),
                    Biome::Ice => assert!(
                        matches!(
                            surface,
                            BlockKind::Snow | BlockKind::Ice | BlockKind::Gravel
                        ),
                        "hielo con {surface:?} en ({x}, {z})"
                    ),
                    Biome::Mines => assert!(
                        matches!(surface, BlockKind::Stone | BlockKind::Gravel),
                        "minas con {surface:?} en ({x}, {z})"
                    ),
                }

                if terrain.biome(x, z) == Biome::Forest && top <= SEA_LEVEL {
                    assert_eq!(surface, BlockKind::Sand, "fondo de agua en ({x}, {z})");
                }
            }
        }
    }

    /// En el bosque: pasto arriba, tierra abajo y piedra al fondo.
    #[test]
    fn the_forest_ground_is_layered() {
        let terrain = Terrain::generate(42);

        let (x, z) = (0..SIZE)
            .flat_map(|z| (0..SIZE).map(move |x| (x, z)))
            .filter(|&(x, z)| terrain.surface_kind(x, z) == BlockKind::Grass)
            .max_by_key(|&(x, z)| terrain.height(x, z))
            .expect("el mapa de prueba no tenia pasto");
        let top = terrain.height(x, z) - 1;
        assert!(top > SUBSOIL, "la columna elegida mide {top}");

        assert_eq!(terrain.kind_at(x, z, top), BlockKind::Grass);
        assert_eq!(terrain.kind_at(x, z, top - 1), BlockKind::Dirt, "debajo, tierra");
        assert_eq!(terrain.kind_at(x, z, 0), BlockKind::Stone, "el fondo es piedra");
    }

    /// Y en el desierto, arena sobre arenisca.
    #[test]
    fn the_desert_ground_is_layered() {
        let terrain = Terrain::generate(42);

        let (x, z) = (0..SIZE)
            .flat_map(|z| (0..SIZE).map(move |x| (x, z)))
            .filter(|&(x, z)| terrain.biome(x, z) == Biome::Desert)
            .max_by_key(|&(x, z)| terrain.height(x, z))
            .expect("el mapa de prueba no tenia desierto");
        let top = terrain.height(x, z) - 1;

        assert!(matches!(
            terrain.kind_at(x, z, top),
            BlockKind::Sand | BlockKind::Sandstone
        ));
        assert_eq!(terrain.kind_at(x, z, top - 1), BlockKind::Sand);
        assert_eq!(terrain.kind_at(x, z, top - 3), BlockKind::Sandstone);
    }

    #[test]
    fn the_peaks_are_rock_under_the_snow() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            for z in 0..SIZE {
                for x in 0..SIZE {
                    if terrain.biome(x, z) != Biome::Forest {
                        continue;
                    }
                    let top = terrain.height(x, z) - 1;
                    if !matches!(
                        terrain.surface_kind(x, z),
                        BlockKind::Snow | BlockKind::Ice
                    ) {
                        continue;
                    }

                    assert_eq!(
                        terrain.kind_at(x, z, top - 1),
                        BlockKind::Stone,
                        "semilla {seed}: la cima ({x}, {z}) tiene tierra debajo"
                    );
                }
            }
        }
    }

    /// Las vetas salen solo en las minas, y las tres clases aparecen.
    #[test]
    fn the_mines_show_their_veins() {
        let mut seen: Vec<BlockKind> = Vec::new();

        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            for z in 0..SIZE {
                for x in 0..SIZE {
                    for level in 0..terrain.height(x, z) {
                        let kind = terrain.kind_at(x, z, level);
                        if !matches!(
                            kind,
                            BlockKind::CoalOre | BlockKind::IronOre | BlockKind::GoldOre
                        ) {
                            continue;
                        }

                        assert_eq!(
                            terrain.biome(x, z),
                            Biome::Mines,
                            "semilla {seed}: una veta fuera de las minas en ({x}, {z})"
                        );
                        if !seen.contains(&kind) {
                            seen.push(kind);
                        }
                    }
                }
            }
        }

        for ore in [BlockKind::CoalOre, BlockKind::IronOre, BlockKind::GoldOre] {
            assert!(seen.contains(&ore), "no salio {ore:?} en ninguna semilla");
        }
    }

    /// Cada planta crece en su bioma, separada de las demas y sin pisar la
    /// frontera del vecino.
    #[test]
    fn every_biome_grows_what_belongs_to_it() {
        let mut totals = [0usize; 3];

        for seed in SEEDS {
            let terrain = Terrain::generate(seed);
            let plants = [
                (
                    Biome::Forest,
                    TREE_SPACING,
                    terrain.scattered(TREE_CHANCE, TREE_SPACING, DRAW_TREE, Terrain::holds_tree),
                ),
                (
                    Biome::Desert,
                    CACTUS_SPACING,
                    terrain.scattered(
                        CACTUS_CHANCE,
                        CACTUS_SPACING,
                        DRAW_CACTUS,
                        Terrain::holds_cactus,
                    ),
                ),
                (
                    Biome::Ice,
                    SPIKE_SPACING,
                    terrain.scattered(SPIKE_CHANCE, SPIKE_SPACING, DRAW_SPIKE, Terrain::holds_spike),
                ),
            ];

            for (slot, (biome, spacing, columns)) in plants.iter().enumerate() {
                totals[slot] += columns.len();

                for &(x, z) in columns {
                    assert_eq!(
                        terrain.biome(x, z),
                        *biome,
                        "semilla {seed}: una planta de {biome:?} en ({x}, {z})"
                    );
                    assert!(terrain.well_inside(x, z, *biome), "planta sobre la frontera");
                }

                for (i, &(x, z)) in columns.iter().enumerate() {
                    for &(ox, oz) in &columns[i + 1..] {
                        let reach =
                            (ox as i32 - x as i32).abs().max((oz as i32 - z as i32).abs());
                        assert!(reach >= *spacing, "semilla {seed}: dos plantas pegadas");
                    }
                }
            }
        }

        for (slot, total) in totals.iter().enumerate() {
            assert!(*total > 0, "el bioma {slot} no hizo crecer nada");
        }
    }

    #[test]
    fn plants_rest_on_the_ground() {
        let terrain = Terrain::generate(42);
        let (x, z) = terrain
            .scattered(TREE_CHANCE, TREE_SPACING, DRAW_TREE, Terrain::holds_tree)[0];

        let mut tree = Vec::new();
        terrain.push_tree(x, z, &mut tree);

        let lowest = tree
            .iter()
            .map(|block| block.bounds().1.y)
            .fold(f32::NEG_INFINITY, f32::max);
        // -Y es arriba: la base del arbol es la Y mas grande
        let ground = block_center(x, terrain.height(x, z) - 1, z).y - BLOCK * 0.5;

        assert!((lowest - ground).abs() < 1e-4, "{lowest} contra {ground}");
    }

    /// Un plano macizo de 48x48x16 serian mas de 36000 cubos, y eso lo paga
    /// cada rayo de cada frame.
    #[test]
    fn only_the_shell_of_the_map_gets_traced() {
        let solid = (SIZE * SIZE) as i32 * MAX_HEIGHT;

        for seed in SEEDS {
            let blocks = Terrain::generate(seed).blocks().len() as i32;

            assert!(blocks >= (SIZE * SIZE) as i32, "semilla {seed}: solo {blocks} bloques");
            assert!(blocks < solid / 3, "semilla {seed}: {blocks} bloques de {solid}");
        }
    }

    #[test]
    fn the_tint_only_nudges_the_colour() {
        let base = BlockKind::Grass.color();

        for draw in [0.0, 0.25, 0.5, 0.75, 0.999] {
            let shade = tinted(base, draw);
            let drift = (shade.g as i32 - base.g as i32).abs();
            assert!(drift <= (base.g as f32 * COLOR_JITTER) as i32 + 1, "corrio {drift}");
        }

        assert_ne!(tinted(base, 0.0).g, tinted(base, 1.0).g, "el tono no varia");
    }
}
