// flight.rs
// Vuelo en tercera persona: W acelera, el mouse apunta y la camara sigue a
// la nave desde atras.
//
// El rasterizador es ortografico: un objeto se ve del mismo tamano este
// cerca o lejos. Para que la nave se acerque a los planetas, cada objeto
// lleva su propia matriz de proyeccion que lo agranda por focal / distancia,
// con la distancia medida desde la camara hasta su centro. Es perspectiva
// "debil": todo el objeto usa la distancia de su centro. En una esfera casi
// no se distingue de la perspectiva completa, y el rasterizador sigue sin
// tener que recortar triangulos contra el plano cercano ni corregir la
// interpolacion.
use raylib::prelude::*;
use std::f32::consts::TAU;
use std::sync::LazyLock;

use crate::color::Rgb;
use crate::framebuffer::{star_hash, Framebuffer};
use crate::light::Light;
use crate::matrix::{create_model_matrix, look_at, multiply, new_matrix4};
use crate::shaders::kit::sphere_mesh;
use crate::shaders::{ship, transform, Normals};
use crate::ships::Ship;
use crate::solar_system::{self, BodyPose};
use crate::vertex::Vertex;
use crate::{fit_unit_sphere, render_objects, SceneObject, Uniforms, HEIGHT, WIDTH};

/// Campo de vision vertical, unos 60 grados.
const FOV: f32 = 1.05;
/// Lo que queda mas cerca de la camara que esto no se dibuja.
const NEAR: f32 = 0.02;

const MAX_SPEED: f32 = 4.0;
const ACCELERATION: f32 = 3.0;
/// Que tan rapido frena la nave al soltar W.
const DRAG: f32 = 1.2;
/// Radianes por segundo con el mouse en el borde de la pantalla.
const TURN_RATE: f32 = 1.6;
/// Alrededor del centro de la pantalla el mouse no gira la nave.
const DEAD_ZONE: f32 = 0.08;
const MAX_PITCH: f32 = 1.3;
/// Cuanto se ladea la nave al girar, y que tan rapido.
const MAX_ROLL: f32 = 0.6;
const ROLL_STIFFNESS: f32 = 4.0;

/// Radio de la nave. Las estrellas y los planetas miden entre 16 y 23
/// veces esto.
const SHIP_SIZE: f32 = 0.035;
/// Lo mas cerca que la nave puede pasar de la superficie de un cuerpo.
const CLEARANCE: f32 = 0.06;
/// Las lunas y anillos de un planeta llegan hasta aca, en radios del planeta.
const BODY_EXTENT: f32 = 3.0;

const CHASE_DISTANCE: f32 = 0.21;
const CHASE_HEIGHT: f32 = 0.085;
/// La camara mira a un punto delante de la nave: asi la nave queda en la
/// parte de abajo de la pantalla y se ve hacia donde va.
const LOOK_AHEAD: f32 = 0.4;
/// Que tan rapido alcanza la camara su lugar detras de la nave.
const CHASE_STIFFNESS: f32 = 5.0;

const START: Vector3 = Vector3::new(0.0, 1.6, 19.0);
const START_PITCH: f32 = -0.08;

/// Radio en pixeles a partir del cual un cuerpo se dibuja con la esfera fina.
/// Por debajo alcanza la del OBJ: las facetas no llegan a verse.
const DETAIL_RADIUS: f32 = 90.0;

const SKY_STARS: usize = 5000;
const ORBIT_SEGMENTS: usize = 240;
const ORBIT_COLOR: Color = Color::new(46, 58, 76, 255);
const RETICLE_COLOR: Color = Color::new(110, 220, 255, 255);
/// Radianes por segundo que giran los arcos de la mira.
const RETICLE_SPIN: f32 = 0.9;

/// Esfera con mas triangulos para los cuerpos que se ven grandes.
static FINE_SPHERE: LazyLock<Vec<Vertex>> = LazyLock::new(|| sphere_mesh(48, 32));

/// Las estrellas de fondo son direcciones y no puntos: estan tan lejos que
/// la nave nunca se les acerca, solo giran con la camara.
static SKY: LazyLock<Vec<(Vector3, Color)>> = LazyLock::new(|| {
    let unit = |hash: u32| hash as f32 / u32::MAX as f32;
    (0..SKY_STARS)
        .map(|index| {
            let height = unit(star_hash(index * 3)) * 2.0 - 1.0;
            let angle = unit(star_hash(index * 3 + 1)) * TAU;
            let ring = (1.0 - height * height).sqrt();
            let brightness = 85 + (star_hash(index * 3 + 2) % 150) as u8;
            (
                Vector3::new(ring * angle.cos(), height, ring * angle.sin()),
                Color::new(brightness - 8, brightness - 3, brightness, 255),
            )
        })
        .collect()
});

