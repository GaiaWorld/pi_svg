use pathfinder_color::ColorF;
use pathfinder_geometry::{
    rect::{RectF, RectI},
    vector::{Vector2F, Vector2I},
};
use pathfinder_gl::{GLDevice as DeviceImpl, GLVersion};
use pathfinder_renderer::{
    concurrent::{executor::SequentialExecutor, scene_proxy::SceneProxy},
    gpu::{
        options::{DestFramebuffer, RendererLevel, RendererMode, RendererOptions},
        renderer::Renderer,
    },
    scene::Scene,
};
use pathfinder_resources::fs::FilesystemResourceLoader;
use pathfinder_svg::SVGScene;
use thiserror::Error;
use usvg::{Options as UsvgOptions, Tree as SvgTree};

#[derive(Error, Debug, Eq, PartialEq)]
pub enum SvgError {
    #[error("LoadSvg failed: `{0}`")]
    Load(String),

    #[error("No Load Svg data")]
    NoLoad,
}

pub struct SvgRenderer {
    gl_version: GLVersion,

    scene: Option<Scene>,
    renderer: Option<Renderer<DeviceImpl>>,

    scene_proxy: Option<SceneProxy>,

    clear_color: ColorF,

    viewport_offset: Vector2I,
    viewport_size: Vector2I,

    view_box: usvg::Rect,
}

impl Default for SvgRenderer {
    fn default() -> Self {
        Self {
            renderer: None,

            scene: None,
            scene_proxy: None,

            gl_version: get_native_gl_version(),

            clear_color: ColorF::new(1.0, 1.0, 1.0, 1.0),

            viewport_offset: Vector2I::new(0, 0),
            viewport_size: Vector2I::new(0, 0),
            view_box: usvg::Rect::new(0.0, 0.0, 0.0, 0.0).unwrap(),
        }
    }
}

impl SvgRenderer {
    /// r, g, b, a = [0.0, 1.0]
    pub fn set_clear_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.clear_color = ColorF::new(r, g, b, a);
    }

    pub fn load_svg(&mut self, data: &[u8]) -> Result<(), SvgError> {
        self.scene = None;
        self.scene_proxy = None;

        let tree = match SvgTree::from_data(data, &UsvgOptions::default()) {
            Ok(tree) => tree,
            Err(e) => return Err(SvgError::Load(e.to_string())),
        };

        let scene = SVGScene::from_tree_and_scene(&tree, Scene::new());
        if !scene.result_flags.is_empty() {
            log::warn!(
                "Warning: These features in the SVG are unsupported: {}.",
                scene.result_flags
            );
        }

        let root = tree.svg_node();
        self.viewport_size = Vector2I::new(root.size.width() as i32, root.size.height() as i32);
        self.view_box = root.view_box.rect;
        let scene = scene.scene;
        self.scene = Some(scene);

        Ok(())
    }

    pub fn set_target(&mut self, framebuffer_id: u32, x: i32, y: i32) {
        self.renderer = None;
        self.scene_proxy = None;

        self.viewport_offset = Vector2I::new(x, y);

        let viewport = RectI::new(self.viewport_offset, self.viewport_size);

        let dest_framebuffer = DestFramebuffer::Default {
            viewport,
            window_size: self.viewport_size,
        };

        let render_options = RendererOptions {
            dest: dest_framebuffer,
            background_color: None,
            show_debug_ui: true,
        };

        let device = DeviceImpl::new(self.gl_version, framebuffer_id);
        let render_mode = RendererMode {
            level: RendererLevel::D3D11,
        };
        let resources = FilesystemResourceLoader::locate();

        self.renderer = Some(Renderer::new(
            device,
            &resources,
            render_mode,
            render_options,
        ));
    }

    pub fn draw_once(&mut self) -> Result<(), SvgError> {
        let renderer = match self.renderer.as_ref() {
            Some(r) => r,
            None => {
                self.set_target(0, 0, 0);
                self.renderer.as_ref().unwrap()
            }
        };

        let scene = match self.scene.as_ref() {
            Some(s) => s,
            None => return Err(SvgError::NoLoad),
        };

        let scene_proyxy = match self.scene_proxy.as_ref() {
            Some(p) => p,
            None => {
                let scene_metadata =
                    SceneMetadata::new_clipping_view_box(&mut scene, viewport.size());
                
                    let camera = Camera::new(scene_metadata.view_box, viewport.size());

                let scene_proxy = SceneProxy::from_scene(scene, level, SequentialExecutor);
            }
        };

        self.prepare_frame();
        self.draw_scene();
        self.begin_compositing();
        self.finish_drawing_frame();

        Ok(())
    }
}

impl SvgRenderer {
    fn prepare_frame_rendering(&mut self) -> u32 {
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

    fn draw_scene(&mut self) {
        let renderer = self.renderer.device().begin_commands();

        self.renderer.device().end_commands();

        self.render_vector_scene();
    }

    fn begin_compositing(&mut self) {
        self.renderer.device().begin_commands();
    }

    #[allow(deprecated)]
    fn render_vector_scene(&mut self) {
        self.renderer.disable_depth();

        self.scene_proxy.render(&mut self.renderer);
    }
}

struct SceneMetadata {
    view_box: RectF,
}

impl SceneMetadata {
    fn new_clipping_view_box(scene: &mut Scene, viewport_size: Vector2I) -> SceneMetadata {
        let view_box = scene.view_box();
        scene.set_view_box(RectF::new(Vector2F::zero(), viewport_size.to_f32()));
        SceneMetadata { view_box }
    }
}

#[cfg(target_os = "android")]
fn get_native_gl_version() -> GLVersion {
    GLVersion::GLES3
}

#[cfg(target_os = "windows")]
fn get_native_gl_version() -> GLVersion {
    GLVersion::GL4
}
