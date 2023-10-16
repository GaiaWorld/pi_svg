use std::time::{Duration, Instant};

use glutin::dpi::PhysicalSize;
use glutin::event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::window::WindowBuilder;
use glutin::{ContextBuilder, GlProfile, GlRequest, PossiblyCurrent, WindowedContext};
use pi_svg::SvgRenderer;

const WINDOW_WIDTH: u32 = 1080;
const WINDOW_HEIGHT: u32 = 720;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let (window, event_loop) = WindowImpl::new();

    run_loop(window, event_loop);
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

        // 测试 不同版本 的 gl 导致的问题
        gl_old::load_with(|name| render_context.get_proc_address(name) as *const _);

        SvgRenderer::load_gl_with(|name| render_context.get_proc_address(name) as *const _);

        (WindowImpl(render_context), event_loop)
    }

    fn _get_device_size(&self) -> (i32, i32) {
        let window = self.0.window();

        let monitor = window.current_monitor().unwrap();
        let logical_size = window.inner_size();

        let backing_scale_factor = monitor.scale_factor() as f32;

        let w = logical_size.width as f32 * backing_scale_factor;
        let h = logical_size.height as f32 * backing_scale_factor;

        (w as i32, h as i32)
    }
}

fn run_loop(window: WindowImpl, event_loop: EventLoop<()>) {
    let mut frame = 0;
    let mut tm = Instant::now();
    let mut x = 0;

    let mut svg = SvgRenderer::default();
    let data: Vec<u8> = std::fs::read("./examples/Ghostscript_Tiger.svg").unwrap();

    // let mut r = 0.0;
    let count = 1;

    let b = Instant::now();
    // for _ in 0..count {
        // let scene = svg.load_svg(data.as_slice()).unwrap();
        // r += scene.view_box().origin_x();
    // }
    let total = b.elapsed().as_millis() as f32;
    println!(
        "load_svg: examples/Ghostscript_Tiger, count = {}, total time = {} ms, avg time = {} ms",
        count,
        total,
        total / count as f32,
    );

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
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
            Event::MainEventsCleared => {
                window.0.window().request_redraw();
            }
            Event::RedrawRequested(_) => {
                frame += 1;

                let now = Instant::now();
                let d = now.duration_since(tm);
                if d >= Duration::from_secs(1) {
                    println!("fps: {}", 1000.0 * frame as f32 / d.as_millis() as f32);

                    frame = 0;
                    tm = now;
                }

                let t = 0.004 * d.as_millis() as f32;
                x += t as i32;
                if x > 100 {
                    x = 0;
                }

                let scene: pi_svg::Scene = svg.load_svg(data.as_slice()).unwrap();

                svg.set_target(0, 1920, 1080);
                svg.set_viewport(x, 0, Some((1080, 720)));
                svg.set_clear_color(0.0, 1.0, 0.0, 0.0);

                svg.draw_once(&scene).unwrap();

                window.0.swap_buffers().unwrap();
            }
            _ => {}
        };
    });
}
