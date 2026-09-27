// noise.rs
// Ruido de gradiente (Perlin) con semilla, y las tres formas de leerlo que
// usa el terreno:
//
//   - `fbm` suma octavas del mismo campo: la primera pone montanas y valles,
//     las siguientes el detalle.
//   - `warp` corre el punto de muestreo con el propio campo antes de leerlo.
//     Es lo que le saca al Perlin su aire de burbujas parejas.
//   - `ridged` lo pliega sobre si mismo: donde el campo cruza cero queda un
//     filo en vez de una cima redonda, que es la forma de una sierra.
//
// De aca sale la altura de cada columna del terreno.
use std::collections::hash_map::RandomState;
use std::f32::consts::{SQRT_2, TAU};
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cuanto se corre el campo de una octava a la siguiente. El ruido de
/// gradiente vale 0 sobre los puntos enteros de su grilla: sin el corrimiento
/// todas las octavas se anulan en los mismos lugares y el terreno sale con
/// una cuadricula de hoyos regulares.
const OCTAVE_SHIFT: f32 = 31.7;

/// Cuanto se aleja la lectura que deforma el dominio de la que se deforma.
/// Cualquier numero grande sirve: lo unico que importa es que las dos caigan
/// en partes del campo sin relacion entre si.
const WARP_OFFSET: f32 = 57.3;
/// La deformacion va con pocas octavas: lo que tiene que hacer es doblar el
/// campo en grande, no agregarle detalle.
const WARP_OCTAVES: u32 = 2;

/// splitmix64. Dos entradas parecidas tienen que dar salidas sin ninguna
/// relacion, o los gradientes quedan alineados y el ruido muestra rayas.
const fn mix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Semilla distinta en cada corrida, sin dependencias externas: RandomState
/// trae las claves que el proceso sortea al arrancar y el reloj desempata.
pub fn random_seed() -> u64 {
    let from_process = RandomState::new().build_hasher().finish();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0);

    mix64(from_process ^ mix64(nanos))
}

#[derive(Debug, Clone, Copy)]
pub struct Noise {
    seed: u64,
}

impl Noise {
    pub fn new(seed: u64) -> Self {
        Noise { seed }
    }

    /// Otro campo sacado de la misma semilla, independiente del primero: sirve
    /// para decidir cosas que no son la altura sin arrastrar el relieve.
    pub fn variant(&self, salt: u64) -> Self {
        Noise::new(mix64(self.seed ^ mix64(salt)))
    }

