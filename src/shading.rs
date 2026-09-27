// shading.rs
// Modelo de iluminacion: difuso de Lambert, brillo especular de Phong y
// reflejos y refracciones por trazado recursivo.
//
// Antes cada cuerpo se pintaba con su color plano, asi que un bloque se veia
// como una silueta de un solo tono. Con Lambert el brillo de cada punto depende
// del angulo entre la normal y la luz, que es lo que da la sensacion de
// volumen; el especular agrega el reflejo puntual de la luz. El albedo
// reparte cuanta luz sale por cada uno de esos dos caminos.
//
// Ademas cada luz se consulta con un rayo de sombra: si otro cuerpo se
// interpone, esa luz no aporta nada a este punto.
//
// Las superficies reflectantes tiran un rayo mas desde el punto de impacto y
// las transparentes otro que sigue para adentro doblado por la ley de Snell.
// Cada uno de esos rayos puede volver a rebotar: ese encadenado es lo que
// limita MAX_DEPTH.
//
// El relieve de la textura entra dos veces en la cuenta. Primero al elegir
// que texel se ve: el rayo se mete en el mapa de alturas y camina hasta
// chocar, asi que lo que se pinta depende de desde donde se mire. Y despues
// en cada luz, porque el mismo relieve puede taparla: eso es lo que deja en
// sombra las juntas entre piedras y lo que termina de despegar el bloque de
// una calcomania.
use raylib::prelude::*;

use crate::color::Rgb;
use crate::material::Material;
use crate::scene::Scene;
use crate::texture::{Surface, Texture};

/// Cuanto se despegan de la superficie los rayos que salen de ella. Sin este
/// empujon el cuerpo se toca a si mismo por error de redondeo: en las sombras
/// sale el moteado clasico y en los reflejos, manchas negras.
const SURFACE_BIAS: f32 = 1e-3;

/// Cuantos rebotes puede encadenar un rayo: el reflejo de un reflejo de un
/// reflejo. Cada nivel mas casi no se nota y multiplica el trabajo.
pub const MAX_DEPTH: u32 = 3;

/// Color que trae el rayo que sale de origin en esa direccion.
/// depth es cuantos rebotes le quedan permitidos.
pub fn trace(scene: &Scene, origin: Vector3, direction: Vector3, depth: u32) -> Rgb {
    let Some((index, distance)) = scene.bvh.nearest_hit(&origin, &direction, &scene.objects) else {
        return scene.background;
    };

    // punto y superficie solo para el cuerpo que gano, no para los descartados
    let object = &scene.objects[index];
    let point = origin + direction * distance;
    let surface = object.surface(point);
    let material = object.material();
    let texture = &scene.textures[material.texture as usize];

    // dentro de un cuerpo transparente el rayo sale por la cara de adentro:
    // ahi la normal apunta al lado contrario del que hay que iluminar
    let facing = if direction.dot(surface.normal) < 0.0 {
        surface.normal
    } else {
        -surface.normal
    };

    // del punto hacia la camara: lo necesitan el especular, para saber desde
    // donde se mira, y el relieve, para saber por donde entra el rayo
    let view_dir = -direction;

    // el rayo se mete en el relieve antes de leer el texel: lo que se ve no
    // es lo que hay donde pego la cara plana, sino donde choca adentro
    let relief = Relief::traced(texture, &surface, facing, view_dir);
    let (texel, tilt) = texture.sample(relief.u, relief.v);

    // la del mapa normal es la que ilumina; la de la cara sigue siendo la que
    // despega los rayos de la superficie, porque es la geometria de verdad
    let bumped = surface.bumped(facing, tilt);

    // el color del material tine al del texel: en los bloques es el gris que
    // los separa entre si, y en lo que no lleva textura es el color a secas
    let base = Rgb::from_color(material.diffuse) * texel;

    let mut color = shade(scene, material, base, point, bumped, view_dir, Some(&relief));

    // lo que el cuerpo emite por su cuenta va aparte del sombreado: no
    // depende de que le llegue ninguna luz, que es justamente lo que hace
    // que una lampara se vea encendida en la sombra y de noche
    if material.emission > 0.0 {
        color = color + base * material.emission;
    }

    if depth == 0 {
        return color;
    }

    if material.albedo.reflection > 0.0 {
        let bounce = reflect(direction, bumped).normalized();
        // el relieve puede tirar el rebote para adentro del cuerpo, y desde
        // ahi el rayo no ve mas que el propio bloque: ahi manda la cara
        let bounce = if bounce.dot(facing) > 0.0 {
            bounce
        } else {
            reflect(direction, facing).normalized()
        };

        // un metal devuelve el mundo tenido con su color; cualquier otra
        // cosa lo devuelve tal cual
        color = color
            + trace(scene, offset(point, facing, bounce), bounce, depth - 1)
                * material.albedo.reflection
                * mirror_tint(material, base);
    }

    if material.albedo.refraction > 0.0 {
        // lo que cruza se dobla contra la cara, no contra el relieve: por
        // dentro del cuerpo el mapa normal no describe nada
        if let Some(through) = refract(direction, surface.normal, material.refractive_index) {
            let through = through.normalized();
            color = color
                + trace(scene, offset(point, facing, through), through, depth - 1)
                    * material.albedo.refraction;
        }
    }

    color
}

