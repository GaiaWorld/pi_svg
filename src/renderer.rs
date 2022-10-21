use super::{window::Window, DemoApp};
use pathfinder_color::ColorF;
use pathfinder_geometry::{rect::RectF, transform2d::Transform2F, vector::Vector2I};
use pathfinder_gpu::Device;
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererOptions};

pub struct Camera(pub Transform2F);

impl Camera {
    pub fn new(view_box: RectF, viewport_size: Vector2I) -> Camera {
        let s = 1.0 / f32::min(view_box.size().x(), view_box.size().y());

        let scale = i32::min(viewport_size.x(), viewport_size.y()) as f32 * s;

        let origin = viewport_size.to_f32() * 0.5 - view_box.size() * (scale * 0.5);

        Camera(Transform2F::from_scale(scale).translate(origin))
    }
}

impl<W> DemoApp<W>
where
    W: Window,
{
    pub fn prepare_frame_rendering(&mut self) -> u32 {
        let clear_color = Some(ColorF::new(1.0, 1.0, 0.0, 1.0));

        let window_size = self.window_size.device_size();
        let scene_count = {
            *self.renderer.options_mut() = RendererOptions {
                dest: DestFramebuffer::Default {
                    viewport: self.window.viewport(),
                    window_size,
                },
                background_color: clear_color,
                show_debug_ui: false,
            };
            1
        };
        scene_count
    }

    pub fn draw_scene(&mut self) {
        self.renderer.device().begin_commands();

        self.renderer.device().end_commands();

        self.render_vector_scene();
    }

    pub fn begin_compositing(&mut self) {
        self.renderer.device().begin_commands();
    }

    #[allow(deprecated)]
    fn render_vector_scene(&mut self) {
        self.renderer.disable_depth();

        // Issue render commands!
        self.scene_proxy.render(&mut self.renderer);
    }
}
