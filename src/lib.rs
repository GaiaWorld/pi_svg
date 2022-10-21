// pathfinder/demo/common/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A demo app for Pathfinder.
use camera::Camera;
use concurrent::DemoExecutor;
use device::{GroundProgram, GroundVertexArray};
use pathfinder_content::effects::PatternFilter;
use pathfinder_content::effects::DEFRINGING_KERNEL_CORE_GRAPHICS;
use pathfinder_content::effects::STEM_DARKENING_FACTORS;
use pathfinder_content::outline::Outline;
use pathfinder_content::pattern::Pattern;
use pathfinder_content::render_target::RenderTargetId;
use pathfinder_export::{Export, FileFormat};
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::vector::{vec2f, vec2i, Vector2F, Vector2I};
use pathfinder_gl::GLDevice as DeviceImpl;
use pathfinder_gpu::Device;
use pathfinder_renderer::concurrent::scene_proxy::SceneProxy;
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererLevel};
use pathfinder_renderer::gpu::options::{RendererMode, RendererOptions};
use pathfinder_renderer::gpu::renderer::{DebugUIPresenterInfo, Renderer};
use pathfinder_renderer::options::{BuildOptions, RenderTransform};
use pathfinder_renderer::paint::Paint;
use pathfinder_renderer::scene::{DrawPath, RenderTarget, Scene};
use pathfinder_resources::ResourceLoader;
use pathfinder_svg::SVGScene;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use ui::{DemoUIModel, ScreenshotInfo, ScreenshotType};
use usvg::{Options as UsvgOptions, Tree as SvgTree};
use window::{Window, WindowSize};

use self::camera::Mode;

const APPROX_FONT_SIZE: f32 = 16.0;

mod camera;
mod concurrent;
mod device;
mod renderer;
mod ui;

pub mod window;

pub struct DemoApp<W>
where
    W: Window,
{
    pub window: W,
    pub should_exit: bool,
    pub options: Options,

    window_size: WindowSize,

    render_transform: Option<RenderTransform>,

    camera: Camera,
    frame_counter: u32,
    pending_screenshot_info: Option<ScreenshotInfo>,

    pub dirty: bool,

    current_frame: Option<Frame>,

    ui_model: DemoUIModel,
    
    scene_proxy: SceneProxy,
    renderer: Renderer<DeviceImpl>,

    scene_framebuffer: Option<<DeviceImpl as Device>::Framebuffer>,

    ground_program: GroundProgram<DeviceImpl>,
    ground_vertex_array: GroundVertexArray<DeviceImpl>,
}

impl<W> DemoApp<W>
where
    W: Window,
{
    pub fn new(window: W, window_size: WindowSize, options: Options) -> DemoApp<W> {
        let device = DeviceImpl::new(window.gl_version(), window.gl_default_framebuffer());

        let resources = window.resource_loader();

        // Set up the executor.
        let executor = DemoExecutor::new(options.jobs);

        let mut ui_model = DemoUIModel::new(&options);

        let level = match options.renderer_level {
            Some(level) => level,
            None => RendererLevel::default_for_device(&device),
        };
        let viewport = window.viewport(options.mode.view(0));
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

        let filter = build_filter(&ui_model);

        let viewport = window.viewport(options.mode.view(0));
        let mut svg = load_scene(resources, &options.input_path);

        let scene = build_svg_tree(&svg, viewport.size(), filter);
        let message = get_svg_building_message(&scene);
        let mut scene = scene.scene;

        let renderer = Renderer::new(device, resources, render_mode, render_options);

        let scene_metadata = SceneMetadata::new_clipping_view_box(&mut scene, viewport.size());
        let camera = Camera::new(options.mode, scene_metadata.view_box, viewport.size());

        let scene_proxy = SceneProxy::from_scene(scene, level, executor);

        let ground_program = GroundProgram::new(renderer.device(), resources);
        let ground_vertex_array = GroundVertexArray::new(
            renderer.device(),
            &ground_program,
            &renderer.quad_vertex_positions_buffer(),
            &renderer.quad_vertex_indices_buffer(),
        );

        // let ui_presenter = DemoUIPresenter::new(renderer.device(), resources);

        DemoApp {
            window,
            should_exit: false,
            options,

            window_size,

            render_transform: None,

            camera,
            frame_counter: 0,
            pending_screenshot_info: None,

            dirty: true,

            current_frame: None,

            ui_model,

            scene_proxy,
            renderer,

            scene_framebuffer: None,

            ground_program,
            ground_vertex_array,
        }
    }

    pub fn prepare_frame(&mut self) -> u32 {
        // Clear dirty flag.
        self.dirty = false;

        // Update the scene.
        self.build_scene();

        // Save the frame.
        //
        // FIXME(pcwalton): This is super ugly.
        let transform = self.render_transform.clone().unwrap();
        self.current_frame = Some(Frame::new(transform));

        // Prepare to render the frame.
        self.prepare_frame_rendering()
    }

    fn build_scene(&mut self) {
        self.render_transform = match self.camera {
            Camera::TwoD(transform) => Some(RenderTransform::Transform2D(transform)),
        };

        let build_options = BuildOptions {
            transform: self.render_transform.clone().unwrap(),
            dilation: if self.ui_model.stem_darkening_effect_enabled {
                let font_size = APPROX_FONT_SIZE * self.window_size.backing_scale_factor;
                vec2f(STEM_DARKENING_FACTORS[0], STEM_DARKENING_FACTORS[1]) * font_size
            } else {
                Vector2F::zero()
            },
            subpixel_aa_enabled: self.ui_model.subpixel_aa_effect_enabled,
        };

        self.scene_proxy.build(build_options);
    }

    pub fn finish_drawing_frame(&mut self) {
        self.maybe_take_screenshot();

        let frame = self.current_frame.take().unwrap();

        if self.options.ui == UIVisibility::All {
            let DebugUIPresenterInfo {
                device,
                allocator,
                debug_ui_presenter,
            } = self.renderer.debug_ui_presenter_mut();
        }

        self.renderer.device().end_commands();

        self.window.present(self.renderer.device_mut());
        self.frame_counter += 1;
    }

    fn maybe_take_screenshot(&mut self) {
        match self.pending_screenshot_info.take() {
            None => {}
            Some(ScreenshotInfo {
                kind: ScreenshotType::PNG,
                path,
            }) => self.take_raster_screenshot(path),
            Some(ScreenshotInfo {
                kind: ScreenshotType::SVG,
                path,
            }) => {
                // FIXME(pcwalton): This won't work on Android.
                let mut writer = BufWriter::new(File::create(path).unwrap());
                self.scene_proxy
                    .copy_scene()
                    .export(&mut writer, FileFormat::SVG)
                    .unwrap();
            }
        }
    }
}

