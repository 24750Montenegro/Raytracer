use raylib::math::Vector3;

pub struct Fragment {
    pub object_position: Vector3,
    pub world_position: Vector3,
    /// Normal de la malla interpolada, en coordenadas del modelo.
    pub normal: Vector3,
}