/// Donde quedo el rayo dentro del relieve de la textura, y con que ejes se
/// lo recorrio. Es lo que hace falta para preguntarle despues si una luz le
/// llega o si se la tapa un bulto de la propia textura.
struct Relief<'a> {
    texture: &'a Texture,
    surface: &'a Surface,
    /// La normal de la cara, del lado por el que llego el rayo.
    facing: Vector3,
    /// Coordenadas del impacto, ya corridas por el paralaje.
    u: f32,
    v: f32,
    /// A que altura del relieve quedo, de 0 (fondo) a 1 (superficie).
    height: f32,
}

impl<'a> Relief<'a> {
    /// Mete el rayo en el relieve y se queda con donde choco.
    fn traced(
        texture: &'a Texture,
        surface: &'a Surface,
        facing: Vector3,
        view_dir: Vector3,
    ) -> Self {
        let view = surface.to_tangent(view_dir, facing);
        let (u, v, height) = texture.parallax(surface.u, surface.v, view);

        Relief {
            texture,
            surface,
            facing,
            u,
            v,
            height,
        }
    }

    /// Cuanta luz directa le llega desde esa direccion: 1 a la vista y 0 en
    /// el fondo de un hueco que la tapa.
    fn lit_fraction(&self, to_light: Vector3) -> f32 {
        let tangent = self.surface.to_tangent(to_light, self.facing);

        self.texture.lit_fraction(self.u, self.v, self.height, tangent)
    }
}

/// Despega el origen del rayo de la superficie, del lado hacia el que sale.
#[inline(always)]
fn offset(point: Vector3, normal: Vector3, direction: Vector3) -> Vector3 {
    if direction.dot(normal) < 0.0 {
        point - normal * SURFACE_BIAS
    } else {
        point + normal * SURFACE_BIAS
    }
}

/// Direccion del rayo al cruzar la superficie, por la ley de Snell.
/// Devuelve None en reflexion total interna: el rayo llega tan de canto desde
/// adentro que no logra salir.
fn refract(incident: Vector3, normal: Vector3, refractive_index: f32) -> Option<Vector3> {
    let mut cos_incident = -incident.dot(normal).clamp(-1.0, 1.0);
    let mut eta_from = crate::material::AIR_INDEX;
    let mut eta_to = refractive_index;
    let mut surface = normal;

    if cos_incident < 0.0 {
        // el rayo viene desde adentro del cuerpo: se invierte todo
        cos_incident = -cos_incident;
        std::mem::swap(&mut eta_from, &mut eta_to);
        surface = -normal;
    }

    let eta = eta_from / eta_to;
    let k = 1.0 - eta * eta * (1.0 - cos_incident * cos_incident);
    if k < 0.0 {
        return None;
    }

    Some(incident * eta + surface * (eta * cos_incident - k.sqrt()))
}

