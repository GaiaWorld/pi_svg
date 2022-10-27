use pathfinder_color::ColorF;
use pathfinder_geometry::{
    rect::{RectF, RectI},
    transform2d::Transform2F,
    vector::{vec2f, vec2i, Vector2F, Vector2I},
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
use pathfinder_svg::SVGScene;
use thiserror::Error;
use usvg::{Options as UsvgOptions, Tree as SvgTree};

mod res;

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

    // 渲染目标
    fbo_id: u32,
    // 清屏色
    clear_color: ColorF,
    // 渲染目标 大小
    target_size: Vector2I,

    view_box: RectF,
    // 视口：offset 来自 set_target
    viewport_offset: Vector2I,
    // 视口 大小：来自 svg 的 width, height
    viewport_size: Option<Vector2I>,
}

impl Default for SvgRenderer {
    fn default() -> Self {
        Self {
            gl_version: get_native_gl_version(),
            gl_level: RendererLevel::D3D9,

            scene_proxy: None,
            renderer: None,

            fbo_id: 0,
            clear_color: ColorF::new(1.0, 0.0, 0.0, 1.0),

            view_box: RectF::new(vec2f(0.0, 0.0), vec2f(0.0, 0.0)),
            viewport_offset: vec2i(0, 0),
            viewport_size: None,

            target_size: vec2i(1, 1),
        }
    }
}

impl SvgRenderer {
    /// 加载 gl 接口，因为 gl库 版本不同，所以需要显示调用一次 load
    pub fn load_gl_with(load_func: impl Fn(&str) -> *const std::ffi::c_void) {
        gl::load_with(load_func);
    }

    /// 设置背景色
    pub fn set_clear_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.clear_color = ColorF::new(r, g, b, a);
    }

    // 设置 渲染目标
    pub fn set_target(&mut self, fbo_id: u32, target_w: i32, target_h: i32) {
        self.target_size = vec2i(target_w, target_h);

        let viewport_size = match self.viewport_size {
            Some(s) => s,
            None => vec2i(1, 1),
        };

        self.fbo_id = fbo_id;
        self.renderer = Some(Renderer::new(
            DeviceImpl::new(self.gl_version, self.fbo_id),
            &res::MemResourceLoader::default(),
            RendererMode {
                level: self.gl_level,
            },
            RendererOptions {
                background_color: None,
                show_debug_ui: false,
                dest: DestFramebuffer::Default {
                    viewport: RectI::new(self.viewport_offset, viewport_size),
                    window_size: self.target_size,
                },
            },
        ));
    }

    // 设置 视口
    pub fn set_viewport(&mut self, x: i32, y: i32, size: Option<(i32, i32)>) {
        self.viewport_offset = vec2i(x, y);
        if let Some((w, h)) = size {
            self.viewport_size = Some(vec2i(w, h));
        }
    }

    /// 加载 svg 二进制数据，格式 见 examples/ 的 svg 文件
    pub fn load_svg(&mut self, data: &[u8]) -> Result<(), SvgError> {
        self.scene_proxy = None;

        let svg = match SvgTree::from_data(data, &UsvgOptions::default().to_ref()) {
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

        if self.viewport_size.is_none() {
            self.viewport_size = Some(vec2i(size.width() as i32, size.height() as i32));
        }

        self.view_box = scene.scene.view_box();
        self.scene_proxy = Some(SceneProxy::from_scene(
            scene.scene,
            self.gl_level,
            SequentialExecutor,
        ));

        Ok(())
    }

    pub fn draw_once(&mut self) -> Result<(), SvgError> {
        if self.scene_proxy.is_none() {
            return Err(SvgError::NoLoad);
        }

        if self.renderer.is_none() {
            let w = self.target_size.x();
            let h = self.target_size.y();
            self.set_target(0, w, h);
        }

        // 注：看了 pathfinder 的源码，这里必须要每次 构建
        let scene_proxy = self.scene_proxy.as_mut().unwrap();
        Self::build_scene(
            scene_proxy,
            (self.viewport_offset, *self.viewport_size.as_ref().unwrap()),
            &self.view_box,
        );

        let vp_offset = self.viewport_offset;
        let vp_size = self.viewport_size.unwrap();
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo_id);

            gl::Viewport(vp_offset.x(), vp_offset.y(), vp_size.x(), vp_size.y());

            gl::Enable(gl::SCISSOR_TEST);
            gl::Scissor(vp_offset.x(), vp_offset.y(), vp_size.x(), vp_size.y());
            gl::ClearColor(
                self.clear_color.r(),
                self.clear_color.g(),
                self.clear_color.b(),
                self.clear_color.a(),
            );
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::Disable(gl::SCISSOR_TEST);
        }

        let renderer = self.renderer.as_mut().unwrap();
        *renderer.options_mut() = RendererOptions {
            show_debug_ui: false,
            // 注：这里的清屏，是 清全屏，将前面画的也清空掉了，所以不能用
            background_color: None,
            dest: DestFramebuffer::Default {
                viewport: RectI::new(vp_offset, vp_size),
                window_size: self.target_size,
            },
        };

        scene_proxy.render(renderer);

        Ok(())
    }
}

impl SvgRenderer {
    fn build_scene(scene_proxy: &mut SceneProxy, viewport: (Vector2I, Vector2I), view_box: &RectF) {
        let viewport = RectI::new(viewport.0, viewport.1);

        scene_proxy.set_view_box(RectF::new(Vector2F::zero(), viewport.size().to_f32()));

        let scale = f32::min(
            viewport.width() as f32 / view_box.width(),
            viewport.height() as f32 / view_box.height(),
        );

        // https://www.zhangxinxu.com/wordpress/2014/08/svg-viewport-viewbox-preserveaspectratio/
        // 默认是 preserveAspectRatio="xMidYMid meet" 中心对齐
        let origin = viewport.size().to_f32() * 0.5 - view_box.size() * (scale * 0.5);
        let camera = Transform2F::from_scale(scale).translate(origin);

        scene_proxy.build(BuildOptions {
            transform: RenderTransform::Transform2D(camera),
            ..Default::default()
        });
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
