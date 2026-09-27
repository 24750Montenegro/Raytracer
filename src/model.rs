// model.rs
// Modelos de bloque del pack Classic 3D: el relieve deja de ser un dibujo
// sobre una cara plana y pasa a ser geometria.
//
// El pack no trae texturas propias: lo que trae son mil y pico de modelos
// JSON que reemplazan el cubo liso de cada bloque por una caja de 16x16x16
// con decenas de cajitas pegadas por fuera de cada cara. Un adoquin deja de
// ser un cuadrado pintado y pasa a tener piedras que sobresalen medio texel,
// y eso es lo que le rompe la silueta: el borde del bloque deja de ser una
// linea recta. Eso es lo que el mapa de alturas nunca pudo dar, porque el
// paralaje solo hunde hacia adentro y la silueta del cubo queda intacta.
//
// Aca cada cajita del modelo se convierte en un cubo mas de la escena. Sale
// caro, asi que solo se generan las de las caras que dan al aire: la
// decoracion de la cara de abajo de un bloque enterrado no la ve nadie.
//
// Los modelos viven en assets/models, copiados del pack tal como vienen.
use raylib::prelude::*;

use crate::json::Json;

/// Lado de un bloque en las coordenadas del modelo. Minecraft las cuenta en
/// dieciseisavos, uno por texel.
const MODEL_SIZE: f32 = 16.0;

/// Donde estan los modelos copiados del pack, relativo a la raiz.
const MODELS: &str = "assets/models";

/// Cuanto tiene que sobresalir una cajita para contarla como decoracion de
/// esa cara. Las del pack sobresalen medio texel; este umbral es diez veces
/// mas chico, y solo esta para no confundir el ruido de coma flotante del
/// cubo del cuerpo (que va de 0 a 16 exacto) con una protuberancia.
const STICKS_OUT: f32 = 0.003;

/// Las seis caras de un bloque, en coordenadas del mundo, donde +Y va hacia
/// abajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Up,
    Down,
    East,
    West,
    North,
    South,
}

impl Face {
    pub const ALL: [Face; 6] = [
        Face::Up,
        Face::Down,
        Face::East,
        Face::West,
        Face::North,
        Face::South,
    ];

    /// Hacia donde mira, en pasos de la grilla del terreno: (x, nivel, z).
    /// El nivel crece hacia arriba en el terreno, al reves que el eje Y del
    /// mundo, asi que Up suma un nivel.
    pub fn step(self) -> (i32, i32, i32) {
        match self {
            Face::Up => (0, 1, 0),
            Face::Down => (0, -1, 0),
            Face::East => (1, 0, 0),
            Face::West => (-1, 0, 0),
            Face::South => (0, 0, 1),
            Face::North => (0, 0, -1),
        }
    }

    fn bit(self) -> u8 {
        1 << match self {
            Face::Up => 0,
            Face::Down => 1,
            Face::East => 2,
            Face::West => 3,
            Face::North => 4,
            Face::South => 5,
        }
    }
}

/// Que caras de un bloque dan al aire.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Faces(u8);

impl Faces {
    pub const NONE: Faces = Faces(0);
    pub const ALL: Faces = Faces(0b11_1111);

    pub fn with(self, face: Face) -> Self {
        Faces(self.0 | face.bit())
    }

    pub fn has(self, face: Face) -> bool {
        self.0 & face.bit() != 0
    }

    pub fn any(self) -> bool {
        self.0 != 0
    }
}

/// Una cajita del modelo, ya en coordenadas locales del bloque: de 0 a 1 de
/// lado a lado, y lo que sobresale se va un poco fuera de ese rango.
#[derive(Debug, Clone, Copy)]
pub struct Element {
    pub min: Vector3,
    pub max: Vector3,
    /// De que cara cuelga, o None si es el cuerpo del bloque.
    pub face: Option<Face>,
}

