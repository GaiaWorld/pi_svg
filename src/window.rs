use gl::types::GLuint;
use pathfinder_geometry::{rect::RectI, vector::Vector2I};
use pathfinder_gl::{GLDevice, GLVersion};
use pathfinder_resources::ResourceLoader;

pub trait Window {
    fn gl_version(&self) -> GLVersion;
    fn gl_default_framebuffer(&self) -> GLuint {
        0
    }
    fn present(&mut self, device: &mut GLDevice);

    fn viewport(&self) -> RectI;
    fn resource_loader(&self) -> &dyn ResourceLoader;
}

#[derive(Clone, Copy, Debug)]
pub struct WindowSize {
    pub logical_size: Vector2I,
    pub backing_scale_factor: f32,
}

impl WindowSize {
    #[inline]
    pub fn device_size(&self) -> Vector2I {
        (self.logical_size.to_f32() * self.backing_scale_factor).to_i32()
    }
}
