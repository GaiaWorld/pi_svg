use euclid::default::Size2D;
use gl::{self, types::GLuint};
use pathfinder_geometry::{rect::RectI, vector::vec2i};
use pathfinder_resources::{fs::FilesystemResourceLoader, ResourceLoader};
use pi_svg::{
    window::{View, Window, WindowSize},
    DemoApp, Options,
};
use std::path::PathBuf;
use surfman::{
    declare_surfman, Connection, Context, ContextAttributeFlags, ContextAttributes, Device,
    GLVersion as SurfmanGLVersion, SurfaceAccess, SurfaceType,
};
use winit::{dpi::LogicalSize, EventsLoop, Window as WinitWindow, WindowBuilder};
use pathfinder_gl::{GLDevice, GLVersion};

const DEFAULT_WINDOW_WIDTH: u32 = 1024;
const DEFAULT_WINDOW_HEIGHT: u32 = 768;

declare_surfman!();

fn main() {
    color_backtrace::install();
    pretty_env_logger::init();

    // Read command line options.
    let mut options = Options::default();
    options.input_path = PathBuf::from("./examples/Ghostscript_Tiger.svg");

    let window = WindowImpl::new(&options);
    let window_size = window.size();

    let mut app = DemoApp::new(window, window_size, options);

    while !app.should_exit {
        let scene_count = app.prepare_frame();

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
    event_loop: EventsLoop,

    device: Device,
    context: Context,
    connection: Connection,

    resource_loader: FilesystemResourceLoader,
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
}

impl WindowImpl {
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

        WindowImpl {
            window,
            event_loop,
            connection,
            context,
            device,
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
}
