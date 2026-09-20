// texture.rs
// Textura de un bloque: un color por texel y un mapa normal en espacio
// tangente.
//
// El mapa normal no toca la geometria. Guarda, para cada texel, hacia donde
// apuntaria la superficie si tuviera ese relieve, y el sombreado usa esa
// normal en vez de la de la cara. Por eso una cara plana de un cubo deja de
// verse como un cuadrado de un solo tono: cada texel recibe la luz con su
// propio angulo.
//
// Las texturas salen del pack Vanilla Normals Renewed, que trae el color de
// cada bloque y, al lado, un mapa normal dibujado a mano. Estan en
// assets/blocks, y su procedencia y condiciones de uso, en el CREDITS.md de
// esa carpeta. Lo que el pack no trae, como el agua, se genera aca con ruido
// a partir de una receta.
use raylib::prelude::*;

use crate::color::Rgb;
use crate::noise::Noise;

/// Lado de una textura, en texels. 16 es la medida de Minecraft.
pub const TILE: usize = 16;

/// Donde viven los PNG del pack, relativo a la raiz del proyecto.
const ASSETS: &str = "assets/blocks";

/// Donde cae un impacto sobre la superficie de un cuerpo: la normal de la
/// cara, los dos ejes de la textura y el punto en coordenadas de textura.
pub struct Surface {
    pub normal: Vector3,
    /// Direccion en la que crece u, y en la que crece v.
    pub tangent: Vector3,
    pub bitangent: Vector3,
    pub u: f32,
    pub v: f32,
}

pub struct Texture {
    albedo: Vec<Rgb>,
    /// Normales en espacio tangente: x sobre la tangente, y sobre la
    /// bitangente y z saliendo de la cara.
    normal: Vec<Vector3>,
}

/// Receta de una textura. Todas salen de la misma cuenta y lo unico que
/// cambia son estos numeros, asi que agregar un bloque es agregar una fila.
pub struct Recipe {
    /// Cuantos texels mide un rasgo en cada eje. Estirar uno de los dos deja
    /// vetas: mucho en y da rayas verticales, mucho en x da bandas.
    pub grain_x: f32,
    pub grain_y: f32,
    /// Cuanto se separan del color base los texels.
    pub contrast: f32,
    /// Cuanto relieve levanta el mapa normal.
    pub bump: f32,
    /// Mezcla entre manchones suaves (0) y grano texel a texel (1).
    pub grit: f32,
    /// Pepitas de otro color, como las vetas de mineral en la piedra.
    pub fleck: Option<Fleck>,
}

pub struct Fleck {
    pub color: Color,
    /// Parte de la textura que ocupan, de 0 a 1.
    pub coverage: f32,
    /// Cuanto sobresalen del resto.
    pub relief: f32,
}

impl Recipe {
    /// Receta base: grano parejo y sin pepitas.
    pub const fn new(grain: f32, contrast: f32, bump: f32, grit: f32) -> Self {
        Recipe {
            grain_x: grain,
            grain_y: grain,
            contrast,
            bump,
            grit,
            fleck: None,
        }
    }

    /// Estira el grano en un eje para que salgan vetas en vez de manchas.
    pub const fn stretched(mut self, grain_x: f32, grain_y: f32) -> Self {
        self.grain_x = grain_x;
        self.grain_y = grain_y;
        self
    }

    pub const fn flecked(mut self, color: Color, coverage: f32, relief: f32) -> Self {
        self.fleck = Some(Fleck {
            color,
            coverage,
            relief,
        });
        self
    }
}

impl Texture {
    /// Textura lisa y blanca: un material que la use queda igual que si no
    /// hubiera texturas.
    pub fn plain() -> Self {
        Texture {
            albedo: vec![Rgb::new(1.0, 1.0, 1.0); TILE * TILE],
            normal: vec![Vector3::new(0.0, 0.0, 1.0); TILE * TILE],
        }
    }

    /// Carga un bloque del pack: el color de `<nombre>.png` y las normales
    /// dibujadas a mano de `<nombre>_n.png`.
    ///
    /// El tinte es para las texturas que el juego guarda en gris porque las
    /// colorea segun el bioma, como el pasto y las hojas. Para el resto va
    /// en blanco y no cambia nada.
    pub fn load(name: &str, tint: Color) -> Result<Self, String> {
        let albedo = load_albedo(&format!("{ASSETS}/{name}.png"), tint)?;
        let normal = load_normals(&format!("{ASSETS}/{name}_n.png"))?;

        Ok(Texture { albedo, normal })
    }

