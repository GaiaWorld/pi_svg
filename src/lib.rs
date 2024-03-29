use pathfinder_color::ColorF;
use pathfinder_geometry::{
    rect::{RectF, RectI},
    transform2d::Transform2F,
    vector::{vec2f, vec2i, Vector2F, Vector2I},
};
use pathfinder_gl::{GLDevice as DeviceImpl, GLVersion};

use pathfinder_renderer::{
    concurrent::{rayon::RayonExecutor, scene_proxy::SceneProxy},
    gpu::{
        options::{DestFramebuffer, RendererOptions},
        renderer::Renderer,
    },
    options::{BuildOptions, RenderTransform},
};

use pathfinder_svg::BuiltSVG;
use res::MemResourceLoader;
use thiserror::Error;
use usvg::{Options as UsvgOptions, Tree as SvgTree};

pub use pathfinder_renderer::scene::Scene;

mod res;

/// SVG 解析和渲染遇到 的 错误
#[derive(Error, Debug, Eq, PartialEq)]
pub enum SvgError {
    #[error("LoadSvg failed: Invalid Scene Key, can't set 0")]
    InvalidSceneKey,

    #[error("LoadSvg failed: `{0}`")]
    Load(String),

    #[error("No Load Svg data")]
    NoLoad,

    #[error("Svg data isn't set width and height")]
    NoSize,
}

/// Svg 渲染器
pub struct SvgRenderer {
    // gl_level: RendererLevel,
    scene_proxy: SceneProxy,
    renderer: Renderer<DeviceImpl>,

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
        // GL 版本，Windows 4.0，Android EL3

        let gl_version = get_native_gl_version();

        // 为了兼容 手机，暂时用 D3D9
        // let gl_level = RendererLevel::D3D9;

        let device = DeviceImpl::new(gl_version, 0);
        let resource_loader = MemResourceLoader::default();
        let begin = std::time::Instant::now();
        let renderer = Renderer::new(
            device,
            &resource_loader,
            DestFramebuffer::full_window(Vector2I::new(1, 1)),
            RendererOptions {
                background_color: None,
            },
        );
        println!("========== time: {:?}", begin.elapsed());
        let scene_proxy = SceneProxy::new(RayonExecutor);

        Self {
            // gl_level,
            renderer,
            scene_proxy,

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
        println!(
            "============= pi_svg: set_target. fbo_id = {}, target_w = {}，target_h = {}",
            fbo_id, target_w, target_h
        );

        self.target_size = vec2i(target_w, target_h);

        let viewport_size = match self.viewport_size {
            Some(s) => s,
            None => vec2i(1, 1),
        };

        self.fbo_id = fbo_id;
        self.renderer.device.set_default_framebuffer(fbo_id);
    }

    // 设置 视口
    pub fn set_viewport(&mut self, x: i32, y: i32, size: Option<(i32, i32)>) {
        println!(
            "============= pi_svg: set_viewport, x = {}, y = {}，size = {:?}",
            x, y, size
        );

        self.viewport_offset = vec2i(x, y);
        if let Some((w, h)) = size {
            self.renderer
                .replace_dest_framebuffer(DestFramebuffer::full_window(Vector2I::new(w, h)));
            self.viewport_size = Some(vec2i(w, h));
        }
    }

    /// 加载 svg 二进制数据，格式 见 examples/ 的 svg 文件
    pub fn load_svg(&mut self, svg_data: &[u8]) -> Result<Scene, SvgError> {
        // println!("pi_svg, load_svg: data.len = {}", data.len());

        let svg = match SvgTree::from_data(svg_data, &UsvgOptions::default().to_ref()) {
            Ok(svg) => svg,
            Err(e) => return Err(SvgError::Load(e.to_string())),
        };

        let scene = BuiltSVG::from_tree_and_scene(&svg, Scene::new());
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
        println!("==== view_box: {:?}", self.view_box);

        Ok(scene.scene)
    }

    pub fn draw_once(&mut self, scene: &Scene) -> Result<(), SvgError> {
        self.scene_proxy.replace_scene(scene.clone());

        // 注：看了 pathfinder 的源码，这里必须要每次 构建
        // Self::build_scene(
        //     &mut self.scene_proxy,
        //     (self.viewport_offset, *self.viewport_size.as_ref().unwrap()),
        //     &self.view_box,
        // );

        let viewport = RectI::new(self.viewport_offset, *self.viewport_size.as_ref().unwrap());

        self.scene_proxy
            .set_view_box(RectF::new(Vector2F::zero(), viewport.size().to_f32()));

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

        self.renderer.set_options(RendererOptions {
            // show_debug_ui: false,
            // 注：这里的清屏，是 清全屏，将前面画的也清空掉了，所以不能用
            background_color: None,
            // dest: DestFramebuffer::Default {
            //     viewport: RectI::new(vp_offset, vp_size),
            //     window_size: self.target_size,
            // },
        });

        let scale = f32::min(
            viewport.width() as f32 / self.view_box.width(),
            viewport.height() as f32 / self.view_box.height(),
        );

        // https://www.zhangxinxu.com/wordpress/2014/08/svg-viewport-viewbox-preserveaspectratio/
        // 默认是 preserveAspectRatio="xMidYMid meet" 中心对齐

        let origin = viewport.size().to_f32() * 0.5 - self.view_box.size() * (scale * 0.5);
        // self.renderer.dest_framebuffer().window_size(device)
        // let origin = Vector2F::new(0., 0.);
        // let y = 720 - viewport.height();
        // let origin = Vector2F::new(0., y as f32);

        println!(
            "===================== origin: {:?}, scale: {}",
            origin, scale
        );
        let camera = Transform2F::from_scale(scale).translate(origin);

        self.scene_proxy.build_and_render(
            &mut self.renderer,
            BuildOptions {
                transform: RenderTransform::Transform2D(camera),
                ..Default::default()
            },
        );

        Ok(())
    }
}

#[cfg(target_os = "android")]
fn get_native_gl_version() -> GLVersion {
    GLVersion::GLES3
}

#[cfg(target_os = "windows")]
fn get_native_gl_version() -> GLVersion {
    GLVersion::GL3
}
