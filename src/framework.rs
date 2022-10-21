use super::window::{Window, WindowSize};
use pathfinder_color::ColorF;
use pathfinder_content::{
    effects::PatternFilter, outline::Outline, pattern::Pattern, render_target::RenderTargetId,
};
use pathfinder_geometry::{
    rect::{RectF, RectI},
    vector::{vec2i, Vector2F, Vector2I}, transform2d::Transform2F,
};
use pathfinder_gl::GLDevice as DeviceImpl;
use pathfinder_gpu::Device;
use pathfinder_renderer::{
    concurrent::{executor::SequentialExecutor, scene_proxy::SceneProxy},
    gpu::{
        options::{DestFramebuffer, RendererLevel, RendererMode, RendererOptions},
        renderer::Renderer,
    },
    options::{BuildOptions, RenderTransform},
    paint::Paint,
    scene::{DrawPath, RenderTarget, Scene},
};
use pathfinder_svg::SVGScene;
use std::path::PathBuf;
use usvg::{Options as UsvgOptions, Tree as SvgTree};

pub struct Camera(pub Transform2F);

impl Camera {
    pub fn new(view_box: RectF, viewport_size: Vector2I) -> Camera {
        let s = 1.0 / f32::min(view_box.size().x(), view_box.size().y());

        let scale = i32::min(viewport_size.x(), viewport_size.y()) as f32 * s;

        let origin = viewport_size.to_f32() * 0.5 - view_box.size() * (scale * 0.5);

        Camera(Transform2F::from_scale(scale).translate(origin))
    }
}

pub struct DemoApp<W>
where
    W: Window,
{
    pub window: W,
    window_size: WindowSize,

    render_transform: Option<RenderTransform>,

    camera: Camera,

    scene_proxy: SceneProxy,
    renderer: Renderer<DeviceImpl>,
}

impl<W> DemoApp<W>
where
    W: Window,
{
    pub fn new(window: W, window_size: WindowSize, opstions: Options) -> DemoApp<W> {
        let device = DeviceImpl::new(window.gl_version(), window.gl_default_framebuffer());

        let resources = window.resource_loader();

        let level = RendererLevel::D3D11;
        let viewport = window.viewport();
        let dest_framebuffer = DestFramebuffer::Default {
            viewport,
            window_size: window_size.device_size(),
        };
        let render_mode = RendererMode { level };
        let render_options = RendererOptions {
            dest: dest_framebuffer,
            background_color: None,
            show_debug_ui: true,
        };

        let filter = None;

        let viewport = window.viewport();
        let svg = load_scene(&opstions.input_path);

        let scene = build_svg_tree(&svg, viewport.size(), filter);
        if !scene.result_flags.is_empty() {
            log::warn!(
                "Warning: These features in the SVG are unsupported: {}.",
                scene.result_flags
            );
        }
        let mut scene = scene.scene;

        let renderer = Renderer::new(device, resources, render_mode, render_options);

        let scene_metadata = SceneMetadata::new_clipping_view_box(&mut scene, viewport.size());
        let camera = Camera::new(scene_metadata.view_box, viewport.size());

        let scene_proxy = SceneProxy::from_scene(scene, level, SequentialExecutor);

        DemoApp {
            window,
            window_size,

            render_transform: None,

            camera,

            scene_proxy,
            renderer,
        }
    }

    pub fn prepare_frame(&mut self) -> u32 {
        self.build_scene();

        self.prepare_frame_rendering()
    }

    fn build_scene(&mut self) {
        self.render_transform = Some(RenderTransform::Transform2D(self.camera.0));

        let build_options = BuildOptions {
            transform: self.render_transform.clone().unwrap(),
            dilation: Vector2F::zero(),
            subpixel_aa_enabled: false,
        };

        self.scene_proxy.build(build_options);
    }

    pub fn finish_drawing_frame(&mut self) {
        self.renderer.device().end_commands();
    }
}

#[derive(Clone)]
pub struct Options {
    pub input_path: PathBuf,
    pub high_performance_gpu: bool,
    pub renderer_level: Option<RendererLevel>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_path: PathBuf::from(""),
            high_performance_gpu: true,
            renderer_level: None,
        }
    }
}

fn load_scene(input_path: &PathBuf) -> SvgTree {
    let data: Vec<u8> = std::fs::read(input_path).unwrap();

    if let Ok(tree) = SvgTree::from_data(&data, &UsvgOptions::default()) {
        tree
    } else {
        panic!("can't load data");
    }
}

fn build_svg_tree(
    tree: &SvgTree,
    viewport_size: Vector2I,
    filter: Option<PatternFilter>,
) -> SVGScene {
    let mut scene = Scene::new();
    let filter_info = filter.map(|filter| {
        let scale = match filter {
            PatternFilter::Text {
                defringing_kernel: Some(_),
                ..
            } => vec2i(3, 1),
            _ => vec2i(1, 1),
        };
        let name = "Text".to_owned();
        let render_target_size = viewport_size * scale;
        let render_target = RenderTarget::new(render_target_size, name);
        let render_target_id = scene.push_render_target(render_target);
        FilterInfo {
            filter,
            render_target_id,
            render_target_size,
        }
    });

    let mut built_svg = SVGScene::from_tree_and_scene(tree, scene);
    if let Some(FilterInfo {
        filter,
        render_target_id,
        render_target_size,
    }) = filter_info
    {
        let mut pattern = Pattern::from_render_target(render_target_id, render_target_size);
        pattern.set_filter(Some(filter));
        let paint_id = built_svg.scene.push_paint(&Paint::from_pattern(pattern));

        let outline = Outline::from_rect(RectI::new(vec2i(0, 0), viewport_size).to_f32());
        let path = DrawPath::new(outline, paint_id);

        built_svg.scene.pop_render_target();
        built_svg.scene.push_draw_path(path);
    }

    return built_svg;

    struct FilterInfo {
        filter: PatternFilter,
        render_target_id: RenderTargetId,
        render_target_size: Vector2I,
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