use pathfinder_color::ColorF;
use pathfinder_geometry::{
    rect::{RectF, RectI},
    transform2d::Transform2F,
    vector::{vec2i, Vector2F, Vector2I},
};
use pathfinder_gl::{GLDevice as DeviceImpl, GLVersion};
use pathfinder_gpu::Device;
use pathfinder_renderer::{
    concurrent::{executor::SequentialExecutor, scene_proxy::SceneProxy},
    gpu::{
        options::{DestFramebuffer, RendererLevel, RendererMode, RendererOptions},
        renderer::Renderer,
    },
    options::{BuildOptions, RenderTransform},
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

    #[error("Svg data isn't set width and height")]
    NoSize,
}

pub struct SvgRenderer {
    // GL 版本，Windows 4.0，Android EL3
    gl_version: GLVersion,

    gl_level: RendererLevel,

    // 到 load_svg 创建
    scene_proxy: Option<SceneProxy>,
    // 到 set_renderer 创建
    renderer: Option<Renderer<DeviceImpl>>,

    // 清屏色
    clear_color: ColorF,
    // 渲染目标 大小
    target_size: Vector2I,

    // 视口：offset 来自 set_target
    viewport_offset: Vector2I,
    // 视口 大小：来自 svg 的 width, height
    viewport_size: Vector2I,
}

impl Default for SvgRenderer {
    fn default() -> Self {
        Self {
            gl_version: get_native_gl_version(),
            gl_level: RendererLevel::D3D9,

            scene_proxy: None,
            renderer: None,

            clear_color: ColorF::new(1.0, 0.0, 0.0, 1.0),

            viewport_offset: vec2i(0, 0),
            viewport_size: vec2i(1, 1),

            target_size: vec2i(1, 1),
        }
    }
}

impl SvgRenderer {
    pub fn load_svg(&mut self, data: &[u8]) -> Result<(), SvgError> {
        self.scene_proxy = None;

        let svg = match SvgTree::from_data(data, &UsvgOptions::default()) {
            Ok(svg) => svg,
            Err(e) => return Err(SvgError::Load(e.to_string())),
        };

        let scene = SVGScene::from_tree_and_scene(&svg, Scene::new());
        if !scene.result_flags.is_empty() {
            log::warn!(
                "Warning: These features in the SVG are unsupported: {}.",
                scene.result_flags
            );
        }

        let svg_node = svg.svg_node();
        let size = svg_node.size;
        let view_box = svg_node.view_box;
        log::info!("svg size = {:?}, view_box = {:?}", size, view_box);

        let viewport = RectI::new(
            Vector2I::new(0, 0),
            Vector2I::new(size.width() as i32, size.height() as i32),
        );

        let mut scene = scene.scene;

        // ============ load scene ============

        let view_box = scene.view_box();
        scene.set_view_box(RectF::new(Vector2F::zero(), viewport.size().to_f32()));

        let scene_proxy = SceneProxy::from_scene(scene, self.gl_level, SequentialExecutor);

        let viewport_size = viewport.size();
        let s = 1.0 / f32::min(view_box.size().x(), view_box.size().y());
        let scale = i32::min(viewport_size.x(), viewport_size.y()) as f32 * s;
        let origin = viewport_size.to_f32() * 0.5 - view_box.size() * (scale * 0.5);
        let camera = Transform2F::from_scale(scale).translate(origin);

        let build_options = BuildOptions {
            transform: RenderTransform::Transform2D(camera),
            ..Default::default()
        };
        scene_proxy.build(build_options);

        self.viewport_size = viewport_size;
        self.scene_proxy = Some(scene_proxy);

        Ok(())
    }

    pub fn set_clear_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.clear_color = ColorF::new(r, g, b, a);
    }

    pub fn set_target(
        &mut self,
        fbo_id: u32,
        viewport_x: i32,
        viewport_y: i32,
        target_w: i32,
        target_h: i32,
    ) {
        self.viewport_offset = vec2i(viewport_x, viewport_y);
        self.target_size = vec2i(target_w, target_h);

        self.renderer = Some(Renderer::new(
            DeviceImpl::new(self.gl_version, fbo_id),
            &FilesystemResourceLoader::locate(),
            RendererMode {
                level: self.gl_level,
            },
            RendererOptions {
                background_color: Some(self.clear_color),
                show_debug_ui: false,
                dest: DestFramebuffer::Default {
                    viewport: RectI::new(self.viewport_offset, self.viewport_size),
                    window_size: self.target_size,
                },
            },
        ));
    }

    pub fn draw_once(&mut self, target_size: Option<(i32, i32)>) -> Result<(), SvgError> {
        if self.scene_proxy.is_none() {
            return Err(SvgError::NoLoad);
        }

        if self.renderer.is_none() {
            let (w, h) = target_size.unwrap();
            self.set_target(0, 0, 0, w, h);
        }

        let scene_proxy = self.scene_proxy.as_mut().unwrap();
        let renderer = self.renderer.as_mut().unwrap();

        *renderer.options_mut() = RendererOptions {
            background_color: Some(self.clear_color),
            show_debug_ui: false,
            dest: DestFramebuffer::Default {
                viewport: RectI::new(self.viewport_offset, self.viewport_size),
                window_size: self.target_size,
            },
        };

        // renderer.disable_depth();
        renderer.device().begin_commands();
        
        scene_proxy.render(renderer);

        renderer.device().end_commands();

        Ok(())
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