    /// Dibuja la textura de un bloque y le saca el mapa normal.
    pub fn build(base: Color, recipe: &Recipe, noise: &Noise) -> Self {
        let base = Rgb::from_color(base);
        let mut albedo = Vec::with_capacity(TILE * TILE);
        let mut height = Vec::with_capacity(TILE * TILE);

        for y in 0..TILE {
            for x in 0..TILE {
                // dos escalas de detalle: manchones continuos y grano suelto
                let blotch = noise.sample(x as f32 / recipe.grain_x, y as f32 / recipe.grain_y);
                let grit = noise.hash_unit(x as i32, 0, y as i32);
                let detail = (blotch * 0.5 + 0.5) * (1.0 - recipe.grit) + grit * recipe.grit;

                let mut color = base * (1.0 + (detail - 0.5) * 2.0 * recipe.contrast);
                let mut relief = detail;

                if let Some(fleck) = &recipe.fleck {
                    // las pepitas van en manchones propios, separados del
                    // grano de la piedra que tienen alrededor
                    let vein = noise.sample(x as f32 / 2.6 + 40.0, y as f32 / 2.6 - 17.0);
                    if vein > 1.0 - fleck.coverage * 2.0 {
                        color = Rgb::from_color(fleck.color) * (0.9 + detail * 0.2);
                        relief += fleck.relief;
                    }
                }

                albedo.push(color);
                height.push(relief);
            }
        }

        Texture::from_relief(albedo, &height, recipe.bump)
    }

    /// Arma el mapa normal a partir de la altura de cada texel: la normal se
    /// inclina en contra de la pendiente, que es lo que hace que una subida
    /// tome la luz y la bajada quede en sombra.
    pub fn from_relief(albedo: Vec<Rgb>, height: &[f32], bump: f32) -> Self {
        let at = |x: usize, y: usize| height[y * TILE + x];
        // los bordes no dan la vuelta: cada cara del cubo muestra la textura
        // entera, asi que el texel de al lado del borde es el borde mismo
        let clamped = |v: usize, delta: i32| {
            (v as i32 + delta).clamp(0, TILE as i32 - 1) as usize
        };

        let mut normal = Vec::with_capacity(TILE * TILE);
        for y in 0..TILE {
            for x in 0..TILE {
                let slope_u = at(clamped(x, 1), y) - at(clamped(x, -1), y);
                let slope_v = at(x, clamped(y, 1)) - at(x, clamped(y, -1));

                normal.push(
                    Vector3::new(-slope_u * bump, -slope_v * bump, 1.0).normalized(),
                );
            }
        }

        Texture { albedo, normal }
    }

    /// Estira o aplana el relieve del mapa normal, sin tocar el color.
    ///
    /// Inclina cada normal mas o menos de lo que viene dibujada. Por encima
    /// de 1 exagera, y el bulto se nota mas en la luz; por debajo aplana, y
    /// con 0 la superficie queda de espejo, que es lo que hace falta donde lo
    /// que importa es el reflejo y no el relieve.
    pub fn with_relief(mut self, strength: f32) -> Self {
        for normal in &mut self.normal {
            *normal =
                Vector3::new(normal.x * strength, normal.y * strength, normal.z).normalized();
        }

        self
    }

    /// Color y normal del texel en el que cae (u, v).
    ///
    /// Sin interpolar, a proposito: el pixelado es justamente el aspecto de
    /// Minecraft, y suavizarlo convertiria cada bloque en un degrade.
    pub fn sample(&self, u: f32, v: f32) -> (Rgb, Vector3) {
        let x = ((u.rem_euclid(1.0) * TILE as f32) as usize).min(TILE - 1);
        let y = ((v.rem_euclid(1.0) * TILE as f32) as usize).min(TILE - 1);
        let texel = y * TILE + x;

        (self.albedo[texel], self.normal[texel])
    }
}

impl Surface {
    /// Aplica el mapa normal: lleva la normal del texel, que viene en espacio
    /// tangente, al espacio del mundo.
    pub fn bumped(&self, facing: Vector3, relief: Vector3) -> Vector3 {
        (self.tangent * relief.x + self.bitangent * relief.y + facing * relief.z).normalized()
    }
}