/// Luz directa en un punto de la superficie: ambiente, difuso y especular.
/// base es el color que ya trae la superficie, material por textura.
/// view_dir va del punto hacia la camara, ya normalizado.
/// relief es donde cayo el rayo dentro del relieve de la textura; None deja
/// la cara lisa, que es lo que hace falta para probar el modelo de luz solo.
fn shade(
    scene: &Scene,
    material: &Material,
    base: Rgb,
    point: Vector3,
    normal: Vector3,
    view_dir: Vector3,
    relief: Option<&Relief>,
) -> Rgb {
    // la ambiente es el piso: sin ella, lo que no ve ninguna luz queda negro
    let mut diffuse = scene.ambient;
    let mut specular = Rgb::new(0.0, 0.0, 0.0);

    for light in &scene.lights {
        let (to_light, distance) = light.direction_from(point);

        // lo primero es lo mas barato: una lampara que no llega hasta aca se
        // descarta con una resta, antes de tocar la geometria
        let reach = light.falloff(distance);
        if reach <= 0.0 {
            continue;
        }

        // ley del coseno: la superficie recibe menos luz cuanto mas de canto
        // le llega. De espaldas a la luz (producto punto <= 0) no recibe nada.
        let lambert = normal.dot(to_light);
        if lambert <= 0.0 {
            continue;
        }

        // el relieve de la textura tambien tapa la luz, y averiguarlo son
        // cinco lecturas de un vector: se pregunta antes que el rayo de
        // sombra, que recorre el BVH entero
        let lit = relief.map_or(1.0, |relief| relief.lit_fraction(to_light));
        if lit <= 0.0 {
            continue;
        }

        // el orden importa: el coseno es una multiplicacion y el rayo de
        // sombra es un recorrido del BVH, asi que primero se descarta barato
        if in_shadow(scene, point, normal, to_light, distance) {
            continue;
        }

        diffuse = diffuse + light.color * (light.intensity * reach * lambert * lit);

        if material.albedo.specular <= 0.0 {
            continue;
        }

        // Phong: el brillo aparece donde el rebote de la luz apunta al ojo.
        // El exponente controla que tan cerrado queda ese brillo.
        let bounce = reflect(-to_light, normal).dot(view_dir);
        if bounce > 0.0 {
            let highlight = bounce.powf(material.shininess) * light.intensity * reach * lit;
            specular = specular + light.color * highlight;
        }
    }

    // albedo: cuanta luz sale por cada camino en vez de absorberse
    base * diffuse * material.albedo.diffuse
        + specular * material.albedo.specular * mirror_tint(material, base)
}

/// Con que color sale lo que la superficie refleja: el suyo si es un metal,
/// blanco si no.
///
/// En un dielectrico (piedra, madera, plastico) la luz que rebota en la
/// superficie no se mete en el material, asi que sale con el color de la luz
/// y el brillo es blanco. En un metal no hay adentro donde meterse: el
/// rebote se lleva puesto el color del metal. Por eso una chapa de oro tiene
/// brillo dorado y una piedra amarilla, blanco.
fn mirror_tint(material: &Material, base: Rgb) -> Rgb {
    if material.metallic {
        base
    } else {
        Rgb::new(1.0, 1.0, 1.0)
    }
}

/// Hay otro cuerpo entre este punto y la luz?
fn in_shadow(
    scene: &Scene,
    point: Vector3,
    normal: Vector3,
    to_light: Vector3,
    distance: f32,
) -> bool {
    let origin = point + normal * SURFACE_BIAS;
    // se corta en la luz: lo que queda detras de ella no hace sombra
    scene.bvh.any_hit(&origin, &to_light, distance, &scene.objects)
}

/// Rebote de un vector contra la superficie, espejado sobre la normal.
#[inline(always)]
fn reflect(incident: Vector3, normal: Vector3) -> Vector3 {
    incident - normal * (2.0 * incident.dot(normal))
}
