use std::time::Instant;

use pathfinder_color::ColorF;
use pathfinder_geometry::{
    rect::{RectF, RectI},
    transform2d::Transform2F,
    vector::{vec2f, vec2i, Vector2F, Vector2I},
};
use pathfinder_gl::{GLDevice as DeviceImpl, GLVersion};
use pathfinder_gpu::Device;
use pathfinder_renderer::{
    concurrent::{executor::SequentialExecutor, rayon::RayonExecutor, scene_proxy::SceneProxy},
    gpu::{
        options::{DestFramebuffer, RendererLevel, RendererMode, RendererOptions},
        renderer::Renderer,
    },
    options::{BuildOptions, RenderTransform},
    scene::Scene,
};
use pathfinder_svg::SVGScene;
use pi_hash::XHashMap;
use res::MemResourceLoader;
use thiserror::Error;
use usvg::{Options as UsvgOptions, Tree as SvgTree};

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
    gl_level: RendererLevel,

    scene_proxy: SceneProxy,
    renderer: Renderer<DeviceImpl>,

    // 场景, 0 不能用做键
    last_svg_key: u32,
    scene_map: XHashMap<u32, Scene>,

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
        let gl_level = RendererLevel::D3D9;

        let device = DeviceImpl::new(gl_version, 0);
        let resource_loader = MemResourceLoader::default();

        let renderer = Renderer::new(
            device,
            &resource_loader,
            RendererMode { level: gl_level },
            RendererOptions {
                background_color: None,
                show_debug_ui: false,
                dest: DestFramebuffer::Default {
                    viewport: RectI::new(vec2i(0, 0), vec2i(1, 1)),
                    window_size: vec2i(1, 1),
                },
            },
        );

        let scene_proxy = SceneProxy::new(gl_level, RayonExecutor);

        Self {
            gl_level,

            renderer,
            scene_proxy,

            last_svg_key: 0,
            scene_map: XHashMap::default(),

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
        // println!(
        //     "============= pi_svg: set_target. fbo_id = {}, target_w = {}，target_h = {}",
        //     fbo_id, target_w, target_h
        // );

        self.target_size = vec2i(target_w, target_h);

        let viewport_size = match self.viewport_size {
            Some(s) => s,
            None => vec2i(1, 1),
        };

        self.fbo_id = fbo_id;
        self.renderer.device_mut().set_default_framebuffer(fbo_id);
    }

    // 设置 视口
    pub fn set_viewport(&mut self, x: i32, y: i32, size: Option<(i32, i32)>) {
        // println!(
        //     "============= pi_svg: set_viewport, x = {}, y = {}，size = {:?}",
        //     x, y, size
        // );

        self.viewport_offset = vec2i(x, y);
        if let Some((w, h)) = size {
            self.viewport_size = Some(vec2i(w, h));
        }
    }

    /// 加载 svg 二进制数据，格式 见 examples/ 的 svg 文件
    /// svg_key 不能为 0
    pub fn unload_svg(&mut self, svg_key: u32) {
        self.scene_map.remove(&svg_key);
    }

    /// 加载 svg 二进制数据，格式 见 examples/ 的 svg 文件
    /// svg_key 不能为 0
    pub fn load_svg(&mut self, svg_key: u32, svg_data: &[u8]) -> Result<(), SvgError> {
        // println!("pi_svg, load_svg: data.len = {}", data.len());

        if svg_key == 0 {
            return Err(SvgError::InvalidSceneKey);
        }

        if self.scene_map.contains_key(&svg_key) {
            return Ok(());
        }

        let svg = match SvgTree::from_data(svg_data, &UsvgOptions::default().to_ref()) {
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

        self.scene_map.insert(svg_key, scene.scene);

        Ok(())
    }

    pub fn draw_once(&mut self, svg_key: u32) -> Result<(), SvgError> {
        if svg_key == 0 {
            return Err(SvgError::InvalidSceneKey);
        }

        if self.last_svg_key != svg_key {
            let scene = match self.scene_map.get(&svg_key) {
                Some(s) => s.clone(),
                None => {
                    return Err(SvgError::NoLoad);
                }
            };
            self.scene_proxy.replace_scene(scene);
            self.last_svg_key = svg_key;
        }

        // 注：看了 pathfinder 的源码，这里必须要每次 构建
        Self::build_scene(
            &mut self.scene_proxy,
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

        *self.renderer.options_mut() = RendererOptions {
            show_debug_ui: false,
            // 注：这里的清屏，是 清全屏，将前面画的也清空掉了，所以不能用
            background_color: None,
            dest: DestFramebuffer::Default {
                viewport: RectI::new(vp_offset, vp_size),
                window_size: self.target_size,
            },
        };

        self.scene_proxy.render(&mut self.renderer);

        Ok(())
    }
}

impl SvgRenderer {
    fn build_scene(scene_proxy: &mut SceneProxy, viewport: (Vector2I, Vector2I), view_box: &RectF) {
        // println!(
        //     "pi_svg, build_scene: viewport = {:?}, view_box = {:?}",
        //     viewport, view_box
        // );

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