/// Lee un PNG de 16x16 y devuelve sus texels en RGBA.
fn read_tile(path: &str) -> Result<Vec<Color>, String> {
    let image = Image::load_image(path).map_err(|error| error.to_string())?;

    if image.width as usize != TILE || image.height as usize != TILE {
        return Err(format!(
            "{path} mide {}x{} y se esperaba {TILE}x{TILE}",
            image.width, image.height
        ));
    }

    Ok(image.get_image_data().to_vec())
}

fn load_albedo(path: &str, tint: Color) -> Result<Vec<Rgb>, String> {
    let texels = read_tile(path)?;
    let tint = Rgb::from_color(tint);

    // Las hojas y el cactus vienen recortados con alfa. Aca los cubos son
    // opacos, asi que los agujeros se rellenan con el color medio de lo que
    // si esta pintado: si se dejaran como vienen, saldrian manchas negras.
    let opaque: Vec<Rgb> = texels
        .iter()
        .filter(|texel| texel.a >= 128)
        .map(|texel| Rgb::from_color(*texel))
        .collect();
    let filler = average(&opaque);

    Ok(texels
        .iter()
        .map(|texel| {
            let color = if texel.a >= 128 {
                Rgb::from_color(*texel)
            } else {
                filler
            };
            color * tint
        })
        .collect())
}

/// Decodifica el mapa normal del pack.
///
/// De los tres canales solo se leen R y G, que son las dos componentes
/// horizontales, y Z se reconstruye sabiendo que la normal es unitaria. Es lo
/// unico que se puede hacer sin saber cual de las dos convenciones trae el
/// archivo: en oldPBR el azul es la tercera componente, pero en labPBR ese
/// mismo canal guarda oclusion ambiental, y leerlo como si fuera Z daria
/// normales torcidas en la mitad de los bloques.
fn load_normals(path: &str) -> Result<Vec<Vector3>, String> {
    let texels = read_tile(path)?;

    Ok(texels
        .iter()
        .map(|texel| {
            let x = texel.r as f32 / 127.5 - 1.0;
            // el verde viene en la convencion de OpenGL, con +Y hacia arriba
            // en la imagen; aca la bitangente baja con las filas, asi que se
            // invierte. Sin esto los bultos se verian hundidos.
            let y = 1.0 - texel.g as f32 / 127.5;
            let z = (1.0 - x * x - y * y).max(0.0).sqrt();

            Vector3::new(x, y, z).normalized()
        })
        .collect())
}

