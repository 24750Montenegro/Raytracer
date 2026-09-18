// shading.rs
// Modelo de iluminacion: difuso de Lambert, brillo especular de Phong y
// reflejos y refracciones por trazado recursivo.
//
// Antes cada cuerpo se pintaba con su color plano, asi que una esfera se veia
// como un circulo de un solo tono. Con Lambert el brillo de cada punto depende
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
use raylib::prelude::*;

use crate::color::Rgb;
use crate::material::Material;
use crate::scene::Scene;

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

    // punto y normal solo para el cuerpo que gano, no para los descartados
    let object = &scene.objects[index];
    let point = origin + direction * distance;
    let normal = object.normal_at(point);
    let material = object.material();

    // dentro de un cuerpo transparente el rayo sale por la cara de adentro:
    // ahi la normal apunta al lado contrario del que hay que iluminar
    let facing = if direction.dot(normal) < 0.0 { normal } else { -normal };

    // del punto hacia la camara: el especular necesita saber desde donde se mira
    let mut color = shade(scene, material, point, facing, -direction);

    if depth == 0 {
        return color;
    }

    if material.albedo.reflection > 0.0 {
        let bounce = reflect(direction, facing).normalized();
        color = color
            + trace(scene, offset(point, facing, bounce), bounce, depth - 1)
                * material.albedo.reflection;
    }

    if material.albedo.refraction > 0.0 {
        // en reflexion total interna no sale nada: el rayo se queda adentro
        if let Some(through) = refract(direction, normal, material.refractive_index) {
            let through = through.normalized();
            color = color
                + trace(scene, offset(point, facing, through), through, depth - 1)
                    * material.albedo.refraction;
        }
    }

    color
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
/// view_dir va del punto hacia la camara, ya normalizado.
fn shade(
    scene: &Scene,
    material: &Material,
    point: Vector3,
    normal: Vector3,
    view_dir: Vector3,
) -> Rgb {
    let base = Rgb::from_color(material.diffuse);

    // la ambiente es el piso: sin ella, lo que no ve ninguna luz queda negro
    let mut diffuse = scene.ambient;
    let mut specular = Rgb::new(0.0, 0.0, 0.0);

    for light in &scene.lights {
        let (to_light, distance) = light.direction_from(point);

        // ley del coseno: la superficie recibe menos luz cuanto mas de canto
        // le llega. De espaldas a la luz (producto punto <= 0) no recibe nada.
        let lambert = normal.dot(to_light);
        if lambert <= 0.0 {
            continue;
        }

        // el orden importa: el coseno es una multiplicacion y el rayo de
        // sombra es un recorrido del BVH, asi que primero se descarta barato
        if in_shadow(scene, point, normal, to_light, distance) {
            continue;
        }

        diffuse = diffuse + light.color * (light.intensity * lambert);

        if material.albedo.specular <= 0.0 {
            continue;
        }

        // Phong: el brillo aparece donde el rebote de la luz apunta al ojo.
        // El exponente controla que tan cerrado queda ese brillo.
        let bounce = reflect(-to_light, normal).dot(view_dir);
        if bounce > 0.0 {
            let highlight = bounce.powf(material.shininess) * light.intensity;
            specular = specular + light.color * highlight;
        }
    }

    // albedo: cuanta luz sale por cada camino en vez de absorberse
    base * diffuse * material.albedo.diffuse + specular * material.albedo.specular
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
    use crate::material::{Albedo, GLASS_INDEX};
    use crate::object::Object;
    use crate::scene::Scene;

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
        shade(scene, material, point, normal, view_dir).to_color()
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

    fn with_blocker(mut scene: Scene, center: Vector3, radius: f32) -> Scene {
        scene.objects = vec![Object::sphere(
            center,
            radius,
            Material::matte(Color::new(0, 0, 0, 255)),
        )];
        scene.bvh = Bvh::build(&mut scene.objects);
        scene
    }

    /// Una esfera entre el punto y la luz lo deja en sombra: solo le queda
    /// la ambiente.
    #[test]
    fn blocked_light_leaves_only_ambient() {
        let lit_scene = white_light_scene(0.1);
        let shadowed = with_blocker(white_light_scene(0.1), Vector3::new(0.0, -2.0, 0.0), 0.5);

        let material = white_matte(1.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);

        let lit = shade_color(&lit_scene, &material, Vector3::zero(), normal, VIEW);
        let dark = shade_color(&shadowed, &material, Vector3::zero(), normal, VIEW);

        assert!(dark.r < lit.r);
        assert_eq!(dark.r, (0.1 * 255.0) as u8, "en sombra solo queda la ambiente");
    }

    /// Lo que esta por detras de la luz no hace sombra.
    #[test]
    fn blockers_behind_the_light_do_not_cast_shadow() {
        let scene = with_blocker(white_light_scene(0.0), Vector3::new(0.0, -9.0, 0.0), 0.5);

        let lit = shade_color(
            &scene,
            &white_matte(1.0),
            Vector3::zero(),
            Vector3::new(0.0, -1.0, 0.0),
            VIEW,
        );

        assert_eq!(lit.r, 255);
    }

    /// El punto no se tapa a si mismo: la esfera sobre la que se sombrea
    /// tambien esta en el BVH y el rayo de sombra arranca sobre su superficie.
    #[test]
    fn surface_does_not_shadow_itself() {
        let scene = with_blocker(white_light_scene(0.0), Vector3::new(0.0, 0.0, 0.0), 1.0);

        // punto en el polo de la esfera, del lado que mira a la luz
        let point = Vector3::new(0.0, -1.0, 0.0);
        let normal = Vector3::new(0.0, -1.0, 0.0);
        let lit = shade_color(&scene, &white_matte(1.0), point, normal, VIEW);

        assert_eq!(lit.r, 255, "la esfera no deberia sombrearse a si misma");
    }

    /// Geometria de los tests de reflejo: un rayo que viaja hacia +Z pega en
    /// el costado de la esfera del medio, y el rebote sale hacia -X, justo
    /// donde esta la esfera roja.
    const MIRROR_CENTER: Vector3 = Vector3 { x: 0.0, y: 0.0, z: -10.0 };
    const RAY_ORIGIN: Vector3 = Vector3 { x: -0.707, y: 0.0, z: -14.0 };
    const RAY_DIR: Vector3 = Vector3 { x: 0.0, y: 0.0, z: 1.0 };

    fn mirror_test_scene(center_material: Material) -> Scene {
        let mut scene = white_light_scene(0.2);
        scene.objects = vec![
            // la que se refleja, fuera del camino del rayo directo
            Object::sphere(
                Vector3::new(-6.0, 0.0, -10.707),
                2.0,
                Material::matte(Color::new(255, 0, 0, 255)),
            ),
            Object::sphere(MIRROR_CENTER, 1.0, center_material),
        ];
        scene.bvh = Bvh::build(&mut scene.objects);
        scene
    }

    /// Un espejo tiene que traer el color de lo que tiene enfrente.
    #[test]
    fn mirror_picks_up_the_sphere_in_front_of_it() {
        let gray = Color::new(120, 120, 120, 255);
        let matte_scene = mirror_test_scene(Material::matte(gray));
        let mirror_scene = mirror_test_scene(Material::mirror(gray));

        let matte = trace(&matte_scene, RAY_ORIGIN, RAY_DIR, MAX_DEPTH).to_color();
        let mirror = trace(&mirror_scene, RAY_ORIGIN, RAY_DIR, MAX_DEPTH).to_color();

        // el gris mate tiene los tres canales parejos; el espejo tiene que
        // inclinarse al rojo de la esfera que refleja
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
            Object::sphere(Vector3::new(-3.0, 0.0, -10.0), 1.0, mirror),
            Object::sphere(Vector3::new(3.0, 0.0, -10.0), 1.0, mirror),
        ];
        scene.bvh = Bvh::build(&mut scene.objects);

        let origin = Vector3::new(-3.0, 0.0, -4.0);
        let direction = Vector3::new(0.0, 0.0, -1.0);

        // si la recursion no cortara, esto no volveria nunca
        let color = trace(&scene, origin, direction, MAX_DEPTH).to_color();
        assert!(color.r > 0);
    }

    /// De frente, el rayo cruza el vidrio sin doblarse.
    #[test]
    fn head_on_rays_pass_straight_through() {
        let incident = Vector3::new(0.0, 0.0, -1.0);
        let normal = Vector3::new(0.0, 0.0, 1.0);

        let through = refract(incident, normal, GLASS_INDEX).expect("tenia que pasar");

        assert!((through - incident).length() < 1e-5);
    }

    /// Al entrar en un medio mas denso el rayo se acerca a la normal.
    #[test]
    fn entering_glass_bends_toward_the_normal() {
        let incident = Vector3::new(0.7071, 0.0, -0.7071);
        let normal = Vector3::new(0.0, 0.0, 1.0);

        let through = refract(incident, normal, GLASS_INDEX)
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

        assert!(refract(incident, normal, GLASS_INDEX).is_none());
    }

    /// Una esfera de vidrio deja ver lo que tiene detras; una mate no.
    #[test]
    fn glass_lets_the_sphere_behind_show_through() {
        let red = Object::sphere(
            Vector3::new(0.0, 0.0, -20.0),
            6.0,
            Material::matte(Color::new(255, 0, 0, 255)),
        );
        let tint = Color::new(180, 180, 180, 255);

        let mut glass_scene = white_light_scene(0.2);
        glass_scene.objects = vec![
            red,
            Object::sphere(Vector3::new(0.0, 0.0, -10.0), 1.0, Material::glass(tint)),
        ];
        glass_scene.bvh = Bvh::build(&mut glass_scene.objects);

        let mut solid_scene = white_light_scene(0.2);
        solid_scene.objects = vec![
            red,
            Object::sphere(Vector3::new(0.0, 0.0, -10.0), 1.0, Material::matte(tint)),
        ];
        solid_scene.bvh = Bvh::build(&mut solid_scene.objects);

        let origin = Vector3::new(0.0, 0.0, -4.0);
        let direction = Vector3::new(0.0, 0.0, -1.0);

        let glass = trace(&glass_scene, origin, direction, MAX_DEPTH).to_color();
        let solid = trace(&solid_scene, origin, direction, MAX_DEPTH).to_color();

        assert!(
            glass.r as i32 - glass.b as i32 > solid.r as i32 - solid.b as i32,
            "por el vidrio tendria que verse el rojo de atras: {glass:?} vs {solid:?}"
        );
    }
}
