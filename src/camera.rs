use pathfinder_geometry::{rect::RectF, transform2d::Transform2F, vector::Vector2I};

pub struct Camera(pub Transform2F);

impl Camera {
    pub fn new(view_box: RectF, viewport_size: Vector2I) -> Camera {
        let s = 1.0 / f32::min(view_box.size().x(), view_box.size().y());

        let scale = i32::min(viewport_size.x(), viewport_size.y()) as f32 * s;

        let origin = viewport_size.to_f32() * 0.5 - view_box.size() * (scale * 0.5);

        Camera(Transform2F::from_scale(scale).translate(origin))
    }
}
