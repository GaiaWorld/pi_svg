#[macro_use]
extern crate lazy_static;

use euclid::default::Size2D;
use gl::{self, types::GLuint};
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::vector::{vec2i, Vector2I};
use pathfinder_resources::fs::FilesystemResourceLoader;
use pathfinder_resources::ResourceLoader;
use pi_svg::window::{DataPath, Event, Keycode, View, Window, WindowSize};
use pi_svg::{DemoApp, Options};
use std::cell::Cell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;
use surfman::{declare_surfman, SurfaceAccess, SurfaceType};
use surfman::{Connection, Context, ContextAttributeFlags, ContextAttributes};
use surfman::{Device, GLVersion as SurfmanGLVersion};
use winit::dpi::LogicalSize;
use winit::{ControlFlow, ElementState, Event as WinitEvent, EventsLoop, EventsLoopProxy};
use winit::{MouseButton, VirtualKeyCode, Window as WinitWindow, WindowBuilder, WindowEvent};

declare_surfman!();

use pathfinder_gl::{GLDevice, GLVersion};

const DEFAULT_WINDOW_WIDTH: u32 = 1024 ;
const DEFAULT_WINDOW_HEIGHT: u32 = 768;

lazy_static! {
    static ref EVENT_QUEUE: Mutex<Option<EventQueue>> = Mutex::new(None);
}

fn main() {
    color_backtrace::install();
    pretty_env_logger::init();

    // Read command line options.
    let mut options = Options::default();
    options.command_line_overrides();

    let window = WindowImpl::new(&options);
    let window_size = window.size();

    let mut app = DemoApp::new(window, window_size, options);

    while !app.should_exit {
        let mut events = vec![];
        if !app.dirty {
            events.push(app.window.get_event());
        }
        while let Some(event) = app.window.try_get_event() {
            events.push(event);
        }

        let scene_count = app.prepare_frame(events);

        app.draw_scene();
        app.begin_compositing();
        for scene_index in 0..scene_count {
            app.composite_scene(scene_index);
        }
        app.finish_drawing_frame();
    }
}

struct WindowImpl {
    window: WinitWindow,

    context: Context,

    #[allow(dead_code)]
    connection: Connection,

    device: Device,

    event_loop: EventsLoop,
    pending_events: VecDeque<Event>,
    mouse_position: Vector2I,
    mouse_down: bool,
    next_user_event_id: Cell<u32>,

    #[allow(dead_code)]
    resource_loader: FilesystemResourceLoader,
}

struct EventQueue {
    event_loop_proxy: EventsLoopProxy,
    pending_custom_events: VecDeque<CustomEvent>,
}

#[derive(Clone)]
enum CustomEvent {
    User {
        message_type: u32,
        message_data: u32,
    },
    OpenData(PathBuf),
}

impl Window for WindowImpl {
    fn gl_version(&self) -> GLVersion {
        GLVersion::GL4
    }

    fn gl_default_framebuffer(&self) -> GLuint {
        self.device
            .context_surface_info(&self.context)
            .unwrap()
            .unwrap()
            .framebuffer_object
    }

    fn viewport(&self, view: View) -> RectI {
        let WindowSize {
            logical_size,
            backing_scale_factor,
        } = self.size();
        let mut size = (logical_size.to_f32() * backing_scale_factor).to_i32();
        let mut x_offset = 0;
        if let View::Stereo(index) = view {
            size.set_x(size.x() / 2);
            x_offset = size.x() * (index as i32);
        }
        RectI::new(vec2i(x_offset, 0), size)
    }

    fn make_current(&mut self, _view: View) {
        self.device.make_context_current(&self.context).unwrap();
    }

    fn present(&mut self, _: &mut GLDevice) {
        let mut surface = self
            .device
            .unbind_surface_from_context(&mut self.context)
            .unwrap()
            .unwrap();
        self.device
            .present_surface(&mut self.context, &mut surface)
            .unwrap();
        self.device
            .bind_surface_to_context(&mut self.context, surface)
            .unwrap();
    }

    fn resource_loader(&self) -> &dyn ResourceLoader {
        &self.resource_loader
    }

    fn present_open_svg_dialog(&mut self) {
        unimplemented!()
    }

    fn run_save_dialog(&self, extension: &str) -> Result<PathBuf, ()> {
        unimplemented!()
    }

    fn create_user_event_id(&self) -> u32 {
        let id = self.next_user_event_id.get();
        self.next_user_event_id.set(id + 1);
        id
    }

    fn push_user_event(message_type: u32, message_data: u32) {
        let mut event_queue = EVENT_QUEUE.lock().unwrap();
        let event_queue = event_queue.as_mut().unwrap();
        event_queue
            .pending_custom_events
            .push_back(CustomEvent::User {
                message_type,
                message_data,
            });
        drop(event_queue.event_loop_proxy.wakeup());
    }
}

