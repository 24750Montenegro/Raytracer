// terrain.rs
// Terreno de bloques generado con ruido, al estilo Minecraft.
//
// Solo se generan los bloques que dan al aire: los de adentro del cerro no se
// ven nunca y meterlos en el BVH solo haria mas lento cada rayo.
use raylib::prelude::*;

use crate::material::Material;
use crate::noise::Noise;
use crate::object::Object;

/// Lado del plano, en bloques. El mapa es SIZE x SIZE columnas.
pub const SIZE: usize = 24;

/// Lado de cada bloque. Van pegados uno al lado del otro: con separacion se
/// colaria el cielo por las junturas, porque lo tapado no se genera.
pub const BLOCK: f32 = 1.0;

const MAX_HEIGHT: i32 = 14;
/// Ninguna columna queda vacia: hasta el fondo del mar tiene suelo.
const MIN_HEIGHT: i32 = 1;

/// Hasta que nivel llega el agua. Lo que quede por debajo se inunda.
const SEA_LEVEL: i32 = 4;
/// Cuantos bloques por encima del agua sigue habiendo arena.
const BEACH_BAND: i32 = 1;
const SNOW_LINE: i32 = 9;

/// Cuantos bloques mide una colina, mas o menos. Mas chico, todo se vuelve
/// picos; mas grande, el mapa entero queda a la misma altura.
const RELIEF_SCALE: f32 = 9.0;
const RELIEF_OCTAVES: u32 = 4;
/// Estira el ruido para que el mapa use todo el rango de alturas.
const RELIEF_GAIN: f32 = 0.78;
/// Por encima de 1 hunde los valles y deja las cimas puntiagudas, que es lo
/// que separa el mar de las montanas en vez de dejar una loma pareja.
const RELIEF_CURVE: f32 = 1.6;

/// Bloques de tierra (o arena) debajo de la superficie antes de la piedra.
const SUBSOIL: i32 = 3;

const CLIMATE_SCALE: f32 = 6.0;
/// A partir de aca la cima es de hielo en vez de nieve.
const CLIMATE_ICE: f32 = 0.1;

const TREE_CHANCE: f32 = 0.06;
const TREE_SPACING: i32 = 4;
const TRUNK_HEIGHT: i32 = 4;

/// Cuanto varia el tono de un bloque al siguiente. En Minecraft esa variacion
/// la da la textura; sin ella una pradera se ve como una chapa de un color.
const COLOR_JITTER: f32 = 0.07;

/// Semillas de los campos derivados: cualquier par de numeros distintos sirve.
const CLIMATE_SALT: u64 = 0x1ce;
const GRAIN_SALT: u64 = 0x9ea1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Water,
    Sand,
    Grass,
    Dirt,
    Stone,
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
            BlockKind::Grass => Color::new(96, 154, 66, 255),
            BlockKind::Dirt => Color::new(133, 96, 66, 255),
            BlockKind::Stone => Color::new(124, 126, 130, 255),
            BlockKind::Snow => Color::new(236, 242, 248, 255),
            BlockKind::Ice => Color::new(151, 192, 221, 255),
            BlockKind::Wood => Color::new(101, 76, 47, 255),
            BlockKind::Leaves => Color::new(64, 114, 50, 255),
        }
    }

    /// La tierra y la piedra son mates; los unicos que valen el trazado son
    /// el agua, el hielo de las cimas y la nieve.
    pub fn material(self, diffuse: Color) -> Material {
        match self {
            BlockKind::Water => Material::water(diffuse),
            BlockKind::Ice => Material::mirror(diffuse),
            BlockKind::Snow => Material::plastic(diffuse),
            _ => Material::matte(diffuse),
        }
    }
}

pub struct Terrain {
    /// Cuantos bloques mide cada columna: la de (x, z) ocupa los niveles
    /// 0..height, o sea que su bloque de arriba esta en height - 1.
    heights: Vec<i32>,
    climate: Noise,
    /// Sorteos sueltos: el tono de cada bloque y donde cae un arbol.
    grain: Noise,
}

impl Terrain {
    pub fn generate(seed: u64) -> Self {
        let relief = Noise::new(seed);
        let mut heights = Vec::with_capacity(SIZE * SIZE);

        for z in 0..SIZE {
            for x in 0..SIZE {
                heights.push(column_height(&relief, x, z));
            }
        }

        Terrain {
            heights,
            climate: relief.variant(CLIMATE_SALT),
            grain: relief.variant(GRAIN_SALT),
        }
    }

