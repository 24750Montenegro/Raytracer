// camera.rs
// Camara orbital: gira alrededor de un punto fijo de la escena.
// Ojo con el signo de Y: en esta escena el eje +Y apunta hacia ABAJO
// (los centros vienen de coordenadas de pantalla), por eso el "arriba"
// del mundo es -Y y la base guarda `down` en vez de `up`.
use raylib::prelude::*;
use std::f32::consts::PI;

/// Limite del pitch para que la camara no cruce los polos y la base no se
/// degenere (arriba y forward paralelos).
const PITCH_LIMIT: f32 = PI / 2.0 - 0.01;
const MIN_DISTANCE: f32 = 0.5;
const MAX_DISTANCE: f32 = 200.0;
/// Radianes por pixel de arrastre.
const ORBIT_SENSITIVITY: f32 = 0.006;
/// Cuanto acerca/aleja cada muesca del scroll (multiplicativo).
const ZOOM_STEP: f32 = 0.12;

/// Base ortonormal de la camara ya lista para tirar rayos.
pub struct CameraBasis {
    pub origin: Vector3,
    pub right: Vector3,
    pub down: Vector3,
    pub forward: Vector3,
}

pub struct Camera {
    /// Punto al que la camara siempre mira.
    pub target: Vector3,
    /// Angulo horizontal (0 = camara sobre +Z, mirando hacia -Z).
    pub yaw: f32,
    /// Angulo vertical, positivo = camara por encima de la escena.
    pub pitch: f32,
    /// Radio de la orbita.
    pub distance: f32,
}

impl Camera {
    pub fn new(target: Vector3, distance: f32) -> Self {
        Camera {
            target,
            yaw: 0.0,
            pitch: 0.0,
            distance: distance.clamp(MIN_DISTANCE, MAX_DISTANCE),
        }
    }

    /// Posicion de la camara en coordenadas esfericas alrededor del target.
    pub fn position(&self) -> Vector3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();

        Vector3::new(
            self.target.x + self.distance * cos_pitch * sin_yaw,
            // -sin(pitch): pitch positivo sube, y arriba es -Y
            self.target.y - self.distance * sin_pitch,
            self.target.z + self.distance * cos_pitch * cos_yaw,
        )
    }

    /// Arma la base una sola vez por frame; el loop de pixeles solo interpola.
    pub fn basis(&self) -> CameraBasis {
        let origin = self.position();
        let forward = (self.target - origin).normalized();
        // +Y es "abajo" en el mundo, asi que sirve de referencia para el cross
        let right = forward.cross(Vector3::new(0.0, 1.0, 0.0)).normalized();
        let down = right.cross(forward);

        CameraBasis { origin, right, down, forward }
    }

    /// Arrastre del mouse: dx gira alrededor de la escena, dy sube y baja.
    /// Devuelve true si la camara cambio (para no re-renderizar de gratis).
    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) -> bool {
        if delta_x == 0.0 && delta_y == 0.0 {
            return false;
        }

        self.yaw -= delta_x * ORBIT_SENSITIVITY;
        self.pitch = (self.pitch - delta_y * ORBIT_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);

        // yaw acotado a [0, 2PI) para que no crezca sin control en sesiones largas
        self.yaw = self.yaw.rem_euclid(2.0 * PI);
        true
    }

    /// Scroll: acerca o aleja. El paso es multiplicativo para que el zoom
    /// se sienta igual de fino estando cerca que estando lejos.
    pub fn zoom(&mut self, wheel: f32) -> bool {
        if wheel == 0.0 {
            return false;
        }

        let distance = (self.distance * (1.0 - wheel * ZOOM_STEP)).clamp(MIN_DISTANCE, MAX_DISTANCE);
        if distance == self.distance {
            return false;
        }

        self.distance = distance;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Con yaw y pitch en cero la camara tiene que quedar igual que la fija
    /// de antes: en +Z respecto al target y mirando hacia -Z.
    #[test]
    fn default_view_looks_down_negative_z() {
        let camera = Camera::new(Vector3::new(0.0, 0.0, -5.0), 5.0);
        let basis = camera.basis();

        assert!((basis.origin - Vector3::new(0.0, 0.0, 0.0)).length() < 1e-5);
        assert!((basis.forward - Vector3::new(0.0, 0.0, -1.0)).length() < 1e-5);
        assert!((basis.right - Vector3::new(1.0, 0.0, 0.0)).length() < 1e-5);
        assert!((basis.down - Vector3::new(0.0, 1.0, 0.0)).length() < 1e-5);
    }

    /// Un cuarto de vuelta deja la camara sobre +X, mirando hacia -X.
    #[test]
    fn quarter_turn_moves_camera_to_positive_x() {
        let mut camera = Camera::new(Vector3::zero(), 4.0);
        camera.yaw = PI / 2.0;
        let basis = camera.basis();

        assert!((basis.origin - Vector3::new(4.0, 0.0, 0.0)).length() < 1e-5);
        assert!((basis.forward - Vector3::new(-1.0, 0.0, 0.0)).length() < 1e-5);
    }

    /// Pitch positivo levanta la camara: arriba es -Y en esta escena.
    #[test]
    fn positive_pitch_raises_camera() {
        let mut camera = Camera::new(Vector3::zero(), 4.0);
        assert!(camera.orbit(0.0, -100.0));
        assert!(camera.pitch > 0.0);
        assert!(camera.position().y < 0.0);
    }

    #[test]
    fn zoom_clamps_and_reports_changes() {
        let mut camera = Camera::new(Vector3::zero(), 10.0);
        assert!(camera.zoom(1.0));
        assert!(camera.distance < 10.0);
        assert!(!camera.zoom(0.0));

        camera.distance = MIN_DISTANCE;
        assert!(!camera.zoom(5.0), "no deberia bajar de la distancia minima");
    }
}