#[derive(Clone)]
pub struct Options {
    pub jobs: Option<usize>,
    pub mode: Mode,
    pub input_path: PathBuf,
    pub ui: UIVisibility,
    pub background_color: BackgroundColor,
    pub high_performance_gpu: bool,
    pub renderer_level: Option<RendererLevel>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            jobs: None,
            mode: Mode::TwoD,
            input_path: PathBuf::from(""),
            ui: UIVisibility::None,
            background_color: BackgroundColor::Light,
            high_performance_gpu: true,
            renderer_level: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum UIVisibility {
    None,
    Stats,
    All,
}

fn load_scene(resource_loader: &dyn ResourceLoader, input_path: &PathBuf) -> SvgTree {
    let data: Vec<u8> = std::fs::read(input_path).unwrap().into();

    if let Ok(tree) = SvgTree::from_data(&data, &UsvgOptions::default()) {
        tree
    } else {
        panic!("can't load data");
    }
}

// FIXME(pcwalton): Rework how transforms work in the demo. The transform affects the final
// composite steps, breaking this approach.
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

    let mut built_svg = SVGScene::from_tree_and_scene(&tree, scene);
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

fn center_of_window(window_size: &WindowSize) -> Vector2F {
    window_size.device_size().to_f32() * 0.5
}

fn get_svg_building_message(built_svg: &SVGScene) -> String {
    if built_svg.result_flags.is_empty() {
        return String::new();
    }
    format!(
        "Warning: These features in the SVG are unsupported: {}.",
        built_svg.result_flags
    )
}

struct Frame {
    transform: RenderTransform,
}

impl Frame {
    fn new(transform: RenderTransform) -> Frame {
        Frame { transform }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BackgroundColor {
    Light = 0,
    Dark = 1,
    Transparent = 2,
}

impl BackgroundColor {
    fn as_str(&self) -> &'static str {
        match *self {
            BackgroundColor::Light => "Light",
            BackgroundColor::Dark => "Dark",
            BackgroundColor::Transparent => "Transparent",
        }
    }
}

struct SceneMetadata {
    view_box: RectF,
}

impl SceneMetadata {
    // FIXME(pcwalton): The fact that this mutates the scene is really ugly!
    // Can we simplify this?
    fn new_clipping_view_box(scene: &mut Scene, viewport_size: Vector2I) -> SceneMetadata {
        let view_box = scene.view_box();
        scene.set_view_box(RectF::new(Vector2F::zero(), viewport_size.to_f32()));
        SceneMetadata { view_box }
    }
}

fn build_filter(ui_model: &DemoUIModel) -> Option<PatternFilter> {
    if !ui_model.gamma_correction_effect_enabled && !ui_model.subpixel_aa_effect_enabled {
        return None;
    }

    Some(PatternFilter::Text {
        fg_color: ui_model.foreground_color().to_f32(),
        bg_color: ui_model.background_color().to_f32(),
        gamma_correction: ui_model.gamma_correction_effect_enabled,
        defringing_kernel: if ui_model.subpixel_aa_effect_enabled {
            // TODO(pcwalton): Select FreeType defringing kernel as necessary.
            Some(DEFRINGING_KERNEL_CORE_GRAPHICS)
        } else {
            None
        },
    })
}