    pub fn height(&self, x: usize, z: usize) -> i32 {
        self.heights[z * SIZE + x]
    }

    pub fn blocks(&self) -> Vec<Object> {
        let mut blocks = Vec::new();
        self.push_ground(&mut blocks);
        self.push_water(&mut blocks);
        self.push_trees(&mut blocks);
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

                let buried_up_to = self.lowest_neighbour(x, z, Terrain::flooded_height);

                for level in height..=SEA_LEVEL {
                    if level != SEA_LEVEL && level < buried_up_to {
                        continue;
                    }
                    blocks.push(self.block(x, level, z, BlockKind::Water));
                }
            }
        }
    }

    /// Sobre que columnas crece un arbol. El margen del borde deja las hojas
    /// adentro del plano: sobresalen un bloque del tronco.
    fn tree_columns(&self) -> Vec<(usize, usize)> {
        let mut planted: Vec<(usize, usize)> = Vec::new();

        for z in 1..SIZE - 1 {
            for x in 1..SIZE - 1 {
                if self.surface_kind(x, z) != BlockKind::Grass {
                    continue;
                }
                if self.grain.hash_unit(x as i32, -1, z as i32) >= TREE_CHANCE {
                    continue;
                }
                if planted.iter().any(|&(px, pz)| {
                    let reach = (px as i32 - x as i32).abs().max((pz as i32 - z as i32).abs());
                    reach < TREE_SPACING
                }) {
                    continue;
                }

                planted.push((x, z));
            }
        }

        planted
    }

    fn push_trees(&self, blocks: &mut Vec<Object>) {
        for (x, z) in self.tree_columns() {
            self.push_tree(x, z, blocks);
        }
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
        let surface = self.surface_kind(x, z);

        if level == top {
            return surface;
        }

        // pasto arriba, tierra debajo, piedra al fondo
        if level + SUBSOIL >= top {
            return match surface {
                BlockKind::Sand => BlockKind::Sand,
                BlockKind::Snow | BlockKind::Ice | BlockKind::Stone => BlockKind::Stone,
                _ => BlockKind::Dirt,
            };
        }

        BlockKind::Stone
    }

    /// Con que se cubre la columna, segun a que altura termina.
    fn surface_kind(&self, x: usize, z: usize) -> BlockKind {
        let top = self.height(x, z) - 1;

        if top <= SEA_LEVEL + BEACH_BAND {
            return BlockKind::Sand;
        }

        if top >= SNOW_LINE {
            return if self.cold(x, z) > CLIMATE_ICE {
                BlockKind::Ice
            } else {
                BlockKind::Snow
            };
        }

        BlockKind::Grass
    }

    /// Que tan fria es esa parte del mapa. Es un campo de ruido aparte del
    /// relieve: si dependiera de la altura, todas las cimas serian iguales.
    fn cold(&self, x: usize, z: usize) -> f32 {
        self.climate
            .sample(x as f32 / CLIMATE_SCALE, z as f32 / CLIMATE_SCALE)
    }

    fn block(&self, x: usize, level: i32, z: usize, kind: BlockKind) -> Object {
        let draw = self.grain.hash_unit(x as i32, level, z as i32);
        let diffuse = tinted(kind.color(), draw);
        Object::cube(block_center(x, level, z), BLOCK, kind.material(diffuse))
    }
}

