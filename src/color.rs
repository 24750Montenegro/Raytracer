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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_raylib_color() {
        let original = Color::new(182, 188, 121, 255);
        let back = Rgb::from_color(original).to_color();

        assert_eq!(back.r, original.r);
        assert_eq!(back.g, original.g);
        assert_eq!(back.b, original.b);
    }

    #[test]
    fn clamps_instead_of_wrapping() {
        let too_bright = Rgb::new(2.0, 1.5, -0.5).to_color();

        assert_eq!(too_bright.r, 255);
        assert_eq!(too_bright.g, 255);
        assert_eq!(too_bright.b, 0);
    }

    #[test]
    fn tints_channel_by_channel() {
        let material = Rgb::new(1.0, 0.5, 0.25);
        let light = Rgb::new(1.0, 1.0, 0.0);

        assert_eq!(material * light, Rgb::new(1.0, 0.5, 0.0));
    }
}
