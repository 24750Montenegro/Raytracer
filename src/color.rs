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

    /// Lo que no se paso de la rodilla sale igual que por `to_color`: la
    /// compresion no tiene que tocar la parte normal de la imagen.
    #[test]
    fn normal_light_goes_through_untouched() {
        let color = Rgb::new(0.5, 0.3, 0.7);

        assert_eq!(color.to_display(), color.to_color());
    }

    /// Y lo que si se paso mantiene el color: lo unico que cambia es que se
    /// acerca al blanco, no que se vuelva de otro tono.
    #[test]
    fn overbright_color_keeps_its_hue() {
        let grass = Rgb::new(0.6, 1.5, 0.3);
        let shown = grass.to_display();
        let clipped = grass.to_color();

        // recortado, el rojo y el verde quedan pegados y el verde se pierde
        assert_eq!(clipped.g, 255);
        assert!(shown.g < 255, "no tendria que llegar al blanco: {shown:?}");
        assert!(
            shown.g as i32 - shown.r as i32 > clipped.g as i32 - clipped.r as i32,
            "el verde se tendria que seguir notando: {shown:?} contra {clipped:?}"
        );
    }

    /// Por mucha luz que llegue, el orden entre los canales se respeta: es
    /// lo que mantiene el color. Recortando, en cambio, los canales se van
    /// juntando en 255 hasta que todo termina blanco.
    #[test]
    fn no_amount_of_light_breaks_the_color() {
        for light in [1.0, 4.0, 40.0, 400.0] {
            let bright = Rgb::new(light, light * 0.5, light * 0.25);
            let shown = bright.to_display();

            assert!(shown.r >= shown.g && shown.g >= shown.b, "con {light}: {shown:?}");
            assert!(shown.b > 0, "con {light} el azul se fue a negro");
        }
    }

    /// Mas luz siempre tiene que verse mas claro: si la curva se aplanara
    /// del todo, dos zonas distintas saldrian del mismo tono.
    #[test]
    fn brighter_still_reads_brighter() {
        let dim = Rgb::new(1.0, 1.0, 1.0).to_display();
        let bright = Rgb::new(1.6, 1.6, 1.6).to_display();

        assert!(bright.r > dim.r, "{} contra {}", bright.r, dim.r);
    }

    #[test]
    fn tints_channel_by_channel() {
        let material = Rgb::new(1.0, 0.5, 0.25);
        let light = Rgb::new(1.0, 1.0, 0.0);

        assert_eq!(material * light, Rgb::new(1.0, 0.5, 0.0));
    }
}
