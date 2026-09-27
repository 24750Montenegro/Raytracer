// texture.rs
// Textura de un bloque: un color por texel, un mapa normal en espacio
// tangente y un mapa de alturas.
//
// El mapa normal no toca la geometria. Guarda, para cada texel, hacia donde
// apuntaria la superficie si tuviera ese relieve, y el sombreado usa esa
// normal en vez de la de la cara. Por eso una cara plana de un cubo deja de
// verse como un cuadrado de un solo tono: cada texel recibe la luz con su
// propio angulo.
//
// Pero la normal sola solo cambia el brillo: la textura sigue pintada sobre
// una cara plana y, girando alrededor del bloque, el dibujo no se mueve. Lo
// que falta es el mapa de alturas y el trazado por paralaje: antes de leer
// el texel, el rayo se mete por debajo de la cara y camina el relieve hasta
// chocar con la primera pared. Eso corre el dibujo segun desde donde se
// mire, hace que un bulto tape lo que tiene detras y que un hueco se quede
// sin luz, que es lo que hace que un bloque se vea tallado y no estampado.
// Es la misma cuenta que usan los shaders de Minecraft.
//
// Ahora bien: el paralaje solo hunde hacia adentro. La silueta del cubo
// queda intacta, asi que de canto el bloque sigue siendo un cuadrado
// perfecto. Eso lo resuelve el pack Classic 3D poniendo geometria de verdad
// (ver src/model.rs), y por eso los bloques que ese pack decora van con el
// relieve trazado apagado: dos relieves contando lo mismo no se suman, se
// pelean. El paralaje queda para todo lo demas, que es la mayoria de los
// bloques naturales.
//
// El color de cada bloque sale de las texturas de Minecraft 1.20.1 y los
// mapas normales, del pack Vanilla Normals Renewed. Estan en assets/blocks;
// su procedencia y condiciones de uso, en assets/CREDITS.md. Lo que no viene
// de ahi, como el agua, se dibuja aca con ruido a partir de una receta.
use raylib::prelude::*;

use crate::color::Rgb;
use crate::noise::Noise;

/// Lado de una textura, en texels. 16 es la medida de Minecraft.
pub const TILE: usize = 16;

/// Donde viven los PNG del pack, relativo a la raiz del proyecto.
const ASSETS: &str = "assets/blocks";

/// Hasta cuanto puede inclinarse una normal del mapa, medido en pendiente:
/// 1.73 es tan(60 grados). Sin tope, exagerar el relieve deja las normales
/// casi de canto, y ahi el texel se prende y se apaga de golpe: en vez de
/// relieve aparecen manchones quemados al lado de otros negros.
const MAX_SLOPE: f32 = 1.73;

/// Pasos con los que el rayo recorre el relieve. De frente alcanza con
/// pocos; de canto el rayo cruza mucha textura por cada tramo que baja, y
/// con pocos pasos se saltea los bultos y el relieve sale en escalones.
const MARCH_STEPS_HEAD_ON: f32 = 6.0;
const MARCH_STEPS_GRAZING: f32 = 24.0;

/// Cuantos pasos da el rayo de la sombra propia. Va muy poca distancia (sube
/// del hueco a la superficie y nada mas), asi que no hacen falta muchos.
const SHADOW_STEPS: usize = 5;
/// Cuanta luz directa llega a apagar el relieve cuando tapa del todo. No es
/// 1: un hueco de un milimetro no deja un pozo negro.
const SHADOW_STRENGTH: f32 = 0.8;

/// Hasta donde se toma en serio un rayo rasante. Cuanto mas de canto llega,
/// mas se corre la textura, y sin tope ese corrimiento se va al infinito
/// justo en el borde de cada cara.
const MIN_SLANT: f32 = 0.25;

/// Cuanto relieve se le saca al brillo cuando el bloque no trae mapa normal
/// dibujado. Va bajo: el brillo de un texel no es su altura, es solo lo que
/// mas se le parece, y exagerarlo convierte cada mancha de color en un pozo.
const LUMA_BUMP: f32 = 0.45;

