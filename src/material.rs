use raylib::prelude::*;

/// Albedo tipico de un material mate. Nada real devuelve toda la luz que le
/// llega: la nieve fresca ronda 0.9 y el asfalto 0.1, asi que quedarse un
/// poco por debajo de 1 ya se ve mas creible que el color plano de antes.
pub const DEFAULT_ALBEDO: f32 = 0.85;

#[derive(Debug, Clone, Copy)]
pub struct Material {
    /// Color base de la superficie.
    pub diffuse: Color,
    /// Fraccion de la luz incidente que la superficie devuelve (0..1).
    /// Lo que no devuelve, lo absorbe.
    pub albedo: f32,
}

impl Material {
    pub fn new(diffuse: Color, albedo: f32) -> Self {
        Material { diffuse, albedo: albedo.clamp(0.0, 1.0) }
    }

    /// Material mate: rebota la luz parejo en todas las direcciones.
    pub fn matte(diffuse: Color) -> Self {
        Material::new(diffuse, DEFAULT_ALBEDO)
    }
}
