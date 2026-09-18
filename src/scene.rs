// scene.rs
// Escena fija que renderiza el raytracer: cuerpos, luces y luz ambiente.
use raylib::prelude::*;

use crate::bvh::Bvh;
use crate::color::Rgb;
use crate::light::Light;
use crate::material::Material;
use crate::object::Object;

/// Color del cielo: lo que devuelve un rayo que no pega con nada.
pub const BACKGROUND_COLOR: Color = Color { r: 14, g: 27, b: 61, a: 255 };

/// Piso de iluminacion. Sin ambiente, todo lo que no ve ninguna luz queda
/// negro puro y el diorama se parte en dos mitades duras.
const AMBIENT: Rgb = Rgb::new(0.16, 0.17, 0.22);

/// Todo lo que el trazado necesita: geometria, aceleracion y luces.
pub struct Scene {
    pub objects: Vec<Object>,
    pub bvh: Bvh,
    pub lights: Vec<Light>,
    pub ambient: Rgb,
    /// Lo que ve un rayo que se escapa de la escena.
    pub background: Rgb,
    /// Centro y radio de la escena, para plantar la camara y las luces.
    pub center: Vector3,
    pub radius: f32,
}

impl Scene {
    pub fn preset() -> Self {
        let mut objects = preset_scene();
        // la escena no cambia nunca, asi que el BVH se arma una sola vez
        let bvh = Bvh::build(&mut objects);
        let (center, radius) = bounding_sphere(&objects);

        Scene {
            objects,
            bvh,
            lights: default_lights(center, radius),
            ambient: AMBIENT,
            background: Rgb::from_color(BACKGROUND_COLOR),
            center,
            radius,
        }
    }

    /// Escena vacia con luces a medida, para los tests de shading.
    #[cfg(test)]
    pub fn for_test(lights: Vec<Light>, ambient: Rgb) -> Self {
        let mut objects = Vec::new();
        let bvh = Bvh::build(&mut objects);
        Scene {
            objects,
            bvh,
            lights,
            ambient,
            background: Rgb::from_color(BACKGROUND_COLOR),
            center: Vector3::zero(),
            radius: 1.0,
        }
    }
}

/// Dos luces, como en fotografia: una principal que define el volumen y una
/// de relleno mas debil del lado opuesto para que las sombras no queden
/// planas. Se ubican en funcion del tamano de la escena, no hardcodeadas.
/// Ojo: en esta escena -Y es arriba, por eso las alturas van en negativo.
fn default_lights(center: Vector3, radius: f32) -> Vec<Light> {
    vec![
        // principal: arriba, a la izquierda y adelante
        Light::new(
            center + Vector3::new(-1.2 * radius, -1.6 * radius, 1.8 * radius),
            Rgb::new(1.0, 0.97, 0.90),
            0.85,
        ),
        // relleno: del otro lado, mas debil y mas fria
        Light::new(
            center + Vector3::new(1.8 * radius, -0.2 * radius, 0.9 * radius),
            Rgb::new(0.65, 0.78, 1.0),
            0.35,
        ),
    ]
}

/// Elige el material segun el color de la paleta con el que se dibujo la
/// esfera. Los tonos oscuros son los trazos de contorno y la vegetacion del
/// dibujo: mates, sin reflejo, se leen como tinta. El resto son cuerpos
/// pintados, con el brillo suave del plastico.
fn material_for(diffuse: Color) -> Material {
    match (diffuse.r, diffuse.g, diffuse.b) {
        // verde casi negro y los dos azules oscuros
        (21, 28, 26) | (14, 27, 61) | (1, 40, 83) => Material::matte(diffuse),
        // el casi blanco de la paleta pasa a metal pulido: es el que devuelve
        // la imagen del resto del diorama
        (157, 172, 180) => Material::mirror(diffuse),
        // el amarillo intenso pasa a vidrio de color: deja ver lo que tiene atras
        (183, 159, 42) => Material::glass(diffuse),
        _ => Material::plastic(diffuse),
    }
}

/// Altura del piso. En esta escena -Y es arriba, asi que el piso es la Y mas
/// grande del dibujo: la esfera mas baja del diorama llega a 5.13, y los
/// cubos arrancan justo abajo para no meterse dentro de ella.
const GROUND_Y: f32 = 5.15;

