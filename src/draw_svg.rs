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

/// SVG 解析和渲染遇到 的 错误
#[derive(Error, Debug, Eq, PartialEq)]
pub enum SvgError {
    #[error("LoadSvg failed: `{0}`")]
    Load(String),

    #[error("No Load Svg data")]
    NoLoad,

    #[error("Svg data isn't set width and height")]
    NoSize,
}

/// Svg 渲染器
pub struct SvgRenderer {
    // GL 版本，Windows 4.0，Android EL3
    gl_version: GLVersion,
    /// 为了兼容 手机，暂时用 D3D9
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
    viewport_size: Option<Vector2I>,
}

impl SvgRenderer {
    pub fn new(
        fbo_id: u32,
        target_w: i32,
        target_h: i32,
        vp_offset: (i32, i32),
        vp_size: Option<(i32, i32)>,
    ) -> Self {
        let mut s = Self {
            gl_version: get_native_gl_version(),
            gl_level: RendererLevel::D3D9,

            scene_proxy: None,
            renderer: None,

            clear_color: ColorF::new(1.0, 0.0, 0.0, 1.0),

            viewport_offset: vec2i(0, 0),
            viewport_size: None,

            target_size: vec2i(1, 1),
        };

        s.set_target(fbo_id, target_w, target_h);
        s.set_viewport(vp_offset.0, vp_offset.1, vp_size);

        s
    }

    /// 设置背景色
    pub fn set_clear_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.clear_color = ColorF::new(r, g, b, a);
    }

    /// 加载 svg 二进制数据，格式 见 examples/ 的 svg 文件
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

        if self.viewport_size.is_none() {
            self.viewport_size = Some(vec2i(size.width() as i32, size.height() as i32));
        }

        let viewport = RectI::new(self.viewport_offset, self.viewport_size.unwrap());

        let mut scene = scene.scene;

        // ============ load scene_proxy ============

        let view_box = scene.view_box();
        scene.set_view_box(RectF::new(Vector2F::zero(), viewport.size().to_f32()));

        let scene_proxy = SceneProxy::from_scene(scene, self.gl_level, SequentialExecutor);

        let viewport_size = viewport.size();
        let s = 1.0 / f32::min(view_box.size().x(), view_box.size().y());
        let scale = i32::min(viewport_size.x(), viewport_size.y()) as f32 * s;
        let origin = viewport_size.to_f32() * 0.5 - view_box.size() * (scale * 0.5);
        let camera = Transform2F::from_scale(scale).translate(origin);

        scene_proxy.build(BuildOptions {
            transform: RenderTransform::Transform2D(camera),
            ..Default::default()
        });
        self.scene_proxy = Some(scene_proxy);

        Ok(())
    }

    pub fn draw_once(&mut self, target_size: Option<(i32, i32)>) -> Result<(), SvgError> {
        if self.scene_proxy.is_none() {
            return Err(SvgError::NoLoad);
        }

        if self.renderer.is_none() {
            let (w, h) = target_size.unwrap();
            self.set_target(0, w, h);
        }

        let scene_proxy = self.scene_proxy.as_mut().unwrap();
        let renderer = self.renderer.as_mut().unwrap();

        *renderer.options_mut() = RendererOptions {
            show_debug_ui: false,
            background_color: Some(self.clear_color),
            dest: DestFramebuffer::Default {
                viewport: RectI::new(self.viewport_offset, self.viewport_size.unwrap()),
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

impl SvgRenderer {
    // 设置 渲染目标
    fn set_target(&mut self, fbo_id: u32, target_w: i32, target_h: i32) {
        self.target_size = vec2i(target_w, target_h);

        let viewport_size = match self.viewport_size {
            Some(s) => s,
            None => vec2i(1, 1),
        };

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
                    viewport: RectI::new(self.viewport_offset, viewport_size),
                    window_size: self.target_size,
                },
            },
        ));
    }

    // 设置 视口
    fn set_viewport(&mut self, x: i32, y: i32, size: Option<(i32, i32)>) {
        self.viewport_offset = vec2i(x, y);
        if let Some((w, h)) = size {
            self.viewport_size = Some(vec2i(w, h));
        }
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