    /// Numero en [0, 1) atado a una posicion entera. No es ruido: no tiene
    /// ninguna continuidad. Es para los sorteos sueltos.
    pub fn hash_unit(&self, x: i32, y: i32, z: i32) -> f32 {
        let key = ((x as u32 as u64) << 40) ^ ((y as u32 as u64) << 20) ^ (z as u32 as u64);
        let bits = mix64(self.seed ^ mix64(key));
        (bits >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Gradiente unitario de una celda, sorteado a partir de su posicion: asi
    /// el campo se reproduce entero sin guardar ninguna tabla.
    fn gradient(&self, cell_x: i32, cell_z: i32) -> (f32, f32) {
        let key = ((cell_x as u32 as u64) << 32) | (cell_z as u32 as u64);
        let bits = mix64(self.seed ^ mix64(key));
        let angle = (bits >> 40) as f32 / (1u64 << 24) as f32 * TAU;
        angle.sin_cos()
    }

    fn corner(&self, cell_x: i32, cell_z: i32, dx: f32, dz: f32) -> f32 {
        let (gx, gz) = self.gradient(cell_x, cell_z);
        gx * dx + gz * dz
    }

    /// Valor del campo en un punto cualquiera, dentro de [-1, 1].
    pub fn sample(&self, x: f32, z: f32) -> f32 {
        let base_x = x.floor();
        let base_z = z.floor();
        let (cell_x, cell_z) = (base_x as i32, base_z as i32);
        let (dx, dz) = (x - base_x, z - base_z);

        let near = lerp(
            self.corner(cell_x, cell_z, dx, dz),
            self.corner(cell_x + 1, cell_z, dx - 1.0, dz),
            fade(dx),
        );
        let far = lerp(
            self.corner(cell_x, cell_z + 1, dx, dz - 1.0),
            self.corner(cell_x + 1, cell_z + 1, dx - 1.0, dz - 1.0),
            fade(dx),
        );

        // el maximo teorico del ruido de gradiente en 2D es sqrt(2)/2
        (lerp(near, far, fade(dz)) * SQRT_2).clamp(-1.0, 1.0)
    }

    /// Corre el punto de muestreo usando el propio campo antes de leerlo.
    ///
    /// Es lo que le saca al ruido de Perlin la cara de ruido de Perlin. Sin
    /// esto sus curvas de nivel son burbujas redondeadas, todas mas o menos
    /// del mismo tamano y repartidas parejo, y un terreno hecho con eso se
    /// lee enseguida como lomas de algodon. Deformando el dominio, esas
    /// mismas curvas se estiran, se doblan y se muerden entre si, que es lo
    /// que hace un valle: una forma que no se puede describir con un circulo.
    ///
    /// Los dos ejes se corren con lecturas de puntos bien separados del
    /// campo. Con la misma lectura para los dos, el corrimiento seria siempre
    /// en diagonal y el mapa entero saldria peinado para el mismo lado.
    ///
    /// `strength` va en las mismas unidades que x y z: cuanto se puede correr
    /// un punto, medido en celdas del ruido.
    pub fn warp(&self, x: f32, z: f32, strength: f32) -> (f32, f32) {
        if strength == 0.0 {
            return (x, z);
        }

        let along_x = self.fbm(x + WARP_OFFSET, z - WARP_OFFSET, WARP_OCTAVES);
        let along_z = self.fbm(x - WARP_OFFSET * 2.0, z + WARP_OFFSET * 3.0, WARP_OCTAVES);

        (x + along_x * strength, z + along_z * strength)
    }

    /// Ruido plegado sobre si mismo, de 0 en el fondo del valle a 1 en la
    /// cresta.
    ///
    /// El valor absoluto dobla el campo donde cruza cero, y ese doblez es un
    /// filo: no una loma con su cima redonda, sino una arista. Elevarlo al
    /// cuadrado despues deja la cresta fina y el valle ancho, que es la forma
    /// de una sierra de verdad, donde lo que abunda es la ladera y la cumbre
    /// es una linea.
    pub fn ridged(&self, x: f32, z: f32, octaves: u32) -> f32 {
        let mut total = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = 1.0;
        let mut reach = 0.0;

        for octave in 0..octaves.max(1) {
            let shift = octave as f32 * OCTAVE_SHIFT;
            let folded = 1.0 - self.sample(x * frequency + shift, z * frequency - shift).abs();
            total += folded * folded * amplitude;
            reach += amplitude;
            amplitude *= 0.5;
            frequency *= 2.0;
        }

        total / reach
    }

    /// Suma de octavas: cada una al doble de frecuencia y a la mitad de
    /// amplitud. La primera pone montanas y valles, las siguientes el detalle.
    pub fn fbm(&self, x: f32, z: f32, octaves: u32) -> f32 {
        let mut total = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = 1.0;
        let mut reach = 0.0;

        for octave in 0..octaves.max(1) {
            let shift = octave as f32 * OCTAVE_SHIFT;
            total += self.sample(x * frequency + shift, z * frequency - shift) * amplitude;
            reach += amplitude;
            amplitude *= 0.5;
            frequency *= 2.0;
        }

        total / reach
    }
}

/// Curva de Perlin: arranca y termina con pendiente cero, asi el borde de
/// cada celda no se ve como un quiebre.
#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_field() {
        let a = Noise::new(20240519);
        let b = Noise::new(20240519);

        for step in 0..50 {
            let (x, z) = (step as f32 * 0.37, step as f32 * -0.21);
            assert_eq!(a.sample(x, z), b.sample(x, z));
            assert_eq!(a.fbm(x, z, 4), b.fbm(x, z, 4));
        }
    }

    #[test]
    fn different_seeds_give_different_fields() {
        let a = Noise::new(1);
        let b = Noise::new(2);

        let differences = (0..50)
            .filter(|step| {
                let (x, z) = (*step as f32 * 0.37, *step as f32 * 0.11);
                a.sample(x, z) != b.sample(x, z)
            })
            .count();

        assert!(differences > 40, "solo cambiaron {differences} de 50 muestras");
    }

    /// La altura del terreno sale de escalar esto: un valor fuera de rango
    /// seria una columna fuera del mapa.
    #[test]
    fn the_field_stays_inside_its_range() {
        let noise = Noise::new(7);

        for step in 0..500 {
            let (x, z) = (step as f32 * 0.13 - 30.0, step as f32 * -0.29 + 11.0);
            let value = noise.sample(x, z);
            assert!((-1.0..=1.0).contains(&value), "sample dio {value}");

            let fractal = noise.fbm(x, z, 5);
            assert!((-1.0..=1.0).contains(&fractal), "fbm dio {fractal}");
        }
    }

