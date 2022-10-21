use gl::{self, types::GLuint};
use glutin::dpi::PhysicalSize;
use glutin::event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::window::WindowBuilder;
use glutin::{ContextBuilder, GlProfile, GlRequest, PossiblyCurrent, WindowedContext};
use pathfinder_geometry::{rect::RectI, vector::vec2i};
use pathfinder_gl::{GLDevice, GLVersion};
use pathfinder_resources::{fs::FilesystemResourceLoader, ResourceLoader};
use pi_svg::{
    window::{Window, WindowSize},
    DemoApp, Options,
};
use std::path::PathBuf;

const WINDOW_WIDTH: u32 = 1024;
const WINDOW_HEIGHT: u32 = 768;

fn main() {
    pretty_env_logger::init();

    let (window, event_loop) = WindowImpl::new();
    let window_size = window.size();

    let options = Options {
        input_path: PathBuf::from("./examples/Ghostscript_Tiger.svg"),
        ..Default::default()
    };
    let app = DemoApp::new(window, window_size, options);

    run_loop(app, event_loop);
}

struct WindowImpl {
    render_context: WindowedContext<PossiblyCurrent>,
    resource_loader: FilesystemResourceLoader,
}

impl Window for WindowImpl {
    fn gl_version(&self) -> GLVersion {
        GLVersion::GL4
    }

    fn viewport(&self) -> RectI {
        let WindowSize {
            logical_size,
            backing_scale_factor,
        } = self.size();
        let size = (logical_size.to_f32() * backing_scale_factor).to_i32();
        RectI::new(vec2i(0, 0), size)
    }

    fn present(&mut self, _: &mut GLDevice) {
        self.render_context.swap_buffers().unwrap();
    }

    fn resource_loader(&self) -> &dyn ResourceLoader {
        &self.resource_loader
    }

    fn gl_default_framebuffer(&self) -> GLuint {
        0
    }
}

impl WindowImpl {
    fn new() -> (WindowImpl, EventLoop<()>) {
        let event_loop = EventLoop::new();

        let physical_window_size = PhysicalSize::new(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64);

        let window_builder = WindowBuilder::new()
            .with_title("Minimal example")
            .with_inner_size(physical_window_size);

        let render_context = ContextBuilder::new()
            .with_gl(GlRequest::Latest)
            .with_gl_profile(GlProfile::Core)
            .build_windowed(window_builder, &event_loop)
            .unwrap();

        // Load OpenGL, and make the context current.
        let render_context = unsafe { render_context.make_current().unwrap() };
        gl::load_with(|name| render_context.get_proc_address(name) as *const _);

        let resource_loader = FilesystemResourceLoader::locate();

        (
            WindowImpl {
                resource_loader,
                render_context,
            },
            event_loop,
        )
    }

    fn size(&self) -> WindowSize {
        let window = self.render_context.window();

        let (monitor, size) = (window.current_monitor().unwrap(), window.inner_size());

        WindowSize {
            logical_size: vec2i(size.width as i32, size.height as i32),
            backing_scale_factor: monitor.scale_factor() as f32,
        }
    }
}

fn run_loop(mut app: DemoApp<WindowImpl>, event_loop: EventLoop<()>) {
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::MainEventsCleared => {
                app.window.render_context.window().request_redraw();
            }
            Event::RedrawRequested(_) => {
                app.prepare_frame();

                app.draw_scene();
                app.begin_compositing();

                app.finish_drawing_frame();
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }
            | Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    },
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {
                *control_flow = ControlFlow::Poll;
            }
        };
    });
}