/// Lado de los cubos del piso.
const FLOOR_SIZE: f32 = 1.2;
/// Distancia entre centros. Un poco mas que el lado a proposito: la junta
/// abierta deja ver las caras laterales y el piso se lee como bloques
/// sueltos y no como una losa lisa.
const FLOOR_STEP: f32 = 1.35;
const FLOOR_COLUMNS: usize = 7;
const FLOOR_ROWS: usize = 5;
/// Fila de cubos mas cercana a la camara; las demas van hacia el fondo.
const FLOOR_FRONT_Z: f32 = -4.65;

/// Lado de los cubos sueltos que se apoyan sobre el piso.
const ACCENT_SIZE: f32 = 1.1;

/// Piso de bloques debajo del diorama. Es lo que le da suelo a las sombras:
/// hasta ahora los rayos que pasaban de largo por abajo se iban al fondo y
/// las esferas no proyectaban nada sobre nada.
fn floor_cubes() -> Vec<Object> {
    let mut cubes = Vec::with_capacity(FLOOR_COLUMNS * FLOOR_ROWS);
    // el centro esta medio lado por debajo del piso, o sea con Y mas grande
    let y = GROUND_Y + FLOOR_SIZE * 0.5;
    let first_x = -((FLOOR_COLUMNS - 1) as f32) * FLOOR_STEP * 0.5;

    for column in 0..FLOOR_COLUMNS {
        for row in 0..FLOOR_ROWS {
            let center = Vector3::new(
                first_x + column as f32 * FLOOR_STEP,
                y,
                FLOOR_FRONT_Z - row as f32 * FLOOR_STEP,
            );

            // damero con los dos azules de la paleta: uno pintado y el otro
            // de tinta, asi se distingue cubo de cubo sin lineas de por medio
            let color = if (column + row) % 2 == 0 {
                Color::new(81, 107, 168, 255)
            } else {
                Color::new(1, 40, 83, 255)
            };

            cubes.push(Object::cube(center, FLOOR_SIZE, material_for(color)));
        }
    }

    cubes
}

/// Tres cubos apoyados sobre el piso, uno por cada material interesante de
/// la paleta: el espejo, el vidrio y un pintado comun. Las caras planas
/// muestran el reflejo y la refraccion mucho mejor que una esfera, donde la
/// curvatura los deforma.
fn accent_cubes() -> Vec<Object> {
    // apoyados encima del piso: medio lado por arriba, o sea Y mas chica
    let y = GROUND_Y - ACCENT_SIZE * 0.5;

    vec![
        // casi blanco: espejo
        Object::cube(
            Vector3::new(-3.4, y, -5.3),
            ACCENT_SIZE,
            material_for(Color::new(157, 172, 180, 255)),
        ),
        // amarillo intenso: vidrio de color
        Object::cube(
            Vector3::new(3.4, y, -5.3),
            ACCENT_SIZE,
            material_for(Color::new(183, 159, 42, 255)),
        ),
        // verde claro: plastico, para tener la forma sin reflejos que
        // distraigan
        Object::cube(
            Vector3::new(0.2, y, -9.7),
            ACCENT_SIZE,
            material_for(Color::new(182, 188, 121, 255)),
        ),
    ]
}

/// Los cubos de la escena: el piso mas los tres sueltos.
fn preset_cubes() -> Vec<Object> {
    let mut cubes = floor_cubes();
    cubes.extend(accent_cubes());
    cubes
}