/// Pasadas de relajacion para levantar el mapa de alturas del mapa normal.
/// Son 256 texels y se hace una sola vez al abrir el programa; con estas la
/// altura ya no se mueve.
const RELAX_PASSES: usize = 300;

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
    /// Altura de cada texel, de 0 en el fondo del hueco mas profundo a 1 en
    /// la superficie de la cara.
    height: Vec<f32>,
    /// Cuanto se hunde ese relieve de verdad, como fraccion del lado de la
    /// cara. Un texel mide 1/16, asi que 0.06 es mas o menos un texel de
    /// hondo. En 0 la cara vuelve a ser plana y no se traza el relieve.
    depth: f32,
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
            height: vec![1.0; TILE * TILE],
            depth: 0.0,
        }
    }

    /// Carga un bloque del pack: el color de `<nombre>.png` y las normales
    /// dibujadas a mano de `<nombre>_n.png`.
    ///
    /// El tinte es para las texturas que el juego guarda en gris porque las
    /// colorea segun el bioma, como el pasto y las hojas. Para el resto va
    /// en blanco y no cambia nada.
    /// El pack de normales no cubre todos los bloques. Para los que no trae,
    /// el relieve sale del propio color: en una textura de Minecraft lo que
    /// esta hundido se dibuja mas oscuro, asi que el brillo de cada texel es
    /// una buena primera aproximacion a su altura. Peor que un mapa dibujado
    /// a mano, mucho mejor que una cara plana.
    pub fn load(name: &str, tint: Color) -> Result<Self, String> {
        let albedo = load_albedo(&format!("{ASSETS}/{name}.png"), tint)?;

        // se pregunta por el archivo antes de abrirlo: pedirselo a raylib y
        // que falle llena la consola de avisos por cada bloque sin mapa
        let normals = format!("{ASSETS}/{name}_n.png");
        if !std::path::Path::new(&normals).exists() {
            let height: Vec<f32> = albedo.iter().map(|texel| texel.luma()).collect();
            return Ok(Texture::from_relief(albedo, &height, LUMA_BUMP));
        }
        let normal = load_normals(&normals)?;

        // el pack dibuja normales y no alturas, asi que el mapa de alturas
        // que necesita el paralaje hay que levantarlo de ahi
        let height = height_from_normals(&normal);

        Ok(Texture {
            albedo,
            normal,
            height,
            depth: 0.0,
        })
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

        Texture {
            albedo,
            normal,
            height: normalized(height),
            depth: 0.0,
        }
    }

    /// Estira o aplana el relieve del mapa normal, sin tocar el color.
    ///
    /// Inclina cada normal mas o menos de lo que viene dibujada. Por encima
    /// de 1 exagera, y el bulto se nota mas en la luz; por debajo aplana, y
    /// con 0 la superficie queda de espejo, que es lo que hace falta donde lo
    /// que importa es el reflejo y no el relieve.
    ///
    /// Lo que se exagera es la pendiente, no las componentes de la normal.
    /// Multiplicar x e y y renormalizar parece lo mismo pero no lo es: esa
    /// cuenta no tiene techo y tumba la normal contra el horizonte, y una
    /// normal tumbada toma la luz casi de canto, asi que el texel salta de
    /// blanco a negro entre un vecino y el siguiente. En pendiente el tope
    /// de MAX_SLOPE tiene sentido y el relieve sube sin quemarse.
    pub fn with_relief(mut self, strength: f32) -> Self {
        for normal in &mut self.normal {
            let z = normal.z.max(1e-4);
            let mut slope_u = normal.x / z * strength;
            let mut slope_v = normal.y / z * strength;

            let slope = (slope_u * slope_u + slope_v * slope_v).sqrt();
            if slope > MAX_SLOPE {
                let cut = MAX_SLOPE / slope;
                slope_u *= cut;
                slope_v *= cut;
            }

            *normal = Vector3::new(slope_u, slope_v, 1.0).normalized();
        }

        self
    }

    /// Cuanto se hunde el relieve, como fraccion del lado de la cara.
    ///
    /// Es lo unico que separa una textura estampada de una tallada: con 0 el
    /// mapa de alturas no se usa y la cara se dibuja plana, como antes. Un
    /// texel mide 1/16 de la cara, asi que alrededor de 0.06 da el relieve
    /// de un texel de hondo, que es el de los packs 3D.
    pub fn with_depth(mut self, depth: f32) -> Self {
        self.depth = depth.max(0.0);
        self
    }

    /// Color y normal del texel en el que cae (u, v).
    ///
    /// Sin interpolar, a proposito: el pixelado es justamente el aspecto de
    /// Minecraft, y suavizarlo convertiria cada bloque en un degrade.
    pub fn sample(&self, u: f32, v: f32) -> (Rgb, Vector3) {
        let texel = texel_at(u, v);

        (self.albedo[texel], self.normal[texel])
    }

    /// Altura del texel en el que cae (u, v), de 0 a 1.
    pub fn height_at(&self, u: f32, v: f32) -> f32 {
        self.height[texel_at(u, v)]
    }

    /// Donde entra de verdad el rayo en el relieve.
    ///
    /// El mapa de alturas cuelga por debajo de la cara como un bajorrelieve.
    /// El rayo no pinta el texel donde pego la cara plana: se mete y avanza
    /// en diagonal hasta que el suelo le pasa por encima, que es donde choca
    /// con la primera pared. Ese corrimiento depende de desde donde se mire,
    /// asi que el dibujo se mueve al girar la camara y los bultos tapan lo
    /// que tienen detras, igual que si fueran geometria.
    ///
    /// `view` va del punto hacia la camara y viene en espacio tangente: x
    /// sobre la tangente, y sobre la bitangente y z saliendo de la cara.
    ///
    /// Devuelve las coordenadas corridas y a que altura quedo el impacto,
    /// que es de donde arranca despues la sombra propia del relieve.
    pub fn parallax(&self, u: f32, v: f32, view: Vector3) -> (f32, f32, f32) {
        // sin relieve, o mirando la cara desde atras (que es lo que pasa
        // dentro de un cuerpo transparente), no hay nada que recorrer
        if self.depth <= 0.0 || view.z <= 0.0 {
            return (u, v, 1.0);
        }

        let slant = view.z.max(MIN_SLANT);
        let steps = MARCH_STEPS_GRAZING
            + (MARCH_STEPS_HEAD_ON - MARCH_STEPS_GRAZING) * view.z.clamp(0.0, 1.0);
        let steps = steps.round().max(1.0);

        // cuanto se corre la textura y cuanto baja el rayo en cada tramo
        let step_u = -view.x / slant * self.depth / steps;
        let step_v = -view.y / slant * self.depth / steps;
        let step_height = 1.0 / steps;

        let (mut current_u, mut current_v) = (u, v);
        // el rayo entra por la superficie, que es la altura 1
        let mut ray = 1.0;
        let mut ground = self.height_at(current_u, current_v);
        // que tan por encima del suelo venia el rayo en el paso anterior
        let mut above = ray - ground;

        for _ in 0..steps as usize {
            if ground >= ray {
                break;
            }

            above = ray - ground;
            current_u += step_u;
            current_v += step_v;
            ray -= step_height;
            ground = self.height_at(current_u, current_v);
        }

        // el choque quedo entre el paso anterior, con el rayo todavia por
        // encima del suelo, y este, con el rayo por debajo: se reparte entre
        // los dos para que la pared no salga en escalones
        let below = ground - ray;
        let share = if below > 0.0 {
            below / (below + above.max(0.0))
        } else {
            0.0
        };

        (
            current_u - step_u * share,
            current_v - step_v * share,
            ray + step_height * share,
        )
    }

    /// Cuanta luz directa le llega a un punto del relieve desde esa
    /// direccion, de 0 (tapado) a 1 (a la vista).
    ///
    /// Es la otra mitad del bajorrelieve: un bulto no solo se ve, tambien
    /// hace sombra. Desde donde quedo el impacto se camina hacia la luz,
    /// subiendo hasta la superficie, y si algun texel del camino queda por
    /// encima del rayo es que el punto esta en el fondo de un hueco. Sin
    /// esto el relieve se nota al girar, pero la textura sigue pareciendo
    /// lisa porque las juntas entre piedras nunca se oscurecen.
    ///
    /// `to_light` va del punto hacia la luz, en espacio tangente.
    pub fn lit_fraction(&self, u: f32, v: f32, height: f32, to_light: Vector3) -> f32 {
        let climb = 1.0 - height;
        if self.depth <= 0.0 || to_light.z <= 0.0 || climb <= 0.0 {
            return 1.0;
        }

        let slant = to_light.z.max(MIN_SLANT);
        let steps = SHADOW_STEPS as f32;
        // el rayo sube del hueco a la superficie: esa es toda la distancia
        // que puede recorrer antes de escaparse del relieve
        let step_u = to_light.x / slant * self.depth * climb / steps;
        let step_v = to_light.y / slant * self.depth * climb / steps;
        let step_height = climb / steps;

        let (mut current_u, mut current_v) = (u, v);
        let mut ray = height;
        let mut buried = 0.0f32;

        for _ in 0..SHADOW_STEPS {
            current_u += step_u;
            current_v += step_v;
            ray += step_height;

            // cuanto le saca el suelo al rayo: un borde apenas asomado tapa
            // menos que una pared entera
            buried = buried.max(self.height_at(current_u, current_v) - ray);
        }

        1.0 - (buried / climb).clamp(0.0, 1.0) * SHADOW_STRENGTH
    }
}