/// Lo que se lee del teclado y el mouse en un cuadro.
pub struct Controls {
    pub thrust: bool,
    /// El mouse respecto del centro de la pantalla, de -1 a 1 en cada eje
    /// (y positivo hacia abajo).
    pub aim: Vector2,
}

impl Controls {
    /// Sin acelerar ni girar: la nave sigue derecho y se va frenando.
    pub fn idle() -> Self {
        Controls { thrust: false, aim: Vector2::zero() }
    }

    pub fn read(window: &RaylibHandle) -> Self {
        let mouse = window.get_mouse_position();
        // con la ventana en segundo plano el mouse anda por otro lado
        let aim = if window.is_window_focused() {
            Vector2::new(
                (mouse.x / WIDTH as f32 * 2.0 - 1.0).clamp(-1.0, 1.0),
                (mouse.y / HEIGHT as f32 * 2.0 - 1.0).clamp(-1.0, 1.0),
            )
        } else {
            Vector2::zero()
        };
        Controls { thrust: window.is_key_down(KeyboardKey::KEY_W), aim }
    }
}

pub struct Flight {
    pub ship: usize,
    position: Vector3,
    yaw: f32,
    pitch: f32,
    roll: f32,
    speed: f32,
    aim: Vector2,
    /// Donde esta la camara: persigue su lugar detras de la nave con un poco
    /// de retraso, que es lo que hace que la nave se vea girar.
    eye: Vector3,
    time: f32,
    stars: Vec<BodyPose>,
    planets: Vec<BodyPose>,
}

impl Flight {
    pub fn new(ship: usize) -> Self {
        let mut flight = Flight {
            ship,
            position: START,
            yaw: 0.0,
            pitch: START_PITCH,
            roll: 0.0,
            speed: 0.0,
            aim: Vector2::zero(),
            eye: START,
            time: 0.0,
            stars: solar_system::star_poses(0.0),
            planets: solar_system::planet_poses(0.0),
        };
        flight.eye = flight.chase_point();
        flight
    }

    pub fn update(&mut self, controls: &Controls, dt: f32) {
        // si la ventana se trabo (por ejemplo al arrastrarla), mejor un salto
        // chico que atravesar medio sistema solar en un cuadro
        let dt = dt.min(0.1);
        self.time += dt;
        self.aim = controls.aim;

        let (turn_x, turn_y) = (steer(controls.aim.x), steer(controls.aim.y));
        self.yaw += turn_x * TURN_RATE * dt;
        self.pitch = (self.pitch - turn_y * TURN_RATE * dt).clamp(-MAX_PITCH, MAX_PITCH);
        // al girar a la derecha baja el ala derecha
        self.roll += (-turn_x * MAX_ROLL - self.roll) * follow(ROLL_STIFFNESS, dt);

        if controls.thrust {
            self.speed = (self.speed + ACCELERATION * dt).min(MAX_SPEED);
        } else {
            self.speed *= (-DRAG * dt).exp();
        }
        let (_, _, forward) = self.basis();
        self.position += forward * (self.speed * dt);

        self.stars = solar_system::star_poses(self.time);
        self.planets = solar_system::planet_poses(self.time);
        self.position = self.keep_clear(self.position, CLEARANCE);

        let target = self.chase_point();
        self.eye = self.keep_clear(self.eye + (target - self.eye) * follow(CHASE_STIFFNESS, dt), NEAR);
    }

    pub fn time(&self) -> f32 {
        self.time
    }

    pub fn position(&self) -> Vector3 {
        self.position
    }

