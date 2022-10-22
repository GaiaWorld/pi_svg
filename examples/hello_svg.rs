use glutin::dpi::PhysicalSize;
use glutin::event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::window::WindowBuilder;
use glutin::{ContextBuilder, GlProfile, GlRequest, PossiblyCurrent, WindowedContext};
use pi_svg::SvgRenderer;

const WINDOW_WIDTH: u32 = 1024;
const WINDOW_HEIGHT: u32 = 768;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let (window, event_loop) = WindowImpl::new();

    let (w, h) = window.get_device_size();
    let mut app = SvgRenderer::new(0, w, h, (100, 300),  None);

    app.set_clear_color(1.0, 1.0, 0.0, 0.0);

    let data: Vec<u8> = std::fs::read("./examples/Ghostscript_Tiger.svg").unwrap();
    app.load_svg(data.as_slice()).unwrap();

    run_loop(window, app, event_loop);
}

struct WindowImpl(WindowedContext<PossiblyCurrent>);

impl WindowImpl {
    fn new() -> (WindowImpl, EventLoop<()>) {
        let event_loop = EventLoop::new();

        let window_builder = WindowBuilder::new()
            .with_title("Minimal example")
            .with_inner_size(PhysicalSize::new(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64));

        let render_context = ContextBuilder::new()
            .with_gl(GlRequest::Latest)
            .with_gl_profile(GlProfile::Core)
            .build_windowed(window_builder, &event_loop)
            .unwrap();

        let render_context = unsafe { render_context.make_current().unwrap() };
        gl::load_with(|name| render_context.get_proc_address(name) as *const _);

        (WindowImpl(render_context), event_loop)
    }

    fn get_device_size(&self) -> (i32, i32) {
        let window = self.0.window();

        let monitor = window.current_monitor().unwrap();
        let logical_size = window.inner_size();

        let backing_scale_factor = monitor.scale_factor() as f32;

        let w = logical_size.width as f32 * backing_scale_factor;
        let h = logical_size.height as f32 * backing_scale_factor;

        (w as i32, h as i32)
    }
}

fn run_loop(window: WindowImpl, mut app: SvgRenderer, event_loop: EventLoop<()>) {
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::MainEventsCleared => {
                window.0.window().request_redraw();
            }
            Event::RedrawRequested(_) => {
                let (w, h) = window.get_device_size();

                app.draw_once(Some((w, h))).unwrap();

                window.0.swap_buffers().unwrap();
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