/// El texel en el que cae (u, v).
///
/// Sin interpolar, a proposito: el pixelado es justamente el aspecto de
/// Minecraft, y suavizarlo convertiria cada bloque en un degrade. Las
/// coordenadas de afuera del rango dan la vuelta en vez de reventar.
fn texel_at(u: f32, v: f32) -> usize {
    let x = ((u.rem_euclid(1.0) * TILE as f32) as usize).min(TILE - 1);
    let y = ((v.rem_euclid(1.0) * TILE as f32) as usize).min(TILE - 1);

    y * TILE + x
}

impl Surface {
    /// Aplica el mapa normal: lleva la normal del texel, que viene en espacio
    /// tangente, al espacio del mundo.
    pub fn bumped(&self, facing: Vector3, relief: Vector3) -> Vector3 {
        (self.tangent * relief.x + self.bitangent * relief.y + facing * relief.z).normalized()
    }

    /// El camino de vuelta: lleva una direccion del mundo al espacio
    /// tangente de la textura, que es donde se recorre el relieve.
    pub fn to_tangent(&self, world: Vector3, facing: Vector3) -> Vector3 {
        Vector3::new(
            world.dot(self.tangent),
            world.dot(self.bitangent),
            world.dot(facing),
        )
    }
}

/// Levanta el mapa de alturas a partir del mapa normal.
///
/// De cada normal sale la pendiente entre su texel y el de al lado; lo que
/// falta es la altura que encaje con todas esas pendientes a la vez. No sale
/// de sumarlas en una direccion: el error de cada texel se arrastra al
/// siguiente y la textura termina con un degrade de un borde al otro. Sale
/// de promediar, que es lo mismo pero sin privilegiar ninguna direccion:
/// cada texel se acomoda a lo que dicen sus cuatro vecinos, y repitiendo esa
/// pasada la superficie se asienta sola.
fn height_from_normals(normal: &[Vector3]) -> Vec<f32> {
    // z nunca es cero: las normales del mapa salen de la cara
    let slope: Vec<(f32, f32)> = normal
        .iter()
        .map(|normal| {
            let z = normal.z.max(1e-3);
            (-normal.x / z, -normal.y / z)
        })
        .collect();

    let mut height = vec![0.0; TILE * TILE];
    let mut next = height.clone();

    for _ in 0..RELAX_PASSES {
        for y in 0..TILE {
            for x in 0..TILE {
                let here = y * TILE + x;
                // cada vecino opina que altura tendria que tener este texel:
                // la suya, menos lo que sube el escalon que los separa
                let mut total = 0.0;
                let mut voters = 0.0;
                let mut vote = |there: usize, rise: f32| {
                    total += height[there] + rise;
                    voters += 1.0;
                };

                if x > 0 {
                    vote(here - 1, (slope[here - 1].0 + slope[here].0) * 0.5);
                }
                if x + 1 < TILE {
                    vote(here + 1, -(slope[here + 1].0 + slope[here].0) * 0.5);
                }
                if y > 0 {
                    vote(here - TILE, (slope[here - TILE].1 + slope[here].1) * 0.5);
                }
                if y + 1 < TILE {
                    vote(here + TILE, -(slope[here + TILE].1 + slope[here].1) * 0.5);
                }

                next[here] = total / voters;
            }
        }

        std::mem::swap(&mut height, &mut next);
    }

    normalized(&height)
}

