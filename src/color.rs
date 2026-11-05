// color.rs
// Color en punto flotante para hacer cuentas de luz.
//
// Los Color de raylib son u8: sumar dos luces ahi se satura enseguida y se
// pierde precision en cada paso. El shading trabaja con este tipo en el rango
// 0..1 y recien al final se convierte de vuelta a u8.
use raylib::prelude::*;
use std::ops::{Add, Mul};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Rgb {
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Rgb { r, g, b }
    }

    /// Pasa un color de raylib al rango 0..1.
    pub fn from_color(color: Color) -> Self {
        const INV: f32 = 1.0 / 255.0;
        Rgb {
            r: color.r as f32 * INV,
            g: color.g as f32 * INV,
            b: color.b as f32 * INV,
        }
    }

    /// Vuelve a u8 recortando lo que se haya pasado de 1.0: si a un pixel le
    /// pegan varias luces puede quedar por encima del blanco.
    pub fn to_color(self) -> Color {
        Color {
            r: (self.r.clamp(0.0, 1.0) * 255.0) as u8,
            g: (self.g.clamp(0.0, 1.0) * 255.0) as u8,
            b: (self.b.clamp(0.0, 1.0) * 255.0) as u8,
            a: 255,
        }
    }
}

/// Donde empieza a doblarse la luz de mas. Por debajo de esta altura el
/// color pasa tal cual; por encima se acuesta contra el blanco. Mas alto
/// deja mas imagen sin tocar, pero la curva se cierra mas de golpe.
const KNEE: f32 = 0.8;

impl Rgb {
    /// Pasa a u8 para mostrar en pantalla, doblando lo que se paso de 1.0 en
    /// vez de recortarlo.
    ///
    /// Recortar canal por canal, que es lo que hace `to_color`, no solo quema
    /// la zona: ademas le cambia el color. Al pasto iluminado de frente le
    /// puede quedar (1.2, 1.5, 0.9); recortado sale (1.0, 1.0, 0.9), que es
    /// un amarillo palido que nunca estuvo en la escena. Por eso la
    /// compresion se calcula una sola vez sobre el canal mas alto y se
    /// aplica a los tres: la proporcion entre ellos no se mueve, asi que el
    /// pasto sigue siendo verde y lo unico que cambia es cuanto se acerca al
    /// blanco.
    pub fn to_display(self) -> Color {
        let peak = self.r.max(self.g).max(self.b);
        if peak <= KNEE {
            return self.to_color();
        }

        (self * (compress(peak) / peak)).to_color()
    }

    /// El rasterizador guarda los colores como Vector3 (x = rojo, y = verde,
    /// z = azul, de 0 a 1). Estas dos conversiones son el puente con el
    /// sombreado, que trabaja con Rgb.
    pub fn from_vector(color: Vector3) -> Self {
        Rgb::new(color.x, color.y, color.z)
    }

    pub fn to_vector(self) -> Vector3 {
        Vector3::new(self.r, self.g, self.b)
    }
}

/// Curva de la zona alta: derecha hasta la rodilla y de ahi asintotica a 1.0,
/// al que se acerca sin tocarlo por mucha luz que llegue.
fn compress(value: f32) -> f32 {
    let room = 1.0 - KNEE;
    KNEE + room * (1.0 - (-(value - KNEE) / room).exp())
}

impl Add for Rgb {
    type Output = Rgb;

    fn add(self, other: Rgb) -> Rgb {
        Rgb::new(self.r + other.r, self.g + other.g, self.b + other.b)
    }
}

/// Escalar: subir o bajar la intensidad.
impl Mul<f32> for Rgb {
    type Output = Rgb;

    fn mul(self, k: f32) -> Rgb {
        Rgb::new(self.r * k, self.g * k, self.b * k)
    }
}

/// Canal por canal: asi es como el color de una luz tine al del material.
impl Mul<Rgb> for Rgb {
    type Output = Rgb;

    fn mul(self, other: Rgb) -> Rgb {
        Rgb::new(self.r * other.r, self.g * other.g, self.b * other.b)
    }
}
