// selector.rs
// Pantalla de inicio: las naves de assets/models giran una al lado de la
// otra para verlas antes de elegir.
use raylib::prelude::*;

use crate::framebuffer::Framebuffer;
use crate::matrix::{create_model_matrix, multiply, new_matrix4, rotation_x};
use crate::shaders::Normals;
use crate::ships::Ship;
use crate::{render_objects, sun, SceneObject, Uniforms, HEIGHT, INITIAL_SUN_ANGLE, WIDTH};

/// Inclinacion de la camara: mirando un poco desde arriba se lee mejor la
/// forma de la nave.
const TILT: f32 = 0.4;
const SPIN_SPEED: f32 = 0.8;
/// Cuanto mas chicas se ven las naves que no estan elegidas.
const UNFOCUSED_SHRINK: f32 = 0.3;
/// Que tan rapido crece la nave elegida (y se achica la anterior).
const FOCUS_SPEED: f32 = 8.0;

/// Altura del centro de las naves en pantalla, y del marco que las rodea.
const ROW_Y: f32 = 294.0;
const BOX_TOP: i32 = 118;
const BOX_BOTTOM: i32 = 470;
const BOX_MARGIN: i32 = 14;

const SELECTED_FRAME: Color = Color::new(110, 170, 255, 255);
const IDLE_FRAME: Color = Color::new(40, 48, 66, 255);

pub struct Selector {
    selected: usize,
    /// De 0 a 1 por nave: que tan agrandada esta. Sigue a `selected` de a
    /// poco para que el cambio no sea de golpe.
    focus: Vec<f32>,
    time: f32,
}

impl Selector {
    pub fn new(count: usize) -> Self {
        let mut focus = vec![0.0; count];
        focus[0] = 1.0;
        Selector { selected: 0, focus, time: 0.0 }
    }

    /// Mouse o flechas (A/D) para mirar, click o Enter para elegir.
    /// Devuelve la nave elegida.
    pub fn update(&mut self, window: &RaylibHandle, dt: f32) -> Option<usize> {
        let count = self.focus.len();
        self.time += dt;

        // la nave bajo el mouse se elige solo si el mouse se movio; si no, un
        // mouse quieto le ganaria siempre al teclado
        let hovered = (window.get_mouse_position().x / slot_width(count)) as usize;
        let hovered = hovered.min(count - 1);
        let delta = window.get_mouse_delta();
        if delta.x != 0.0 || delta.y != 0.0 {
            self.selected = hovered;
        }

        let pressed = |keys: [KeyboardKey; 2]| keys.iter().any(|key| window.is_key_pressed(*key));
        if pressed([KeyboardKey::KEY_LEFT, KeyboardKey::KEY_A]) {
            self.selected = (self.selected + count - 1) % count;
        }
        if pressed([KeyboardKey::KEY_RIGHT, KeyboardKey::KEY_D]) {
            self.selected = (self.selected + 1) % count;
        }

        let blend = 1.0 - (-FOCUS_SPEED * dt).exp();
        for (index, focus) in self.focus.iter_mut().enumerate() {
            let target = if index == self.selected { 1.0 } else { 0.0 };
            *focus += (target - *focus) * blend;
        }

        if window.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
            return Some(hovered);
        }
        if pressed([KeyboardKey::KEY_ENTER, KeyboardKey::KEY_SPACE]) {
            return Some(self.selected);
        }
        None
    }

    pub fn render(&self, framebuffer: &mut Framebuffer, uniforms: &mut Uniforms, ships: &[Ship]) {
        // camara ortografica fija; el viewport mide en pixeles, asi que las
        // naves se escalan directo al radio que tienen en pantalla
        uniforms.view_matrix = rotation_x(TILT);
        uniforms.viewport_matrix = new_matrix4([
            [1.0, 0.0, 0.0, WIDTH as f32 / 2.0],
            [0.0, -1.0, 0.0, ROW_Y],
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        uniforms.to_viewer = Vector3::new(0.0, TILT.sin(), TILT.cos());
        uniforms.sun = sun(INITIAL_SUN_ANGLE);
        uniforms.time = self.time;

        let slot = slot_width(ships.len());
        let largest = (slot * 0.38).min(HEIGHT as f32 * 0.26);
        let objects: Vec<_> = ships
            .iter()
            .zip(&self.focus)
            .enumerate()
            .map(|(index, (ship, focus))| {
                let center_x = slot * (index as f32 + 0.5) - WIDTH as f32 / 2.0;
                let radius = largest * (1.0 - UNFOCUSED_SHRINK * (1.0 - focus));
                let spin = Vector3::new(0.0, self.time * SPIN_SPEED, 0.0);
                let model_matrix = multiply(
                    &create_model_matrix(Vector3::new(center_x, 0.0, 0.0), radius, spin),
                    &ship.fit,
                );
                SceneObject {
                    normals: Normals::Mesh,
                    ..SceneObject::new(&ship.vertices, model_matrix, ship.hull)
                }
            })
            .collect();

        framebuffer.clear_with_starfield();
        // los marcos van antes que las naves para quedar detras
        for index in 0..ships.len() {
            let color = if index == self.selected { SELECTED_FRAME } else { IDLE_FRAME };
            let left = (slot * index as f32) as i32 + BOX_MARGIN;
            let width = slot as i32 - 2 * BOX_MARGIN;
            framebuffer.outline(left, BOX_TOP, width, BOX_BOTTOM - BOX_TOP, 2, color);
        }
        render_objects(framebuffer, uniforms, &objects);
    }

    pub fn overlay(&self, d: &mut RaylibDrawHandle, ships: &[Ship]) {
        let middle = WIDTH / 2;
        centered(d, "SELECCIONA TU NAVE", middle, 34, 36, Color::WHITE);
        centered(d, "Mouse o flechas para mirar | click o Enter para despegar", middle, 80, 16, Color::LIGHTGRAY);

        let slot = slot_width(ships.len());
        for (index, ship) in ships.iter().enumerate() {
            let center = (slot * (index as f32 + 0.5)) as i32;
            let color = if index == self.selected { Color::WHITE } else { Color::GRAY };
            centered(d, &ship.name, center, BOX_BOTTOM + 14, 20, color);
            centered(d, &format!("{} triangulos", ship.triangles()), center, BOX_BOTTOM + 40, 16, Color::GRAY);
        }
    }
}

fn slot_width(count: usize) -> f32 {
    WIDTH as f32 / count as f32
}

/// Texto centrado en `center_x`.
pub fn centered(d: &mut RaylibDrawHandle, text: &str, center_x: i32, y: i32, size: i32, color: Color) {
    let width = d.measure_text(text, size);
    d.draw_text(text, center_x - width / 2, y, size, color);
}