/// Cuerpos del diorama: las esferas del dibujo y los cubos que lo apoyan.
fn preset_scene() -> Vec<Object> {
    let mut objects = vec![
        
    Object::sphere(Vector3::new(1.8100519, -1.9459274, -5.0), 0.6675576, material_for(Color::new(182, 188, 121, 255))),
    Object::sphere(Vector3::new(1.7711501, -1.9104263, -4.9027395), 0.60205203, material_for(Color::new(183, 159, 42, 255))),
    Object::sphere(Vector3::new(1.7029026, -1.9012673, -4.515962), 0.4403378, material_for(Color::new(182, 188, 121, 255))),
    Object::sphere(Vector3::new(-2.2157428, 2.218038, -4.515962), 0.43833515, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.1275501, 1.642594, -4.515962), 0.28494725, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.4873564, 0.64139396, -4.515962), 0.12623174, material_for(Color::new(157, 172, 180, 255))),
    Object::sphere(Vector3::new(-2.2892025, 0.7874023, -4.515962), 0.09001779, material_for(Color::new(182, 188, 121, 255))),
    Object::sphere(Vector3::new(-2.2892025, -2.143194, -4.515962), 0.17760244, material_for(Color::new(157, 172, 180, 255))),
    Object::sphere(Vector3::new(-1.2045689, -2.1014774, -4.515962), 0.15859544, material_for(Color::new(157, 172, 180, 255))),
    Object::sphere(Vector3::new(-0.6413938, 1.1054919, -4.515962), 0.19173793, material_for(Color::new(157, 172, 180, 255))),
    Object::sphere(Vector3::new(-0.8082605, 0.2190125, -4.515962), 0.1332529, material_for(Color::new(157, 172, 180, 255))),
    Object::sphere(Vector3::new(-2.3648136, 1.4145082, -5.0), 0.21478654, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.2279184, 1.0648297, -5.0), 0.17907165, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.218332, 0.88853544, -5.0), 0.16743127, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.8186533, -2.0611403, -5.0), 0.09933115, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.8879354, -1.9398968, -5.0), 0.13165598, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(1.0167547, -1.8651711, -5.0), 0.011546969, material_for(Color::new(182, 188, 121, 255))),
    Object::sphere(Vector3::new(-1.9687645, -1.7962108, -5.0), 0.09018523, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.9687645, -1.6916361, -5.0), 0.06757721, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.3498156, -1.3625467, -5.0), 0.06137337, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.332495, -1.2586236, -5.0), 0.062182534, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.3036275, -1.1720212, -5.0), 0.06027715, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.8481747, -2.2120693, -5.0), 0.04966567, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.0091789, -1.091192, -5.0), 0.118462384, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.263213, -1.079645, -5.0), 0.06582807, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.1939309, -1.010363, -5.0), 0.08812856, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.1188755, -0.8833459, -5.0), 0.07505555, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.170837, -0.7621023, -5.0), 0.059441715, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.6750933, -0.9905361, -5.0), 0.045092486, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.9468138, -1.2546722, -5.0), 0.09309501, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.9072223, -1.3827472, -5.0), 0.095568515, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.9514439, -1.5889804, -5.0), 0.07767459, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.916803, -1.529978, -5.0), 0.07047463, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.7724652, -0.9237604, -5.0), 0.075055346, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.9572173, -0.92953396, -5.0), 0.06454971, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.882162, -0.81906176, -5.0), 0.0879395, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.8494375, 1.3326304, -4.28099), 0.08524466, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.9161341, 1.466553, -4.28099), 0.08533389, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.69275886, 1.142071, -4.18078), 0.033160552, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.738615, 1.1827495, -4.18078), 0.041528054, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.7679046, 1.2119368, -4.18078), 0.04096307, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.8252918, 1.908375, -4.18078), 0.24525651, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.6964748, 2.252993, -4.18078), 0.23212446, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.92603123, 1.5446608, -4.18078), 0.087029845, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.83908296, 1.6342248, -4.18078), 0.07858701, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.5074015, 2.0782037, -4.18078), 0.3925781, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.1763349, 2.1435077, -4.18078), 0.42201737, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.0900744, 0.6526319, -4.18078), 0.08221024, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.0427505, 0.5608337, -4.18078), 0.04152815, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.9925184, 0.5409892, -4.18078), 0.025997201, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.836045, 1.2187264, -4.18078), 0.069120176, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.92206186, 1.264818, -4.18078), 0.061822828, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.0041301, 1.3563075, -4.18078), 0.07858671, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.0451556, 1.51022, -4.18078), 0.10848545, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.91723424, 1.3420585, -4.18078), 0.014482617, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-0.89309657, 1.3420585, -4.18078), 0.001, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.3205506, 1.883893, -5.0), 0.28242975, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.4306445, -0.36950424, -5.0), 0.05446709, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.355589, -0.34063655, -5.0), 0.053228837, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.3036275, -0.24826068, -5.0), 0.049665526, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.263213, -0.3287668, -5.0), 0.04760934, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.2527418, -0.21163908, -5.0), 0.06582825, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.217025, -0.13279057, -5.0), 0.06245013, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.214132, -0.06350857, -5.0), 0.07505549, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.196607, 0.040414687, -5.0), 0.06582783, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.3567733, -0.45033324, -5.0), 0.036968514, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.3629386, -0.38859642, -5.0), 0.048989832, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.3786527, -0.32331613, -5.0), 0.049328897, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.4219573, -0.2577191, -5.0), 0.06582826, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.4601433, -0.17320508, -5.0), 0.061373334, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.3764087, -0.5241076, -5.0), 0.012909984, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.3318452, -0.48006088, -4.921124), 0.025819834, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.7274694, 0.04895299, -4.921124), 0.30495077, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.4376535, -0.0784391, -4.921124), 0.046858445, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.5614505, 0.306982, -4.921124), 0.095254906, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.5172074, 0.42618176, -4.921124), 0.07387169, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.4831128, 0.5455128, -4.921124), 0.06146493, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.4490181, 0.66668886, -4.921124), 0.07894289, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.4092412, 0.8016572, -4.921124), 0.11033269, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.1187137, 0.62226474, -4.921124), 0.025412612, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.212756, 1.6076514, -4.921124), 0.18085799, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.59433, 1.15925, -4.921124), 0.32943404, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.7330636, 1.5285722, -4.921124), 0.52675205, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.8862579, 0.71061075, -4.921124), 0.39142796, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.9559553, 0.17615525, -4.921124), 0.22277725, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.6649503, 0.38640496, -4.921124), 0.12882935, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.7673738, -0.13356218, -4.921124), 0.2986383, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.8524704, -0.5047104, -4.921124), 0.23186803, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.994531, -0.8637286, -4.921124), 0.17056762, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.9661189, -0.561158, -4.921124), 0.22414975, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.9776322, -0.16812746, -4.921124), 0.22871329, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-2.1057587, 0.32389808, -4.921124), 0.18889245, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.8033472, -1.7104096, -4.921124), 0.08984689, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.8183757, -1.5853964, -4.921124), 0.08195297, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.8199918, -1.4212663, -4.921124), 0.08821488, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.8267977, -1.2655157, -4.921124), 0.13253564, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.8799934, -1.1936795, -4.921124), 0.091802485, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.8934454, -1.0569681, -4.921124), 0.10165029, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.7561041, -1.5777335, -4.921124), 0.09457453, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.6867061, -1.4361801, -4.921124), 0.09457436, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.7389154, -1.3120197, -4.921124), 0.09660127, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(0.12501343, -1.9433892, -4.921124), 0.19395293, material_for(Color::new(157, 172, 180, 255))),
    Object::sphere(Vector3::new(0.88068444, -1.4262885, -4.921124), 0.17047267, material_for(Color::new(182, 188, 121, 255))),
    Object::sphere(Vector3::new(-0.11716671, 0.51700765, -4.921124), 0.6150944, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.7231059, -0.3271149, -4.921124), 0.66445506, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.6000462, 0.78844196, -4.921124), 0.4752221, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.018436708, 0.5823984, -4.921124), 0.5512244, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.22085842, 0.7494156, -4.921124), 0.34890267, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.46766028, 0.6819968, -5.601494), 0.47799963, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.7287401, 0.81840926, -5.601494), 0.22941042, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.8673661, 0.7179533, -5.601494), 0.23428278, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.7105768, 0.50843185, -5.601494), 0.16363001, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.5886598, 0.517692, -5.601494), 0.15856636, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.5257269, 0.5357844, -5.601494), 0.15974924, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.4382238, 0.57659024, -5.601494), 0.17355601, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.3386813, 0.5957681, -5.601494), 0.1396257, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.99388444, 0.68136835, -5.601494), 0.20788515, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.1054701, 0.5541963, -5.601494), 0.110147044, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.1965889, 0.54331595, -5.601494), 0.118030876, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.2871414, 0.48510352, -5.601494), 0.06752841, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.1383765, 0.6920811, -5.601494), 0.1459259, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.2677373, 0.7179533, -5.601494), 0.094176225, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.495811, 0.9984199, -5.601494), 0.34152308, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.4567006, 1.1906809, -5.601494), 0.17795987, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.5775144, 1.2698165, -5.601494), 0.17760666, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.7205006, 1.3683593, -5.601494), 0.12887521, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.531851, -0.7566296, -5.601494), 0.46371874, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.42259806, -0.6098099, -5.601494), 0.6318856, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.040318795, -0.856564, -5.601494), 0.44816598, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-0.3033356, -0.8793892, -5.601494), 0.42713606, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-0.73660564, -0.8075173, -5.601494), 0.37631595, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-1.2482728, -0.7173537, -5.937371), 0.3975613, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-1.6482699, -0.49151218, -5.937371), 0.331147, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-2.5481398, -0.121741384, -5.937371), 0.33872503, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-3.2634013, -0.30851474, -5.937371), 0.4360364, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-2.9202118, -0.0039002933, -5.937371), 0.23798928, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-3.3113925, 0.082270786, -5.937371), 0.19318509, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-2.9939177, 0.114754684, -5.937371), 0.23670192, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-2.8177688, 0.19882055, -5.937371), 0.18725476, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-1.7893862, 0.047991395, -5.937371), 0.15036128, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-1.6031799, -0.13026176, -5.937371), 0.13521908, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-3.5151713, -0.7234107, -5.937371), 0.2700905, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.78770244, -1.0986587, -5.937371), 0.20349386, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.12520237, -1.8756512, -4.7634187), 0.06855891, material_for(Color::new(183, 159, 42, 255))),
    Object::sphere(Vector3::new(0.12311193, -1.8958334, -4.825995), 0.12887663, material_for(Color::new(182, 188, 121, 255))),
    Object::sphere(Vector3::new(0.854968, -1.3638947, -4.7226825), 0.033435345, material_for(Color::new(183, 159, 42, 255))),
    Object::sphere(Vector3::new(-0.5990447, 1.0322909, -4.176982), 0.0561448, material_for(Color::new(183, 159, 42, 255))),
    Object::sphere(Vector3::new(-0.75707763, 0.20210952, -4.176982), 0.05035522, material_for(Color::new(183, 159, 42, 255))),
    Object::sphere(Vector3::new(-1.1093274, -1.9340883, -4.176982), 0.04550146, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(-1.1265227, -1.9575487, -4.2043777), 0.107091874, material_for(Color::new(182, 188, 121, 255))),
    Object::sphere(Vector3::new(-2.124298, -1.9903841, -4.2043777), 0.04475894, material_for(Color::new(183, 159, 42, 255))),
    Object::sphere(Vector3::new(-2.3136282, 0.6068496, -4.2043777), 0.035008483, material_for(Color::new(183, 159, 42, 255))),
    Object::sphere(Vector3::new(-2.1361105, 0.7356064, -4.2043777), 0.024273908, material_for(Color::new(183, 159, 42, 255))),
    Object::sphere(Vector3::new(-1.0914623, -1.8863083, -4.060204), 0.02614389, material_for(Color::new(183, 159, 42, 255))),
    Object::sphere(Vector3::new(1.1345733, 2.0143697, -4.060204), 0.6500554, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.5632037, 1.7924728, -4.060204), 0.55429333, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(2.0576217, 1.5638051, -4.060204), 0.5774822, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(2.345043, 1.0495957, -4.060204), 0.20225109, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(2.114432, 2.1706922, -4.060204), 0.28043836, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.6546578, 2.0578372, -4.060204), 0.48189452, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(0.457025, 2.5332708, -4.8571773), 0.47665474, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-0.43356517, 4.0581656, -6.728544), 1.0720762, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(-2.526792, -0.29143515, -5.609857), 0.09735103, material_for(Color::new(21, 28, 26, 255))),
    Object::sphere(Vector3::new(0.9021208, 1.5854923, -10.688196), 0.77421457, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.2374609, 1.711669, -10.688196), 0.52997434, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.6057894, 1.6167579, -10.688196), 0.37596536, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(1.9623246, 1.5797335, -10.688196), 0.21553819, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(2.055606, 1.5034363, -10.688196), 0.19513892, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(2.6770272, 0.9470688, -10.688196), 0.3806577, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(2.922123, 0.7396474, -10.688196), 0.26180685, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(3.3075664, 0.77056533, -10.688196), 0.30481666, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(3.723731, 0.7452275, -10.688196), 0.24497505, material_for(Color::new(81, 107, 168, 255))),
    Object::sphere(Vector3::new(4.0727496, 0.85157496, -10.688196), 0.19278325, material_for(Color::new(81, 107, 168, 255))),
    ];

    objects.extend(preset_cubes());
    objects
}