fn average(colors: &[Rgb]) -> Rgb {
    if colors.is_empty() {
        return Rgb::new(1.0, 1.0, 1.0);
    }

    let total = colors
        .iter()
        .fold(Rgb::new(0.0, 0.0, 0.0), |sum, color| sum + *color);

    total * (1.0 / colors.len() as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stone() -> Recipe {
        Recipe::new(1.5, 0.2, 0.6, 0.7)
    }

    /// La lisa tiene que dejar el material como estaba: blanco y sin relieve.
    #[test]
    fn the_plain_texture_changes_nothing() {
        let (color, normal) = Texture::plain().sample(0.3, 0.7);

        assert_eq!(color, Rgb::new(1.0, 1.0, 1.0));
        assert_eq!(normal, Vector3::new(0.0, 0.0, 1.0));
    }

    /// Recorrer u y v tiene que pasar por los 256 texels, sin repetir ni
    /// salirse: si no, parte de la textura no se veria nunca.
    #[test]
    fn the_whole_tile_gets_sampled() {
        let mut seen = vec![false; TILE * TILE];

        for row in 0..TILE {
            for column in 0..TILE {
                let u = (column as f32 + 0.5) / TILE as f32;
                let v = (row as f32 + 0.5) / TILE as f32;

                let x = ((u * TILE as f32) as usize).min(TILE - 1);
                let y = ((v * TILE as f32) as usize).min(TILE - 1);
                seen[y * TILE + x] = true;
            }
        }

        assert!(seen.iter().all(|hit| *hit), "quedaron texels sin alcanzar");
    }

    /// Las coordenadas de afuera del rango dan la vuelta en vez de reventar.
    #[test]
    fn coordinates_outside_the_tile_wrap_around() {
        let texture = Texture::build(Color::new(120, 120, 120, 255), &stone(), &Noise::new(1));

        assert_eq!(texture.sample(0.25, 0.25), texture.sample(1.25, -0.75));
        assert_eq!(texture.sample(0.0, 0.0), texture.sample(2.0, 3.0));
    }

    /// Toda normal del mapa sale de la cara, nunca hacia adentro: una normal
    /// con z negativa dejaria el texel iluminado desde atras.
    #[test]
    fn every_mapped_normal_points_out_of_the_face() {
        let texture = Texture::build(Color::new(120, 120, 120, 255), &stone(), &Noise::new(7));

        for normal in &texture.normal {
            assert!(normal.z > 0.0, "{normal:?} apunta para adentro");
            assert!((normal.length() - 1.0).abs() < 1e-4, "sin normalizar");
        }
    }

    /// Una rampa que sube hacia +u tiene que inclinar la normal hacia -u.
    #[test]
    fn a_slope_tilts_the_normal_against_it() {
        let mut height = vec![0.0; TILE * TILE];
        for y in 0..TILE {
            for x in 0..TILE {
                height[y * TILE + x] = x as f32 / TILE as f32;
            }
        }

        let texture = Texture::from_relief(vec![Rgb::new(1.0, 1.0, 1.0); TILE * TILE], &height, 4.0);
        let (_, normal) = texture.sample(0.5, 0.5);

        assert!(normal.x < 0.0, "la normal tendria que caer en contra: {normal:?}");
        assert!(normal.y.abs() < 1e-5, "en v no hay pendiente");
    }

    /// Sin relieve no hay mapa normal que valga: todas quedan rectas.
    #[test]
    fn a_flat_relief_leaves_the_normals_straight() {
        let texture = Texture::from_relief(
            vec![Rgb::new(1.0, 1.0, 1.0); TILE * TILE],
            &vec![0.4; TILE * TILE],
            5.0,
        );

        for normal in &texture.normal {
            assert!((normal.z - 1.0).abs() < 1e-5);
        }
    }

    /// El relieve tiene que mover las normales de verdad, si no el mapa no
    /// agrega nada. Y las pepitas, aparecer.
    #[test]
    fn the_recipe_produces_relief_and_flecks() {
        let gold = Color::new(214, 176, 72, 255);
        let recipe = Recipe::new(1.5, 0.2, 0.8, 0.7).flecked(gold, 0.3, 0.6);
        let texture = Texture::build(Color::new(120, 120, 120, 255), &recipe, &Noise::new(3));

        let tilted = texture
            .normal
            .iter()
            .filter(|normal| normal.z < 0.98)
            .count();
        assert!(tilted > TILE * TILE / 4, "solo {tilted} texels con relieve");

        let golden = texture
            .albedo
            .iter()
            .filter(|color| color.r > color.b * 1.5)
            .count();
        assert!(golden > 0, "no salio una sola pepita");
    }

    /// Por encima de 1 el relieve se nota mas y por debajo menos, pero la
    /// normal nunca se da vuelta ni deja de ser unitaria.
    #[test]
    fn the_relief_can_be_exaggerated_or_flattened() {
        let tilt = |texture: &Texture| {
            let total: f32 = texture.normal.iter().map(|normal| 1.0 - normal.z).sum();
            total / (TILE * TILE) as f32
        };
        let rock = || Texture::build(Color::new(120, 120, 120, 255), &stone(), &Noise::new(5));

        let plain = tilt(&rock());
        let strong = rock().with_relief(3.0);
        let flat = rock().with_relief(0.0);

        assert!(plain > 0.0, "la textura de prueba no tenia relieve");
        assert!(tilt(&strong) > plain * 1.5, "exagerar no cambio nada");
        assert!(tilt(&flat) < 1e-6, "aplanar del todo tendria que dejarla lisa");

        for normal in &strong.normal {
            assert!(normal.z > 0.0, "{normal:?} se dio vuelta");
            assert!((normal.length() - 1.0).abs() < 1e-4, "sin normalizar");
        }
    }

    /// Dos recetas distintas tienen que dar texturas distintas.
    #[test]
    fn different_recipes_look_different() {
        let noise = Noise::new(11);
        let base = Color::new(150, 150, 150, 255);

        let speckled = Texture::build(base, &Recipe::new(1.4, 0.2, 0.6, 0.9), &noise);
        let streaked = Texture::build(
            base,
            &Recipe::new(1.4, 0.2, 0.6, 0.0).stretched(1.0, 8.0),
            &noise,
        );

        assert_ne!(speckled.albedo, streaked.albedo);
    }
}