    /// Lo que distingue al ruido de sortear cada punto. Sin esto el terreno
    /// sale en picos de un bloque.
    #[test]
    fn nearby_points_give_nearby_values() {
        let noise = Noise::new(99);

        for step in 0..200 {
            let (x, z) = (step as f32 * 0.41, step as f32 * 0.17);
            let jump = (noise.sample(x, z) - noise.sample(x + 0.02, z + 0.02)).abs();
            assert!(jump < 0.2, "salto de {jump} entre puntos pegados");
        }
    }

    /// Si las octavas se anularan entre si, el mapa entero saldria a la misma
    /// altura.
    #[test]
    fn the_fractal_sum_actually_varies() {
        let noise = Noise::new(3);
        let samples: Vec<f32> = (0..100)
            .map(|step| noise.fbm(step as f32 * 0.23, step as f32 * 0.19, 4))
            .collect();

        let low = samples.iter().cloned().fold(f32::INFINITY, f32::min);
        let high = samples.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(high - low > 0.5, "el campo se movio apenas {}", high - low);
    }

    /// La deformacion tiene que mover el punto, pero sin teletransportarlo:
    /// si el corrimiento pasara de `strength` el campo se rompe en pedazos
    /// que no se tocan.
    #[test]
    fn warping_shifts_the_point_within_its_reach() {
        let noise = Noise::new(5);
        let mut moved = 0;

        for step in 0..200 {
            let (x, z) = (step as f32 * 0.31, step as f32 * -0.17);
            let (wx, wz) = noise.warp(x, z, 2.0);

            assert!((wx - x).abs() <= 2.0, "corrio {} en x", wx - x);
            assert!((wz - z).abs() <= 2.0, "corrio {} en z", wz - z);
            if (wx - x).abs() > 0.05 || (wz - z).abs() > 0.05 {
                moved += 1;
            }
        }

        assert!(moved > 150, "solo se movieron {moved} de 200 puntos");
    }

    /// Y los dos ejes tienen que correrse por su cuenta: con el mismo
    /// corrimiento en los dos, el mapa entero sale peinado en diagonal.
    #[test]
    fn the_two_axes_warp_independently() {
        let noise = Noise::new(13);
        let same = (0..200)
            .filter(|step| {
                let (x, z) = (*step as f32 * 0.29, *step as f32 * 0.43);
                let (wx, wz) = noise.warp(x, z, 3.0);
                ((wx - x) - (wz - z)).abs() < 0.1
            })
            .count();

        assert!(same < 40, "{same} de 200 puntos se corrieron parejo");
    }

    #[test]
    fn warping_with_no_reach_leaves_the_point_alone() {
        let noise = Noise::new(8);
        assert_eq!(noise.warp(3.5, -1.25, 0.0), (3.5, -1.25));
    }

    /// El ruido plegado va de 0 a 1, y pasa mucho mas tiempo abajo que
    /// arriba: eso es lo que hace que la cresta sea una linea y no una
    /// meseta.
    #[test]
    fn the_folded_field_is_mostly_hillside() {
        let noise = Noise::new(21);
        let samples: Vec<f32> = (0..2000)
            .map(|step| {
                let (x, z) = (step as f32 % 50.0 * 0.4, (step / 50) as f32 * 0.4);
                noise.ridged(x, z, 3)
            })
            .collect();

        assert!(samples.iter().all(|value| (0.0..=1.0).contains(value)));

        let crests = samples.iter().filter(|value| **value > 0.8).count();
        assert!(crests > 0, "el campo nunca llega a una cresta");
        assert!(
            crests * 4 < samples.len(),
            "{crests} de {} muestras son cresta",
            samples.len()
        );
    }

    #[test]
    fn the_loose_draws_spread_over_the_unit_range() {
        let noise = Noise::new(11);
        let draws: Vec<f32> = (0..1000)
            .map(|i| noise.hash_unit(i % 32, i / 32, i % 7))
            .collect();

        assert!(draws.iter().all(|value| (0.0..1.0).contains(value)));

        let average = draws.iter().sum::<f32>() / draws.len() as f32;
        assert!((average - 0.5).abs() < 0.05, "promedio {average}");
    }

    #[test]
    fn each_call_draws_a_new_seed() {
        let seeds: Vec<u64> = (0..16).map(|_| random_seed()).collect();

        for (i, seed) in seeds.iter().enumerate() {
            assert!(!seeds[i + 1..].contains(seed), "la semilla {seed} salio dos veces");
        }
    }
}
