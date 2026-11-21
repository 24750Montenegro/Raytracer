// Cascos de las naves. Los patrones salen de la posicion en el modelo (las
// coordenadas del OBJ), asi que quedan pegados a la nave cuando gira.
use std::f32::consts::TAU;

use super::kit::*;

/// Luz de relleno que sale de la camara: sin ella la cara que no ve a las
/// estrellas queda negra, y en vuelo esa es justo la que se tiene enfrente.
const FILL: f32 = 0.22;

fn fill(f: &Frag, base: Vector3) -> Vector3 {
    base * (f.normal.dot(f.to_viewer).max(0.0) * FILL)
}

// ---------- generico, para naves sin casco propio ----------

const HULL: Vector3 = Vector3::new(0.74, 0.76, 0.82);
const RIM: Vector3 = Vector3::new(0.25, 0.48, 0.95);

/// Metal claro laqueado, con un filo azul en el borde para que la silueta se
/// lea contra el fondo negro.
pub fn shade(f: &Frag) -> Vector3 {
    f.shade(HULL, &Material::lacquer(), f.normal) + fill(f, HULL) + RIM * (rim(f, 3.0) * 0.55)
}

// ---------- juanmontenegro: paneles negros, uniones rojas y tubos plateados ----------

/// El recuadro de los tubos, en la cara de atras de la nave (donde va el
/// motor), es lo unico plateado. Medidas en coordenadas del OBJ.
const TUBES_HALF_WIDTH: f32 = 0.47;
const TUBES_BOTTOM: f32 = -0.25;
const TUBES_TOP: f32 = 0.64;
/// La cara de atras empieza aca en z.
const TUBES_FACE: f32 = 0.8;
/// Ancho de la linea roja que enmarca el recuadro.
const FRAME_WIDTH: f32 = 0.03;
/// Paneles por unidad del OBJ.
const PANEL_SCALE: f32 = 1.0;
/// Ancho de las uniones entre paneles, en la escala de `cell_edge`.
const SEAM_WIDTH: f32 = 0.03;
/// Fraccion de paneles que llevan una luz roja en el centro.
const PANEL_LIGHTS: f32 = 0.1;
const SEAM_GLOW: f32 = 1.5;

const BLACK_PANEL: Vector3 = Vector3::new(0.05, 0.05, 0.06);
const SILVER: Vector3 = Vector3::new(0.86, 0.88, 0.92);
const SEAM_RED: Vector3 = Vector3::new(1.0, 0.07, 0.09);
/// Reflejo del entorno en el borde: es lo que hace que un negro se lea
/// brillante y no mate.
const SHEEN: Vector3 = Vector3::new(0.32, 0.33, 0.38);

pub fn panels(f: &Frag) -> Vector3 {
    let q = f.p * PANEL_SCALE;
    let edge = cell_edge(q);

    // cada panel es una celda de Voronoi con un negro un poco distinto, y se
    // oscurece hacia sus bordes como si estuviera biselado
    let tone = (0.5 + 1.1 * cell_value(q)) * (0.55 + 0.45 * smoothstep(0.03, 0.16, edge));
    let black = BLACK_PANEL * tone;
    let gloss = Material::new(Albedo::new(0.55, 0.5), 200.0);
    let panel = f.shade(black, &gloss, f.normal) + fill(f, black) + SHEEN * rim(f, 3.0);

    // metal pulido con vetas finas, como cepillado
    let brushed = SILVER * (0.85 + 0.15 * fbm(f.p * Vector3::new(18.0, 18.0, 0.8), 2));
    let polished = Material { metallic: true, ..Material::new(Albedo::new(0.6, 0.5), 90.0) };
    let silver = f.shade(brushed, &polished, f.normal) + fill(f, brushed * 1.6) + SHEEN * rim(f, 2.0);

    let tubes = tubes_box(f.p, 0.0);
    let color = mix(panel, silver, tubes);

    // lo que brilla en rojo: las uniones entre paneles (mas encendidas en
    // algunas vetas que en otras), un marco alrededor de los tubos y una luz
    // chica en el centro de algunos paneles
    let streak = 0.25 + 0.75 * smoothstep(0.45, 0.7, noise(f.p * 1.3 + 5.0));
    let seam = (1.0 - smoothstep(SEAM_WIDTH * 0.5, SEAM_WIDTH, edge)) * streak;
    let frame = tubes_box(f.p, FRAME_WIDTH) - tubes;
    let lamp = (1.0 - smoothstep(0.03, 0.05, cellular(q))) * step(1.0 - PANEL_LIGHTS, cell_value(q));
    let pulse = 0.85 + 0.15 * (f.time * 2.2).sin();
    let glow = (seam.max(lamp) * (1.0 - tubes)).max(frame);
    color + SEAM_RED * (glow * SEAM_GLOW * pulse)
}