/// Esfera envolvente de toda la escena: centro y radio.
/// Sirve para plantar la camara a una distancia que encuadre el diorama
/// completo sin tener que hardcodear numeros.
pub fn bounding_sphere(objects: &[Object]) -> (Vector3, f32) {
    if objects.is_empty() {
        return (Vector3::zero(), 1.0);
    }

    let mut min = Vector3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vector3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

    // cada forma sabe su propia caja: la esfera la arma con su radio y el
    // cubo ya es una
    for object in objects {
        let (low, high) = object.bounds();
        min.x = min.x.min(low.x);
        min.y = min.y.min(low.y);
        min.z = min.z.min(low.z);
        max.x = max.x.max(high.x);
        max.y = max.y.max(high.y);
        max.z = max.z.max(high.z);
    }

    let center = (min + max) * 0.5;
    let radius = (max - center).length().max(0.001);
    (center, radius)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Los colores de contorno no tienen que reflejar nada.
    #[test]
    fn ink_colors_get_matte_materials() {
        for ink in [
            Color::new(21, 28, 26, 255),
            Color::new(14, 27, 61, 255),
            Color::new(1, 40, 83, 255),
        ] {
            assert_eq!(material_for(ink).albedo.specular, 0.0);
        }
    }

    /// El casi blanco es el unico espejo de la paleta.
    #[test]
    fn near_white_reflects_the_scene() {
        assert!(material_for(Color::new(157, 172, 180, 255)).albedo.reflection > 0.0);
        assert_eq!(material_for(Color::new(21, 28, 26, 255)).albedo.reflection, 0.0);
    }

    /// El amarillo intenso es el unico que deja pasar la luz.
    #[test]
    fn intense_yellow_is_the_glass_of_the_palette() {
        let glass = material_for(Color::new(183, 159, 42, 255));
        assert!(glass.albedo.refraction > 0.0);
        assert!(glass.refractive_index > 1.0);

        let opaque = material_for(Color::new(81, 107, 168, 255));
        assert_eq!(opaque.albedo.refraction, 0.0);
    }

    #[test]
    fn painted_colors_keep_a_highlight() {
        let material = material_for(Color::new(81, 107, 168, 255));
        assert!(material.albedo.specular > 0.0);
    }

    /// Las luces se plantan en funcion del tamano de la escena, no pegadas
    /// al origen: si no, con una escena mas grande quedarian adentro.
    #[test]
    fn lights_scale_with_the_scene() {
        let center = Vector3::new(1.0, 2.0, -3.0);
        let lights = default_lights(center, 4.0);

        assert_eq!(lights.len(), 2);
        for light in &lights {
            assert!((light.position - center).length() > 4.0);
        }
        assert!(lights[0].intensity > lights[1].intensity, "la principal manda");
    }

    /// La esfera envolvente tiene que cubrir hasta el borde de cada cuerpo,
    /// no solo su centro. En el cubo el punto mas lejano es una esquina.
    #[test]
    fn bounding_sphere_covers_every_object() {
        let objects = preset_scene();
        let (center, radius) = bounding_sphere(&objects);

        for object in &objects {
            let (min, max) = object.bounds();
            for corner in [
                Vector3::new(min.x, min.y, min.z),
                Vector3::new(min.x, min.y, max.z),
                Vector3::new(min.x, max.y, min.z),
                Vector3::new(min.x, max.y, max.z),
                Vector3::new(max.x, min.y, min.z),
                Vector3::new(max.x, min.y, max.z),
                Vector3::new(max.x, max.y, min.z),
                Vector3::new(max.x, max.y, max.z),
            ] {
                let reach = (corner - center).length();
                assert!(reach <= radius + 1e-3, "{reach} pasa de {radius}");
            }
        }
    }

    /// El diorama tiene las dos formas, no solo esferas.
    #[test]
    fn the_preset_has_spheres_and_cubes() {
        let objects = preset_scene();

        assert!(objects.iter().any(|o| matches!(o, Object::Sphere(_))));
        assert!(objects.iter().any(|o| matches!(o, Object::Cube(_))));
    }

    /// Los cubos del piso van pegados abajo de todo, sin meterse en el dibujo.
    /// Recordar que -Y es arriba: abajo del dibujo es Y mas grande.
    #[test]
    fn the_floor_sits_under_the_drawing() {
        for cube in floor_cubes() {
            let (min, _) = cube.bounds();
            assert!(min.y >= GROUND_Y, "{} se mete en el dibujo", min.y);
        }
    }

    /// Los cubos sueltos apoyan sobre el piso: no flotan ni se hunden.
    #[test]
    fn accent_cubes_rest_on_the_floor() {
        for cube in accent_cubes() {
            let (_, max) = cube.bounds();
            assert!((max.y - GROUND_Y).abs() < 1e-4, "{} no apoya", max.y);
        }
    }
}