fn column_height(relief: &Noise, x: usize, z: usize) -> i32 {
    let field = relief.fbm(
        x as f32 / RELIEF_SCALE,
        z as f32 / RELIEF_SCALE,
        RELIEF_OCTAVES,
    );

    // de [-1, 1] a [0, 1], estirado y curvado
    let normalized = (field * RELIEF_GAIN + 0.5).clamp(0.0, 1.0);
    let height = (normalized.powf(RELIEF_CURVE) * MAX_HEIGHT as f32).round() as i32;

    height.clamp(MIN_HEIGHT, MAX_HEIGHT)
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

    /// Semillas fijas para que los tests no dependan del sorteo del dia.
    const SEEDS: [u64; 8] = [1, 7, 42, 1234, 20240519, 777777, 8_675_309, u64::MAX];

    #[test]
    fn the_map_is_twenty_four_by_twenty_four() {
        let terrain = Terrain::generate(42);

        assert_eq!(SIZE, 24);
        assert_eq!(terrain.heights.len(), 24 * 24);
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
            assert!(average < 1.5, "semilla {seed}: escalon promedio de {average}");
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
                            !terrain
                                .blocks()
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
            let mut surfaces = 0;

            for z in 0..SIZE {
                for x in 0..SIZE {
                    if terrain.height(x, z) > SEA_LEVEL {
                        continue;
                    }
                    surfaces += 1;

                    let top = block_center(x, SEA_LEVEL, z);
                    assert!(
                        terrain
                            .blocks()
                            .iter()
                            .any(|block| (block.center() - top).length() < 1e-4),
                        "semilla {seed}: la columna ({x}, {z}) quedo sin superficie de agua"
                    );
                }
            }

            assert!(surfaces > 0, "semilla {seed}: no se genero nada de agua");
        }
    }

    #[test]
    fn height_decides_what_the_surface_is_made_of() {
        let terrain = Terrain::generate(42);

        for z in 0..SIZE {
            for x in 0..SIZE {
                let top = terrain.height(x, z) - 1;
                let surface = terrain.surface_kind(x, z);

                if top <= SEA_LEVEL {
                    assert_eq!(surface, BlockKind::Sand, "fondo de agua en ({x}, {z})");
                } else if top >= SNOW_LINE {
                    assert!(
                        matches!(surface, BlockKind::Snow | BlockKind::Ice),
                        "la cima ({x}, {z}) salio de {surface:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_ground_is_layered() {
        let terrain = Terrain::generate(42);

        // la columna de pasto mas alta: tiene lugar para las tres capas
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

    #[test]
    fn the_peaks_are_rock_under_the_snow() {
        for seed in SEEDS {
            let terrain = Terrain::generate(seed);

            for z in 0..SIZE {
                for x in 0..SIZE {
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

    #[test]
    fn trees_grow_spread_over_the_grass() {
        let mut total = 0;

        for seed in SEEDS {
            let terrain = Terrain::generate(seed);
            let trees = terrain.tree_columns();
            total += trees.len();

            for &(x, z) in &trees {
                assert_eq!(
                    terrain.surface_kind(x, z),
                    BlockKind::Grass,
                    "semilla {seed}: el arbol en ({x}, {z}) no esta sobre pasto"
                );
                assert!(
                    (1..SIZE - 1).contains(&x) && (1..SIZE - 1).contains(&z),
                    "semilla {seed}: el arbol en ({x}, {z}) se pasa del borde"
                );
            }

            for (i, &(x, z)) in trees.iter().enumerate() {
                for &(ox, oz) in &trees[i + 1..] {
                    let reach = (ox as i32 - x as i32).abs().max((oz as i32 - z as i32).abs());
                    assert!(reach >= TREE_SPACING, "semilla {seed}: dos arboles pegados");
                }
            }
        }

        assert!(total > 0, "no crecio un solo arbol en ninguna semilla");
    }

    #[test]
    fn the_trunk_rests_on_the_ground() {
        let terrain = Terrain::generate(42);
        let (x, z) = terrain.tree_columns()[0];

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

    /// Un plano macizo de 24x24x14 serian mas de 8000 cubos, y eso lo paga
    /// cada rayo de cada frame.
    #[test]
    fn only_the_shell_of_the_map_gets_traced() {
        let solid = (SIZE * SIZE) as i32 * MAX_HEIGHT;

        for seed in SEEDS {
            let blocks = Terrain::generate(seed).blocks().len() as i32;

            assert!(blocks >= (SIZE * SIZE) as i32, "semilla {seed}: solo {blocks} bloques");
            assert!(blocks < solid / 2, "semilla {seed}: {blocks} bloques de {solid}");
        }
    }

    #[test]
    fn the_maps_show_every_kind_of_ground() {
        let mut seen = Vec::new();

        for seed in SEEDS {
            let terrain = Terrain::generate(seed);
            for z in 0..SIZE {
                for x in 0..SIZE {
                    let kind = terrain.surface_kind(x, z);
                    if !seen.contains(&kind) {
                        seen.push(kind);
                    }
                }
            }
        }

        for kind in [BlockKind::Sand, BlockKind::Grass, BlockKind::Snow, BlockKind::Ice] {
            assert!(seen.contains(&kind), "no salio {kind:?} en ninguna semilla");
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