pub struct BlockModel {
    elements: Vec<Element>,
}

impl BlockModel {
    /// El cubo liso de siempre: un solo elemento, sin decoracion.
    pub fn plain() -> Self {
        BlockModel {
            elements: vec![Element {
                min: Vector3::zero(),
                max: Vector3::new(1.0, 1.0, 1.0),
                face: None,
            }],
        }
    }

    /// Lee `assets/models/<nombre>.json`, tal como sale del pack.
    pub fn load(name: &str) -> Result<Self, String> {
        let path = format!("{MODELS}/{name}.json");
        let source = std::fs::read_to_string(&path).map_err(|problem| format!("{path}: {problem}"))?;
        let model = Json::parse(&source).map_err(|problem| format!("{path}: {problem}"))?;

        let raw = model
            .get("elements")
            .and_then(Json::list)
            .ok_or_else(|| format!("{path}: no trae elements"))?;

        let elements: Vec<Element> = raw.iter().filter_map(element).collect();
        if elements.is_empty() {
            return Err(format!("{path}: no trae ninguna caja entera"));
        }

        Ok(BlockModel { elements })
    }

    /// Las cajitas que hay que dibujar si esas son las caras que dan al
    /// aire: el cuerpo del bloque siempre, y la decoracion solo de las caras
    /// expuestas.
    pub fn visible(&self, exposed: Faces) -> impl Iterator<Item = &Element> {
        self.elements
            .iter()
            .filter(move |element| element.face.map_or(true, |face| exposed.has(face)))
    }
}

/// Convierte una caja del modelo a coordenadas locales del bloque.
///
/// El modelo viene en el sistema de Minecraft, con Y hacia arriba; aca +Y va
/// hacia abajo, asi que la altura se da vuelta y con ella el minimo y el
/// maximo de ese eje.
///
/// La rotacion de los elementos se ignora a proposito: la traen 18 de las
/// 253 cajitas del adoquin, miden medio texel y el raytracer solo sabe
/// trazar cajas alineadas a los ejes. Girarlas seria otro tipo de cuerpo
/// entero para una diferencia que a esta escala no se ve.
fn element(raw: &Json) -> Option<Element> {
    let from = corner(raw.get("from")?)?;
    let to = corner(raw.get("to")?)?;

    let min = Vector3::new(
        from.x.min(to.x),
        (1.0 - from.y).min(1.0 - to.y),
        from.z.min(to.z),
    );
    let max = Vector3::new(
        from.x.max(to.x),
        (1.0 - from.y).max(1.0 - to.y),
        from.z.max(to.z),
    );

    Some(Element {
        min,
        max,
        face: hangs_from(min, max),
    })
}

fn corner(raw: &Json) -> Option<Vector3> {
    let axes = raw.list()?;
    if axes.len() != 3 {
        return None;
    }

    Some(Vector3::new(
        axes[0].number()? as f32 / MODEL_SIZE,
        axes[1].number()? as f32 / MODEL_SIZE,
        axes[2].number()? as f32 / MODEL_SIZE,
    ))
}

/// De que cara cuelga una cajita: de aquella por la que se sale del bloque.
///
/// Si no sobresale por ninguna es el cuerpo del bloque, y va siempre. Si
/// sobresale por varias, porque cae en una arista, manda la que mas asoma.
fn hangs_from(min: Vector3, max: Vector3) -> Option<Face> {
    let mut best: Option<(Face, f32)> = None;

    for (face, stick_out) in [
        (Face::Up, -min.y),
        (Face::Down, max.y - 1.0),
        (Face::West, -min.x),
        (Face::East, max.x - 1.0),
        (Face::North, -min.z),
        (Face::South, max.z - 1.0),
    ] {
        if stick_out > STICKS_OUT && best.map_or(true, |(_, most)| stick_out > most) {
            best = Some((face, stick_out));
        }
    }

    best.map(|(face, _)| face)
}