/// 1 dentro del recuadro de los tubos agrandado en `grow` hacia los lados,
/// 0 afuera, con el borde suavizado.
fn tubes_box(p: Vector3, grow: f32) -> f32 {
    const SOFT: f32 = 0.01;
    let inside = |value: f32, low: f32, high: f32| {
        smoothstep(low - SOFT, low + SOFT, value) * (1.0 - smoothstep(high - SOFT, high + SOFT, value))
    };
    let half_width = TUBES_HALF_WIDTH + grow;
    inside(p.x, -half_width, half_width)
        * inside(p.y, TUBES_BOTTOM - grow, TUBES_TOP + grow)
        * smoothstep(TUBES_FACE - SOFT, TUBES_FACE + SOFT, p.z)
}

// ---------- alejandraaviles: abeja de broche ----------

/// Hasta aca llega el cuerpo a los lados (|x| en el OBJ); mas afuera son alas.
const BODY_HALF_WIDTH: f32 = 0.6;
/// La cabeza va adelante, de aca hacia -z.
const HEAD_END: f32 = -1.15;
/// Franjas negras por unidad del OBJ a lo largo del cuerpo.
const STRIPES: f32 = 1.4;
/// Venas que salen en abanico desde el cuerpo, por media vuelta.
const VEINS: f32 = 9.0;
/// Celdas de las venas transversales por unidad del OBJ.
const CROSS_VEIN_SCALE: f32 = 1.1;

const BEE_YELLOW: Vector3 = Vector3::new(1.0, 0.78, 0.06);
const BEE_BLACK: Vector3 = Vector3::new(0.03, 0.03, 0.035);
const GOLD: Vector3 = Vector3::new(0.95, 0.72, 0.26);
const PEARL: Vector3 = Vector3::new(0.93, 0.95, 1.0);

pub fn bee(f: &Frag) -> Vector3 {
    let enamel = Material::lacquer();
    let gold = f.shade(GOLD, &Material::metal(), f.normal) + fill(f, GOLD * 1.5);

    // cuerpo: cabeza negra adelante y franjas negras y amarillas hacia atras
    let band = fract(f.p.z * STRIPES);
    let yellow = smoothstep(0.40, 0.46, band) * (1.0 - smoothstep(0.94, 1.0, band));
    let head = 1.0 - smoothstep(HEAD_END - 0.03, HEAD_END + 0.03, f.p.z);
    let skin = mix(mix(BEE_BLACK, BEE_YELLOW, yellow), BEE_BLACK, head);
    let mut body = f.shade(skin, &enamel, f.normal) + fill(f, skin);
    // un aro dorado separa la cabeza del cuerpo, como en el broche
    let collar = 1.0 - smoothstep(0.03, 0.06, (f.p.z - HEAD_END).abs());
    body = mix(body, gold, collar);

    // alas de vidrio nacarado. El rasterizador guarda un solo fragmento por
    // pixel, asi que no se puede ver lo que hay detras: la transparencia se
    // imita apagando un poco el ala cuando se la mira de frente y dejandola
    // blanca de canto, donde un vidrio refleja mas
    let facing = f.normal.dot(f.to_viewer).abs();
    let edge_on = (1.0 - facing).powf(2.0);
    // tornasol de pelicula delgada: el color cambia con el angulo de vista
    let phase = facing * 1.4;
    let film = Vector3::new(
        0.5 + 0.5 * (TAU * phase).cos(),
        0.5 + 0.5 * (TAU * (phase + 0.33)).cos(),
        0.5 + 0.5 * (TAU * (phase + 0.67)).cos(),
    );
    let tint = mix(mix(PEARL * 0.6, PEARL, edge_on), film, 0.1);
    let glass_material = Material::new(Albedo::new(0.6, 0.6), 220.0);
    let glass = f.shade(tint, &glass_material, f.normal) + fill(f, tint * 2.0) + PEARL * (edge_on * 0.3);

    // venas doradas: en abanico desde el cuerpo, como en el broche, y unas
    // pocas transversales
    let fan = f.p.z.atan2(f.p.x.abs() - BODY_HALF_WIDTH * 0.5) / std::f32::consts::PI * VEINS;
    let radial = 1.0 - smoothstep(0.04, 0.09, (fract(fan) - 0.5).abs().mul_add(-1.0, 0.5));
    let cross = 1.0 - smoothstep(0.02, 0.04, cell_edge(f.p * CROSS_VEIN_SCALE));
    let wing = mix(glass, gold, radial.max(cross));

    mix(body, wing, smoothstep(BODY_HALF_WIDTH - 0.05, BODY_HALF_WIDTH + 0.05, f.p.x.abs()))
}