/// Lleva el relieve al rango 0..1: 1 en la superficie de la cara y 0 en el
/// fondo del hueco mas profundo. La escala con la que venia no importa,
/// porque cuanto se hunde de verdad lo decide `with_depth`.
fn normalized(height: &[f32]) -> Vec<f32> {
    let low = height.iter().copied().fold(f32::INFINITY, f32::min);
    let high = height.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let range = high - low;

    // todos a la misma altura: la cara es plana y no hay nada que hundir
    if range < 1e-6 {
        return vec![1.0; height.len()];
    }

    height.iter().map(|texel| (texel - low) / range).collect()
}

/// Lee un PNG de bloque y devuelve sus texels en RGBA.
///
/// Los bloques animados (el magma, el farol de mar, la prismarina) no vienen
/// en 16x16 sino en una tira vertical con un cuadro debajo del otro. Aca la
/// escena no se mueve, asi que se toma el primero y listo.
fn read_tile(path: &str) -> Result<Vec<Color>, String> {
    let image = Image::load_image(path).map_err(|error| error.to_string())?;
    let (width, height) = (image.width as usize, image.height as usize);

    if width != TILE || height < TILE || height % TILE != 0 {
        return Err(format!(
            "{path} mide {width}x{height} y se esperaba {TILE} de ancho por un alto multiplo de {TILE}"
        ));
    }

    Ok(image.get_image_data()[..TILE * TILE].to_vec())
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