impl WindowImpl {
    #[cfg(any(not(target_os = "macos"), feature = "pf-gl"))]
    fn new(options: &Options) -> WindowImpl {
        let event_loop = EventsLoop::new();
        let window_size = Size2D::new(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT);
        let logical_size = LogicalSize::new(window_size.width as f64, window_size.height as f64);
        let window = WindowBuilder::new()
            .with_title("Pathfinder Demo")
            .with_dimensions(logical_size)
            .build(&event_loop)
            .unwrap();
        window.show();

        let connection = Connection::from_winit_window(&window).unwrap();
        let native_widget = connection
            .create_native_widget_from_winit_window(&window)
            .unwrap();

        let adapter = if options.high_performance_gpu {
            connection.create_hardware_adapter().unwrap()
        } else {
            connection.create_low_power_adapter().unwrap()
        };

        let mut device = connection.create_device(&adapter).unwrap();

        let context_attributes = ContextAttributes {
            version: SurfmanGLVersion::new(3, 0),
            flags: ContextAttributeFlags::ALPHA,
        };
        let context_descriptor = device
            .create_context_descriptor(&context_attributes)
            .unwrap();

        let surface_type = SurfaceType::Widget { native_widget };
        let mut context = device.create_context(&context_descriptor).unwrap();
        let surface = device
            .create_surface(&context, SurfaceAccess::GPUOnly, surface_type)
            .unwrap();
        device
            .bind_surface_to_context(&mut context, surface)
            .unwrap();
        device.make_context_current(&context).unwrap();

        gl::load_with(|symbol_name| device.get_proc_address(&context, symbol_name));

        let resource_loader = FilesystemResourceLoader::locate();

        *EVENT_QUEUE.lock().unwrap() = Some(EventQueue {
            event_loop_proxy: event_loop.create_proxy(),
            pending_custom_events: VecDeque::new(),
        });

        WindowImpl {
            window,
            event_loop,
            connection,
            context,
            device,
            next_user_event_id: Cell::new(0),
            pending_events: VecDeque::new(),
            mouse_position: vec2i(0, 0),
            mouse_down: false,
            resource_loader,
        }
    }

    fn window(&self) -> &WinitWindow {
        &self.window
    }

    fn size(&self) -> WindowSize {
        let window = self.window();
        let (monitor, size) = (
            window.get_current_monitor(),
            window.get_inner_size().unwrap(),
        );

        WindowSize {
            logical_size: vec2i(size.width as i32, size.height as i32),
            backing_scale_factor: monitor.get_hidpi_factor() as f32,
        }
    }

    fn get_event(&mut self) -> Event {
        if self.pending_events.is_empty() {
            let window = &self.window;
            let mouse_position = &mut self.mouse_position;
            let mouse_down = &mut self.mouse_down;
            let pending_events = &mut self.pending_events;
            self.event_loop.run_forever(|winit_event| {
                //println!("blocking {:?}", winit_event);
                match convert_winit_event(winit_event, window, mouse_position, mouse_down) {
                    Some(event) => {
                        //println!("handled");
                        pending_events.push_back(event);
                        ControlFlow::Break
                    }
                    None => ControlFlow::Continue,
                }
            });
        }

        self.pending_events.pop_front().expect("Where's the event?")
    }

    fn try_get_event(&mut self) -> Option<Event> {
        if self.pending_events.is_empty() {
            let window = &self.window;
            let mouse_position = &mut self.mouse_position;
            let mouse_down = &mut self.mouse_down;
            let pending_events = &mut self.pending_events;
            self.event_loop.poll_events(|winit_event| {
                //println!("nonblocking {:?}", winit_event);
                if let Some(event) =
                    convert_winit_event(winit_event, window, mouse_position, mouse_down)
                {
                    //println!("handled");
                    pending_events.push_back(event);
                }
            });
        }
        self.pending_events.pop_front()
    }
}

fn convert_winit_event(
    winit_event: WinitEvent,
    window: &WinitWindow,
    mouse_position: &mut Vector2I,
    mouse_down: &mut bool,
) -> Option<Event> {
    match winit_event {
        WinitEvent::Awakened => {
            let mut event_queue = EVENT_QUEUE.lock().unwrap();
            let event_queue = event_queue.as_mut().unwrap();
            match event_queue
                .pending_custom_events
                .pop_front()
                .expect("`Awakened` with no pending custom event!")
            {
                CustomEvent::OpenData(data_path) => {
                    Some(Event::OpenData(DataPath::Path(data_path)))
                }
                CustomEvent::User {
                    message_data,
                    message_type,
                } => Some(Event::User {
                    message_data,
                    message_type,
                }),
            }
        }
        WinitEvent::WindowEvent {
            event: window_event,
            ..
        } => match window_event {
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                *mouse_down = true;
                Some(Event::MouseDown(*mouse_position))
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                *mouse_down = false;
                None
            }
            WindowEvent::CursorMoved { position, .. } => {
                *mouse_position = vec2i(position.x as i32, position.y as i32);
                if *mouse_down {
                    Some(Event::MouseDragged(*mouse_position))
                } else {
                    Some(Event::MouseMoved(*mouse_position))
                }
            }
            WindowEvent::KeyboardInput { input, .. } => input
                .virtual_keycode
                .and_then(|virtual_keycode| match virtual_keycode {
                    VirtualKeyCode::Escape => Some(Keycode::Escape),
                    VirtualKeyCode::Tab => Some(Keycode::Tab),
                    virtual_keycode => {
                        let vk = virtual_keycode as u32;
                        let vk_a = VirtualKeyCode::A as u32;
                        let vk_z = VirtualKeyCode::Z as u32;
                        if vk >= vk_a && vk <= vk_z {
                            let character = ((vk - vk_a) + 'A' as u32) as u8;
                            Some(Keycode::Alphanumeric(character))
                        } else {
                            None
                        }
                    }
                })
                .map(|keycode| match input.state {
                    ElementState::Pressed => Event::KeyDown(keycode),
                    ElementState::Released => Event::KeyUp(keycode),
                }),
            WindowEvent::CloseRequested => Some(Event::Quit),
            WindowEvent::Resized(new_size) => {
                let logical_size = vec2i(new_size.width as i32, new_size.height as i32);
                let backing_scale_factor = window.get_current_monitor().get_hidpi_factor() as f32;
                Some(Event::WindowResized(WindowSize {
                    logical_size,
                    backing_scale_factor,
                }))
            }
            _ => None,
        },
        _ => None,
    }
}
