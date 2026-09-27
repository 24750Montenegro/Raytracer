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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bvh::Bvh;
    use crate::light::Light;
    use crate::material::{Albedo, WATER_INDEX};
    use crate::object::Object;
    use crate::scene::Scene;
    use crate::texture::TILE;

    /// Camara imaginaria justo encima del punto, mirando hacia abajo.
    const VIEW: Vector3 = Vector3 { x: 0.0, y: -1.0, z: 0.0 };

    fn white_matte(albedo: f32) -> Material {
        Material::new(Color::new(255, 255, 255, 255), Albedo::new(albedo, 0.0, 0.0, 0.0), 1.0)
    }

    /// Los tests miran colores de raylib: mas facil de leer que los f32.
    fn shade_color(
        scene: &Scene,
        material: &Material,
        point: Vector3,
        normal: Vector3,
        view_dir: Vector3,
    ) -> Color {
        // los tests de sombreado no usan texturas: el color sale del
        // material y la cara va lisa, sin relieve que tape nada
        let base = Rgb::from_color(material.diffuse);
        shade(scene, material, base, point, normal, view_dir, None).to_color()
    }

    /// Escena minima: una luz blanca a un metro sobre el punto (0,0,0),
    /// mirando hacia abajo. Recordar que en esta escena -Y es arriba.
    fn single_light_scene(intensity: f32, ambient: f32) -> Scene {
        Scene::for_test(
            vec![Light::new(
                Vector3::new(0.0, -1.0, 0.0),
                Rgb::new(1.0, 1.0, 1.0),
                intensity,
            )],
            Rgb::new(ambient, ambient, ambient),
        )
    }

    #[test]
    fn surface_facing_the_light_is_brightest() {
        let scene = single_light_scene(1.0, 0.0);
        let material = white_matte(1.0);

        let facing = shade_color(&scene, &material, Vector3::zero(), Vector3::new(0.0, -1.0, 0.0), VIEW);
        let tilted = shade_color(&scene, &material, Vector3::zero(), Vector3::new(0.7, -0.7, 0.0), VIEW);

        assert_eq!(facing.r, 255);
        assert!(tilted.r < facing.r, "de canto tiene que llegar menos luz");
    }

    #[test]
    fn surface_turned_away_only_gets_ambient() {
        let scene = single_light_scene(1.0, 0.25);
        let material = white_matte(1.0);

        // normal apuntando al lado opuesto de la luz
        let away = shade_color(&scene, &material, Vector3::zero(), Vector3::new(0.0, 1.0, 0.0), VIEW);

        assert_eq!(away.r, (0.25 * 255.0) as u8);
    }

    /// Dos materiales del mismo color pero distinto albedo tienen que
    /// verse distinto: eso es justamente lo que aporta el albedo.
    #[test]
    fn albedo_scales_the_reflected_light() {
        let scene = single_light_scene(1.0, 0.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);
        let color = Color::new(200, 200, 200, 255);
        let bright_material = Material::new(color, Albedo::new(0.9, 0.0, 0.0, 0.0), 1.0);
        let dark_material = Material::new(color, Albedo::new(0.2, 0.0, 0.0, 0.0), 1.0);

        let bright = shade_color(&scene, &bright_material, Vector3::zero(), normal, VIEW);
        let dark = shade_color(&scene, &dark_material, Vector3::zero(), normal, VIEW);

        assert!(dark.r < bright.r);
        assert!(bright.r > 0);
    }

    #[test]
    fn light_color_tints_the_material() {
        let mut scene = single_light_scene(1.0, 0.0);
        scene.lights[0].color = Rgb::new(1.0, 0.0, 0.0);

        let white = white_matte(1.0);
        let lit = shade_color(&scene, &white, Vector3::zero(), Vector3::new(0.0, -1.0, 0.0), VIEW);

        assert_eq!(lit.r, 255);
        assert_eq!(lit.g, 0);
        assert_eq!(lit.b, 0);
    }

    /// El brillo especular aparece donde el rebote de la luz apunta al ojo.
    #[test]
    fn specular_highlight_shows_up_head_on() {
        let scene = single_light_scene(1.0, 0.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);
        let shiny = Material::new(Color::new(30, 30, 30, 255), Albedo::new(0.2, 0.8, 0.0, 0.0), 32.0);

        // luz justo arriba y ojo justo arriba: el rebote va derecho a la camara
        let head_on = shade_color(&scene, &shiny, Vector3::zero(), normal, VIEW);
        // mirando de costado, el brillo se apaga
        let off_axis = shade_color(
            &scene,
            &shiny,
            Vector3::zero(),
            normal,
            Vector3::new(0.9, -0.44, 0.0).normalized(),
        );

        assert!(head_on.r > off_axis.r, "{} vs {}", head_on.r, off_axis.r);
    }

    /// Mismo material pero mas pulido: el brillo se cierra, asi que fuera del
    /// eje cae mas rapido.
    #[test]
    fn higher_shininess_narrows_the_highlight() {
        let scene = single_light_scene(1.0, 0.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);
        let off_axis = Vector3::new(0.5, -0.86, 0.0).normalized();
        let color = Color::new(30, 30, 30, 255);

        let soft = Material::new(color, Albedo::new(0.2, 0.8, 0.0, 0.0), 8.0);
        let sharp = Material::new(color, Albedo::new(0.2, 0.8, 0.0, 0.0), 128.0);

        let soft_edge = shade_color(&scene, &soft, Vector3::zero(), normal, off_axis);
        let sharp_edge = shade_color(&scene, &sharp, Vector3::zero(), normal, off_axis);

        assert!(sharp_edge.r < soft_edge.r);
    }

    /// Un material mate no tiene brillo por mas que la luz le pegue de frente.
    #[test]
    fn matte_material_has_no_highlight() {
        let scene = single_light_scene(1.0, 0.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);
        let matte = Material::matte(Color::new(0, 0, 0, 255));

        let lit = shade_color(&scene, &matte, Vector3::zero(), normal, VIEW);

        assert_eq!(lit.r, 0, "el negro mate tiene que seguir negro");
    }

    fn white_light_scene(ambient: f32) -> Scene {
        Scene::for_test(
            vec![Light::new(
                Vector3::new(0.0, -4.0, 0.0),
                Rgb::new(1.0, 1.0, 1.0),
                1.0,
            )],
            Rgb::new(ambient, ambient, ambient),
        )
    }

    fn with_blocker(mut scene: Scene, center: Vector3, size: f32) -> Scene {
        scene.objects = vec![Object::cube(
            center,
            size,
            Material::matte(Color::new(0, 0, 0, 255)),
        )];
        scene.bvh = Bvh::build(&mut scene.objects);
        scene
    }

    /// Un bloque entre el punto y la luz lo deja en sombra: solo le queda
    /// la ambiente.
    #[test]
    fn blocked_light_leaves_only_ambient() {
        let lit_scene = white_light_scene(0.1);
        let shadowed = with_blocker(white_light_scene(0.1), Vector3::new(0.0, -2.0, 0.0), 1.0);

        let material = white_matte(1.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);

        let lit = shade_color(&lit_scene, &material, Vector3::zero(), normal, VIEW);
        let dark = shade_color(&shadowed, &material, Vector3::zero(), normal, VIEW);

        assert!(dark.r < lit.r);
        assert_eq!(dark.r, (0.1 * 255.0) as u8, "en sombra solo queda la ambiente");
    }

    /// El agua deja pasar la luz: lo que tiene debajo se sigue viendo.
    #[test]
    fn transparent_blockers_let_the_light_through() {
        let mut scene = white_light_scene(0.0);
        scene.objects = vec![Object::cube(
            Vector3::new(0.0, -2.0, 0.0),
            1.0,
            Material::water(Color::new(60, 120, 190, 255)),
        )];
        scene.bvh = Bvh::build(&mut scene.objects);

        let lit = shade_color(
            &scene,
            &white_matte(1.0),
            Vector3::zero(),
            Vector3::new(0.0, -1.0, 0.0),
            VIEW,
        );

        assert_eq!(lit.r, 255, "el agua no tendria que apagar el fondo");
    }

    /// Lo que esta por detras de la luz no hace sombra.
    #[test]
    fn blockers_behind_the_light_do_not_cast_shadow() {
        let scene = with_blocker(white_light_scene(0.0), Vector3::new(0.0, -9.0, 0.0), 1.0);

        let lit = shade_color(
            &scene,
            &white_matte(1.0),
            Vector3::zero(),
            Vector3::new(0.0, -1.0, 0.0),
            VIEW,
        );

        assert_eq!(lit.r, 255);
    }

    /// El punto no se tapa a si mismo: el bloque sobre el que se sombrea
    /// tambien esta en el BVH y el rayo de sombra arranca sobre su superficie.
    #[test]
    fn surface_does_not_shadow_itself() {
        let scene = with_blocker(white_light_scene(0.0), Vector3::new(0.0, 0.0, 0.0), 2.0);

        // en el medio de la cara de arriba, la que mira a la luz
        let point = Vector3::new(0.0, -1.0, 0.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);
        let lit = shade_color(&scene, &white_matte(1.0), point, normal, VIEW);

        assert_eq!(lit.r, 255, "el bloque no deberia sombrearse a si mismo");
    }

    /// Un bloque blanco con media cara hundida, para ver que hace el
    /// relieve. El color es parejo a proposito: asi toda diferencia entre
    /// dos frames sale del relieve y no del dibujo de la textura.
    fn carved_scene(depth: f32) -> Scene {
        let mut scene = white_light_scene(0.1);

        // escalon a lo ancho de v, que es por donde le entra la luz
        let mut height = vec![0.0; TILE * TILE];
        for y in TILE / 2..TILE {
            for x in 0..TILE {
                height[y * TILE + x] = 1.0;
            }
        }
        let carved = Texture::from_relief(
            vec![Rgb::new(1.0, 1.0, 1.0); TILE * TILE],
            &height,
            1.0,
        )
        .with_depth(depth);

        scene.textures = vec![Texture::plain(), carved];
        scene.objects = vec![Object::cube(
            Vector3::new(0.0, 0.0, -10.0),
            4.0,
            Material::matte(Color::new(255, 255, 255, 255)).textured(1),
        )];
        scene.bvh = Bvh::build(&mut scene.objects);
        scene
    }

    /// Cuanta luz devuelve la cara de arriba del bloque, sumada sobre una
    /// grilla de rayos.
    fn face_brightness(scene: &Scene) -> u32 {
        let mut total = 0;

        for row in 0..5 {
            for column in 0..5 {
                let x = -1.6 + 0.8 * column as f32;
                let z = -11.6 + 0.8 * row as f32;
                let origin = Vector3::new(x, -8.0, z);
                let down = Vector3::new(0.0, 1.0, 0.0);

                total += trace(scene, origin, down, MAX_DEPTH).to_color().r as u32;
            }
        }

        total
    }

    /// El relieve de la textura tambien hace sombra: los texels del fondo
    /// del escalon reciben menos luz que la misma cara dibujada plana. Sin
    /// esto el mapa de alturas solo correria el dibujo, y las juntas entre
    /// piedras nunca se oscurecerian.
    #[test]
    fn the_relief_shadows_its_own_face() {
        let flat = face_brightness(&carved_scene(0.0));
        let carved = face_brightness(&carved_scene(0.3));

        assert!(carved < flat, "el relieve no apago nada: {carved} contra {flat}");
    }

    /// Pero no puede apagar la cara entera: lo que esta a la vista sigue
    /// recibiendo la luz igual que antes.
    #[test]
    fn the_relief_shadow_does_not_swallow_the_face() {
        let flat = face_brightness(&carved_scene(0.0));
        let carved = face_brightness(&carved_scene(0.3));

        assert!(carved > flat / 2, "quedo demasiado oscuro: {carved} contra {flat}");
    }

    /// Geometria de los tests de reflejo. El rayo entra en diagonal porque
    /// contra una cara plana uno alineado a los ejes rebota por donde vino:
    /// pega en la cara del frente del espejo y sale hacia el bloque rojo.
    const MIRROR_CENTER: Vector3 = Vector3 { x: 0.0, y: 0.0, z: -10.0 };
    const RAY_ORIGIN: Vector3 = Vector3 { x: 4.0, y: 0.0, z: -5.0 };
    const RAY_DIR: Vector3 = Vector3 { x: -0.7071, y: 0.0, z: -0.7071 };

    fn mirror_test_scene(center_material: Material) -> Scene {
        let mut scene = white_light_scene(0.2);
        scene.objects = vec![
            // el que se refleja, fuera del camino del rayo directo
            Object::cube(
                Vector3::new(-4.24, 0.0, -4.76),
                3.0,
                Material::matte(Color::new(255, 0, 0, 255)),
            ),
            Object::cube(MIRROR_CENTER, 2.0, center_material),
        ];
        scene.bvh = Bvh::build(&mut scene.objects);
        scene
    }

    /// Un espejo tiene que traer el color de lo que tiene enfrente.
    #[test]
    fn mirror_picks_up_the_block_in_front_of_it() {
        let gray = Color::new(120, 120, 120, 255);
        let matte_scene = mirror_test_scene(Material::matte(gray));
        let mirror_scene = mirror_test_scene(Material::mirror(gray));

        let matte = trace(&matte_scene, RAY_ORIGIN, RAY_DIR, MAX_DEPTH).to_color();
        let mirror = trace(&mirror_scene, RAY_ORIGIN, RAY_DIR, MAX_DEPTH).to_color();

        // el gris mate tiene los tres canales parejos; el espejo tiene que
        // inclinarse al rojo del bloque que refleja
        assert!(
            mirror.r as i32 - mirror.b as i32 > matte.r as i32 - matte.b as i32,
            "el espejo tendria que traer el rojo de enfrente: {mirror:?} vs {matte:?}"
        );
    }

    /// Sin rebotes permitidos, un espejo solo muestra su luz directa.
    #[test]
    fn depth_zero_skips_the_reflection() {
        let scene = mirror_test_scene(Material::mirror(Color::new(120, 120, 120, 255)));

        let with_bounces = trace(&scene, RAY_ORIGIN, RAY_DIR, MAX_DEPTH).to_color();
        let flat = trace(&scene, RAY_ORIGIN, RAY_DIR, 0).to_color();

        assert!(
            flat.r < with_bounces.r,
            "sin rebotes no deberia llegar el rojo: {flat:?} vs {with_bounces:?}"
        );
    }

    /// Dos espejos enfrentados se reflejan entre si sin colgar el trazado:
    /// la profundidad corta la cadena.
    #[test]
    fn facing_mirrors_terminate() {
        let mirror = Material::mirror(Color::new(200, 200, 200, 255));
        let mut scene = white_light_scene(0.1);
        scene.objects = vec![
            Object::cube(Vector3::new(-3.0, 0.0, -10.0), 2.0, mirror),
            Object::cube(Vector3::new(3.0, 0.0, -10.0), 2.0, mirror),
        ];
        scene.bvh = Bvh::build(&mut scene.objects);

        // desde el medio: el rebote va de un espejo al otro y vuelta
        let origin = Vector3::new(0.0, 0.0, -10.0);
        let direction = Vector3::new(1.0, 0.0, 0.0);

        // si la recursion no cortara, esto no volveria nunca
        let color = trace(&scene, origin, direction, MAX_DEPTH).to_color();
        assert!(color.r > 0);
    }

    /// De frente, el rayo cruza el vidrio sin doblarse.
    #[test]
    fn head_on_rays_pass_straight_through() {
        let incident = Vector3::new(0.0, 0.0, -1.0);
        let normal = Vector3::new(0.0, 0.0, 1.0);

        let through = refract(incident, normal, WATER_INDEX).expect("tenia que pasar");

        assert!((through - incident).length() < 1e-5);
    }

    /// Al entrar en un medio mas denso el rayo se acerca a la normal.
    #[test]
    fn entering_water_bends_toward_the_normal() {
        let incident = Vector3::new(0.7071, 0.0, -0.7071);
        let normal = Vector3::new(0.0, 0.0, 1.0);

        let through = refract(incident, normal, WATER_INDEX)
            .expect("tenia que pasar")
            .normalized();

        // el angulo contra la normal se mide con el coseno: mas grande = mas
        // cerca de la normal
        let incident_cos = -incident.dot(normal);
        let through_cos = -through.dot(normal);
        assert!(through_cos > incident_cos, "{through_cos} vs {incident_cos}");
    }

    /// Desde adentro y muy de canto, el rayo no sale: reflexion total interna.
    #[test]
    fn grazing_rays_from_inside_do_not_escape() {
        // normal apuntando hacia afuera, rayo saliendo casi paralelo a ella
        let normal = Vector3::new(0.0, 0.0, 1.0);
        let incident = Vector3::new(0.9994, 0.0, 0.0349).normalized();

        assert!(refract(incident, normal, WATER_INDEX).is_none());
    }

    /// Un bloque de agua deja ver lo que tiene detras; uno mate no.
    #[test]
    fn water_lets_the_block_behind_show_through() {
        let red = Object::cube(
            Vector3::new(0.0, 0.0, -20.0),
            12.0,
            Material::matte(Color::new(255, 0, 0, 255)),
        );
        let tint = Color::new(180, 180, 180, 255);

        // cielo negro en las dos: el agua tambien refleja, y con el azul de
        // verdad no se sabria si el color vino de atras del bloque o de arriba
        let mut clear_scene = white_light_scene(0.2);
        clear_scene.background = Rgb::new(0.0, 0.0, 0.0);
        clear_scene.objects = vec![
            red,
            Object::cube(Vector3::new(0.0, 0.0, -10.0), 2.0, Material::water(tint)),
        ];
        clear_scene.bvh = Bvh::build(&mut clear_scene.objects);

        let mut solid_scene = white_light_scene(0.2);
        solid_scene.background = Rgb::new(0.0, 0.0, 0.0);
        solid_scene.objects = vec![
            red,
            Object::cube(Vector3::new(0.0, 0.0, -10.0), 2.0, Material::matte(tint)),
        ];
        solid_scene.bvh = Bvh::build(&mut solid_scene.objects);

        let origin = Vector3::new(0.0, 0.0, -4.0);
        let direction = Vector3::new(0.0, 0.0, -1.0);

        let clear = trace(&clear_scene, origin, direction, MAX_DEPTH).to_color();
        let solid = trace(&solid_scene, origin, direction, MAX_DEPTH).to_color();

        assert!(
            clear.r as i32 - clear.b as i32 > solid.r as i32 - solid.b as i32,
            "por el agua tendria que verse el rojo de atras: {clear:?} vs {solid:?}"
        );
    }

    // -- lo que emite luz por su cuenta ------------------------------------

    /// Una lampara se ve encendida aunque no le llegue nada: esa es toda la
    /// diferencia entre emitir luz y reflejarla.
    #[test]
    fn a_lamp_shines_with_no_light_in_the_scene() {
        let dark = Scene::for_test(Vec::new(), Rgb::new(0.0, 0.0, 0.0));
        let mut scene = dark;
        scene.objects = vec![
            Object::cube(
                Vector3::new(0.0, 0.0, -5.0),
                2.0,
                Material::lamp(Color::new(200, 240, 235, 255), 1.0),
            ),
            // el mismo bloque sin emision, corrido al costado
            Object::cube(
                Vector3::new(6.0, 0.0, -5.0),
                2.0,
                Material::lamp(Color::new(200, 240, 235, 255), 0.0),
            ),
        ];
        scene.bvh = Bvh::build(&mut scene.objects);

        let lit = trace(&scene, Vector3::zero(), Vector3::new(0.0, 0.0, -1.0), 0);
        let unlit = trace(
            &scene,
            Vector3::new(6.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, -1.0),
            0,
        );

        assert!(lit.r > 0.5, "la lampara salio en {lit:?}");
        assert!(unlit.r < 0.01, "el apagado tendria que estar negro: {unlit:?}");
    }

    /// Y lo que emite sale con su propio color, no en blanco.
    #[test]
    fn what_a_lamp_emits_carries_its_own_color() {
        let mut scene = Scene::for_test(Vec::new(), Rgb::new(0.0, 0.0, 0.0));
        scene.objects = vec![Object::cube(
            Vector3::new(0.0, 0.0, -5.0),
            2.0,
            Material::lamp(Color::new(40, 255, 200, 255), 1.0),
        )];
        scene.bvh = Bvh::build(&mut scene.objects);

        let color = trace(&scene, Vector3::zero(), Vector3::new(0.0, 0.0, -1.0), 0);

        assert!(color.g > color.r * 2.0, "tendria que salir verde: {color:?}");
        assert!(color.b > color.r, "{color:?}");
    }

    /// Una lampara alumbra lo que tiene al lado y nada mas. Es lo que la hace
    /// barata: un punto que le queda lejos ni siquiera llega al rayo de
    /// sombra.
    #[test]
    fn a_lamp_only_lights_what_is_next_to_it() {
        let mut scene = Scene::for_test(Vec::new(), Rgb::new(0.0, 0.0, 0.0));
        scene.lights = vec![Light::lamp(
            Vector3::new(0.0, -2.0, 0.0),
            Rgb::new(1.0, 1.0, 1.0),
            1.0,
            6.0,
        )];

        let material = white_matte(1.0);
        let up = Vector3::new(0.0, -1.0, 0.0);

        let close = shade_color(&scene, &material, Vector3::zero(), up, VIEW);
        let far = shade_color(&scene, &material, Vector3::new(0.0, 0.0, 20.0), up, VIEW);

        assert!(close.r > 100, "pegado a la lampara salio {}", close.r);
        assert_eq!(far.r, 0, "a veinte metros no tendria que llegar nada");
    }
}
