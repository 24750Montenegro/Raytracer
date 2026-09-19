// noise.rs
// Ruido de gradiente (Perlin) con semilla, y el fbm que suma octavas del
// mismo campo. De aca sale la altura de cada columna del terreno.
use std::collections::hash_map::RandomState;
use std::f32::consts::{SQRT_2, TAU};
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cuanto se corre el campo de una octava a la siguiente. El ruido de
/// gradiente vale 0 sobre los puntos enteros de su grilla: sin el corrimiento
/// todas las octavas se anulan en los mismos lugares y el terreno sale con
/// una cuadricula de hoyos regulares.
const OCTAVE_SHIFT: f32 = 31.7;

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