    pub fn render(
        &self,
        framebuffer: &mut Framebuffer,
        uniforms: &mut Uniforms,
        ship: &Ship,
        sphere: &[Vertex],
        show_orbits: bool,
    ) {
        let (_, up, forward) = self.basis();
        let target = self.position + forward * LOOK_AHEAD;
        let view = look_at(self.eye, target, up);
        uniforms.view_matrix = view.clone();
        // sin escala: el tamano en pantalla lo pone la proyeccion de cada objeto
        uniforms.viewport_matrix = new_matrix4([
            [1.0, 0.0, 0.0, WIDTH as f32 / 2.0],
            [0.0, -1.0, 0.0, HEIGHT as f32 / 2.0],
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        uniforms.to_viewer = (self.eye - target).normalized();
        uniforms.time = self.time;
        // Una sola luz puntual en el centro de la orbita de las dos estrellas.
        uniforms.sun = Light::new(Vector3::zero(), Rgb::new(1.0, 0.92, 0.78), 1.4);

        // nivel de detalle: la esfera del OBJ de lejos y la fina de cerca
        let coarse = (sphere, fit_unit_sphere(sphere));
        let fine = (FINE_SPHERE.as_slice(), fit_unit_sphere(&FINE_SPHERE));
        let mut objects = Vec::new();
        for body in self.stars.iter().chain(&self.planets) {
            if (self.eye - body.center).length() < body.radius {
                continue;
            }
            let Some(at) = project(&view, body.center).filter(|at| at.on_screen(body.radius * BODY_EXTENT)) else {
                continue;
            };
            let (mesh, unit_sphere) = if body.radius * at.scale > DETAIL_RADIUS { &fine } else { &coarse };
            let model_matrix = multiply(&body.model_matrix, unit_sphere);
            let to_viewer = Some((self.eye - body.center).normalized());
            objects.push(SceneObject {
                projection: at.matrix(),
                to_viewer,
                ..SceneObject::new(mesh, model_matrix, body.shader)
            });
            // lunas y anillos se ven como su planeta, para no separarse de el
            for accessory in &body.accessories {
                let model_matrix = accessory.model_matrix.clone();
                objects.push(SceneObject {
                    projection: at.matrix(),
                    to_viewer,
                    ..SceneObject::new(accessory.vertices, model_matrix, accessory.shade)
                });
            }
        }
        let ship_at = project(&view, self.position);
        if let Some(at) = &ship_at {
            objects.push(SceneObject {
                normals: Normals::Mesh,
                projection: at.matrix(),
                to_viewer: Some((self.eye - self.position).normalized()),
                ..SceneObject::new(&ship.vertices, self.ship_matrix(ship), ship::shade)
            });
        }

        framebuffer.clear();
        draw_sky(framebuffer, &view);
        render_objects(framebuffer, uniforms, &objects);
        if show_orbits {
            draw_orbits(framebuffer, &view);
        }

        for (index, star) in self.stars.iter().enumerate() {
            let Some(at) = project(&view, star.center) else { continue };
            let glow = if index == 0 { Color::new(135, 190, 255, 255) } else { Color::new(90, 205, 255, 255) };
            framebuffer.add_glow(at.x as i32, at.y as i32, at.scale * 1.5, glow, 0.75);
        }
        // fuego del motor, mas grande cuanto mas rapido va
        if let Some(at) = project(&view, self.position - forward * (SHIP_SIZE * 0.6)) {
            let power = self.speed / MAX_SPEED;
            if power > 0.02 {
                let radius = at.scale * SHIP_SIZE * (0.2 + 0.3 * power);
                framebuffer.add_glow(at.x as i32, at.y as i32, radius, Color::new(120, 180, 255, 255), 0.2 + 0.4 * power);
            }
        }

        // un punto tenue en el centro (por donde se sigue derecho) y la mira
        // donde esta el mouse, que reemplaza al puntero
        let (center_x, center_y) = (WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0);
        framebuffer.ring(center_x, center_y, 0.0, 3.0, RETICLE_COLOR, 0.5, |_| true);
        let aim_x = center_x + self.aim.x * WIDTH as f32 / 2.0;
        let aim_y = center_y + self.aim.y * HEIGHT as f32 / 2.0;
        draw_reticle(framebuffer, aim_x, aim_y, self.time);
    }

    pub fn overlay(&self, d: &mut RaylibDrawHandle, ship: &Ship) {
        d.draw_text(&ship.name, 18, 16, 22, Color::WHITE);
        let speed = format!("velocidad {:.1} / {MAX_SPEED:.0}", self.speed);
        d.draw_text(&speed, 18, 44, 16, Color::LIGHTGRAY);
        let nearest = self.stars.iter().chain(&self.planets).min_by(|a, b| {
            self.surface_distance(a).total_cmp(&self.surface_distance(b))
        });
        if let Some(body) = nearest {
            let text = format!("cerca de {} ({:.1})", body.name, self.surface_distance(body));
            d.draw_text(&text, 18, 64, 16, Color::LIGHTGRAY);
        }
        d.draw_text("W: avanzar | mouse: apuntar | Tab: orbitas / sistema solar", 18, HEIGHT - 52, 16, Color::LIGHTGRAY);
        d.draw_text("R: reiniciar | Backspace: cambiar nave", 18, HEIGHT - 30, 16, Color::LIGHTGRAY);
    }

    /// Derecha, arriba y adelante de la nave, sin contar el ladeo.
    fn basis(&self) -> (Vector3, Vector3, Vector3) {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let forward = Vector3::new(sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch);
        let right = Vector3::new(cos_yaw, 0.0, sin_yaw);
        (right, right.cross(forward), forward)
    }

    /// Detras y un poco arriba de la nave: donde quiere estar la camara.
    fn chase_point(&self) -> Vector3 {
        let (_, up, forward) = self.basis();
        self.position - forward * CHASE_DISTANCE + up * CHASE_HEIGHT
    }

    fn ship_matrix(&self, ship: &Ship) -> Matrix {
        let (right, up, forward) = self.basis();
        let back = -forward;
        // cada columna dice a donde va un eje de la nave: x a la derecha, y
        // arriba y z atras, porque la nariz mira hacia -z
        let placement = new_matrix4([
            [right.x, up.x, back.x, self.position.x],
            [right.y, up.y, back.y, self.position.y],
            [right.z, up.z, back.z, self.position.z],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let size_and_roll = create_model_matrix(Vector3::zero(), SHIP_SIZE, Vector3::new(0.0, 0.0, self.roll));
        multiply(&placement, &multiply(&size_and_roll, &ship.fit))
    }

    /// Empuja `point` fuera de las estrellas y los planetas, dejando al menos
    /// `clearance` de la superficie.
    fn keep_clear(&self, mut point: Vector3, clearance: f32) -> Vector3 {
        for body in self.stars.iter().chain(&self.planets) {
            let offset = point - body.center;
            let minimum = body.radius + clearance;
            let distance = offset.length();
            if distance < minimum && distance > f32::EPSILON {
                point = body.center + offset * (minimum / distance);
            }
        }
        point
    }

    fn surface_distance(&self, body: &BodyPose) -> f32 {
        (self.position - body.center).length() - body.radius
    }
}

/// Un punto visto por la camara: donde cae en pantalla y cuanto se agranda
/// un objeto centrado ahi.
struct Projected {
    /// El punto en coordenadas de la camara.
    center: Vector3,
    x: f32,
    y: f32,
    /// focal / distancia: pixeles por unidad del mundo a esa distancia.
    scale: f32,
}

impl Projected {
    /// La proyeccion del objeto, en el espacio de la camara. Primero lo gira
    /// para mirarlo por la linea que va de la camara a su centro: sin ese giro
    /// todo se veria en la direccion a la que apunta la camara, y un anillo
    /// cambiaria de forma con solo girar la camara en el lugar. Despues lo
    /// agranda por focal / distancia, asi que al acercarse crecen tanto su
    /// tamano como su separacion del centro de la pantalla, y lo deja a su
    /// distancia real para que la prueba de profundidad siga ordenando bien.
    fn matrix(&self) -> Matrix {
        let (c, s) = (self.center, self.scale);
        // ejes de esa linea de vista; `back` va del objeto hacia la camara y
        // nunca es vertical porque el objeto esta delante de la camara
        let back = (-c).normalized();
        let right = Vector3::new(0.0, 1.0, 0.0).cross(back).normalized();
        let up = back.cross(right);
        new_matrix4([
            [s * right.x, s * right.y, s * right.z, s * (c.x - right.dot(c))],
            [s * up.x, s * up.y, s * up.z, s * (c.y - up.dot(c))],
            [back.x, back.y, back.z, c.z - back.dot(c)],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    /// Si un circulo de `radius` unidades alrededor del punto toca la
    /// pantalla. Lo que cae entero afuera no se transforma ni se rasteriza.
    fn on_screen(&self, radius: f32) -> bool {
        let r = radius * self.scale;
        self.x + r >= 0.0 && self.x - r < WIDTH as f32 && self.y + r >= 0.0 && self.y - r < HEIGHT as f32
    }
}

/// Mira circular: cuatro arcos que giran despacio, un anillo fino adentro,
/// marcas fijas hacia afuera en los cuatro puntos cardinales y un punto en el
/// centro.
fn draw_reticle(framebuffer: &mut Framebuffer, x: f32, y: f32, time: f32) {
    const QUARTER: f32 = TAU / 4.0;
    let spin = time * RETICLE_SPIN;
    // cada cuarto de vuelta lleva un arco que cubre dos tercios de el
    framebuffer.ring(x, y, 15.0, 1.8, RETICLE_COLOR, 0.95, |angle| {
        (angle - spin).rem_euclid(QUARTER) < QUARTER * 0.66
    });
    framebuffer.ring(x, y, 8.0, 1.0, RETICLE_COLOR, 0.45, |_| true);
    framebuffer.ring(x, y, 21.5, 4.0, RETICLE_COLOR, 0.8, |angle| {
        let offset = angle.rem_euclid(QUARTER);
        offset < 0.06 || offset > QUARTER - 0.06
    });
    framebuffer.ring(x, y, 0.0, 3.2, RETICLE_COLOR, 1.0, |_| true);
}

/// None si el punto queda detras de la camara.
fn project(view: &Matrix, point: Vector3) -> Option<Projected> {
    let at = transform(view, point, 1.0);
    let distance = -at.z;
    if distance < NEAR {
        return None;
    }
    let scale = focal() / distance;
    Some(Projected {
        center: at,
        x: WIDTH as f32 / 2.0 + at.x * scale,
        y: HEIGHT as f32 / 2.0 - at.y * scale,
        scale,
    })
}

/// Pixeles que mide una unidad del mundo a una unidad de distancia.
fn focal() -> f32 {
    HEIGHT as f32 / 2.0 / (FOV / 2.0).tan()
}

fn draw_sky(framebuffer: &mut Framebuffer, view: &Matrix) {
    for (direction, color) in SKY.iter() {
        // w = 0: una direccion gira con la camara pero no se traslada
        let at = transform(view, *direction, 0.0);
        if at.z >= 0.0 {
            continue;
        }
        let scale = focal() / -at.z;
        let x = WIDTH as f32 / 2.0 + at.x * scale;
        let y = HEIGHT as f32 / 2.0 - at.y * scale;
        framebuffer.plot(x as i32, y as i32, *color);
    }
}

/// Las orbitas son lineas: cada punto se proyecta con la misma cuenta que el
/// centro de un planeta, solo que punto por punto.
fn draw_orbits(framebuffer: &mut Framebuffer, view: &Matrix) {
    for radius in solar_system::orbit_radii() {
        let point = |step: usize| {
            let (sin, cos) = (TAU * step as f32 / ORBIT_SEGMENTS as f32).sin_cos();
            transform(view, Vector3::new(radius * cos, 0.0, radius * sin), 1.0)
        };
        for step in 0..ORBIT_SEGMENTS {
            if let Some((from, to)) = clip_near(point(step), point(step + 1)) {
                framebuffer.line(to_screen(from), to_screen(to), ORBIT_COLOR);
            }
        }
    }
}

/// Recorta un segmento en coordenadas de la camara contra el plano cercano.
fn clip_near(a: Vector3, b: Vector3) -> Option<(Vector3, Vector3)> {
    // cuanto queda cada punto delante del plano cercano
    let (ahead_a, ahead_b) = (-a.z - NEAR, -b.z - NEAR);
    if ahead_a < 0.0 && ahead_b < 0.0 {
        return None;
    }
    if ahead_a >= 0.0 && ahead_b >= 0.0 {
        return Some((a, b));
    }
    let cut = a + (b - a) * (ahead_a / (ahead_a - ahead_b));
    Some(if ahead_a < 0.0 { (cut, b) } else { (a, cut) })
}

/// De coordenadas de la camara a pixeles, con la distancia como profundidad.
fn to_screen(point: Vector3) -> Vector3 {
    let distance = -point.z;
    let scale = focal() / distance;
    Vector3::new(
        WIDTH as f32 / 2.0 + point.x * scale,
        HEIGHT as f32 / 2.0 - point.y * scale,
        distance,
    )
}

/// El mouse fuera de la zona muerta, llevado de nuevo a 0..1.
fn steer(value: f32) -> f32 {
    let past = (value.abs() - DEAD_ZONE).max(0.0) / (1.0 - DEAD_ZONE);
    past.min(1.0) * value.signum()
}

/// Fraccion del camino que recorre en `dt` algo que persigue a su objetivo
/// con esa rigidez; no depende de los cuadros por segundo.
fn follow(stiffness: f32, dt: f32) -> f32 {
    1.0 - (-stiffness * dt).exp()
}
