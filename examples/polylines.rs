use glutin::dpi::PhysicalSize;
use glutin::event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::window::WindowBuilder;
use glutin::{ContextBuilder, GlProfile, GlRequest, PossiblyCurrent, WindowedContext};
use pi_svg::SvgRenderer;
use std::rc::Rc;
use std::time::{Duration, Instant};

const WINDOW_WIDTH: u32 = 1920;
const WINDOW_HEIGHT: u32 = 1080;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let (window, event_loop) = WindowImpl::new();

    let (w, h) = window.get_device_size();

    let mut scene = Scene::new(WINDOW_WIDTH, WINDOW_HEIGHT);
    let shader = Rc::new(Shader::new(VS_SOURCE, FS_SOURCE));

    let data = [
        (
            std::fs::read("./examples/polylines/1.svg").unwrap(),
            scene.create_fbo(WINDOW_WIDTH, WINDOW_HEIGHT),
        ),
        (
            std::fs::read("./examples/polylines/2.svg").unwrap(),
            scene.create_fbo(WINDOW_WIDTH, WINDOW_HEIGHT),
        ),
        (
            std::fs::read("./examples/polylines/3.svg").unwrap(),
            scene.create_fbo(WINDOW_WIDTH, WINDOW_HEIGHT),
        ),
    ];

    scene.create_quad(
        data[0].1.texture.clone(),
        (-0.6, -0.6),
        (0.5, 0.5),
        shader.clone(),
    );
    scene.create_quad(
        data[1].1.texture.clone(),
        (0.1, 0.1),
        (0.5, 0.5),
        shader.clone(),
    );
    scene.create_quad(
        data[2].1.texture.clone(),
        (-0.6, 0.1),
        (0.5, 0.5),
        shader.clone(),
    );

    let mut svg = SvgRenderer::default();

    run_loop(window, svg, scene, data, event_loop);
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

fn run_loop(
    window: WindowImpl,
    mut svg: SvgRenderer,
    scene: Scene,
    data: [(Vec<u8>, Fbo); 3],
    event_loop: EventLoop<()>,
) {
    let mut frame = 0;
    let mut tm = Instant::now();

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

                for i in 0..3 {
                    let (data, fbo) = &data[i];

                    let svg_key = i as u32 + 1;
                    svg.load_svg(svg_key, data.as_slice()).unwrap();

                    svg.set_target(fbo.fbo, WINDOW_WIDTH as i32, WINDOW_HEIGHT as i32);
                    svg.set_clear_color(1.0, 1.0, 0.0, 1.0);

                    svg.set_viewport(0, 0, None);
                    svg.draw_once(svg_key).unwrap();
                }

                scene.clear();
                scene.render();

                window.0.swap_buffers().unwrap();
            }
            _ => {}
        };
    });
}

struct Scene {
    width: u32,
    height: u32,

    meshes: Vec<Mesh>,
}

impl Scene {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            meshes: vec![],
        }
    }

    fn create_quad(
        &mut self,
        texture: Rc<u32>,
        orgin: (f32, f32),
        size: (f32, f32),
        shader: Rc<Shader>,
    ) {
        let (x, y) = orgin;
        let (w, h) = size;

        #[rustfmt::skip]
        let quad: [f32; 16] = [
            // pos2, texcoord
            x, y,  0.0,  0.0,
            x + w, y,  1.0,  0.0,
            x + w,  y + h,  1.0,  1.0,
            x,  y + h,  0.0,  1.0,
        ];

        let indices = [0, 1, 2, 0, 2, 3];

        let m = Mesh::new(quad.as_slice(), indices.as_slice(), texture, shader);
        self.meshes.push(m);
    }

    fn create_fbo(&self, w: u32, h: u32) -> Fbo {
        Fbo::new(w, h)
    }

    fn clear(&self) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

            gl::Viewport(0, 0, WINDOW_WIDTH as i32, WINDOW_HEIGHT as i32);
            gl::Scissor(0, 0, WINDOW_WIDTH as i32, WINDOW_HEIGHT as i32);

            gl::ClearColor(0.0, 0.0, 1.0, 1.0);
            gl::ClearDepthf(1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        }
    }

    fn render(&self) {
        for mesh in self.meshes.iter() {
            mesh.draw();
        }
    }
}

struct Fbo {
    fbo: u32,
    texture: Rc<u32>,
}

impl Fbo {
    fn new(w: u32, h: u32) -> Self {
        unsafe {
            let texture = Self::create_texture(w, h);

            let mut fbo = std::mem::zeroed();
            gl::GenFramebuffers(1, &mut fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);

            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                texture,
                0,
            );

            let mut rbo = std::mem::zeroed();
            gl::GenRenderbuffers(1, &mut rbo);
            gl::BindRenderbuffer(gl::RENDERBUFFER, rbo);
            gl::RenderbufferStorage(gl::RENDERBUFFER, gl::DEPTH24_STENCIL8, w as i32, h as i32);
            gl::FramebufferRenderbuffer(
                gl::FRAMEBUFFER,
                gl::DEPTH_STENCIL_ATTACHMENT,
                gl::RENDERBUFFER,
                rbo,
            );

            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

            Self {
                texture: Rc::new(texture),
                fbo,
            }
        }
    }

    unsafe fn create_texture(w: u32, h: u32) -> u32 {
        let mut texture = std::mem::zeroed();
        gl::GenTextures(1, &mut texture);

        gl::ActiveTexture(gl::TEXTURE0);
        gl::BindTexture(gl::TEXTURE_2D, texture);

        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);

        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);

        let data = vec![0u8; w as usize * h as usize * 4];
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGBA as i32,
            w as i32,
            h as i32,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            data.as_ptr() as *const _,
        );
        texture
    }
}

struct Shader {
    program: u32,

    pos_attrib: i32,
    texcoord_attrib: i32,
}

impl Shader {
    fn new(vs: &[u8], fs: &[u8]) -> Self {
        unsafe {
            let vs = Self::create_shader(gl::VERTEX_SHADER, vs);
            let fs = Self::create_shader(gl::FRAGMENT_SHADER, fs);

            let program = gl::CreateProgram();

            gl::AttachShader(program, vs);
            gl::AttachShader(program, fs);

            gl::LinkProgram(program);

            let mut linked = std::mem::zeroed();
            gl::GetProgramiv(program, gl::LINK_STATUS, &mut linked);
            assert!(linked != 0);

            gl::UseProgram(program);

            let sampler = gl::GetUniformLocation(program, b"sampler\0".as_ptr() as *const _);
            gl::Uniform1i(sampler, 0);

            gl::DeleteShader(vs);
            gl::DeleteShader(fs);

            let pos_attrib = gl::GetAttribLocation(program, b"position\0".as_ptr() as *const _);

            let texcoord_attrib =
                gl::GetAttribLocation(program, b"texcoord\0".as_ptr() as *const _);

            Self {
                program,
                texcoord_attrib,
                pos_attrib,
            }
        }
    }

    unsafe fn create_shader(shader: gl::types::GLenum, source: &[u8]) -> gl::types::GLuint {
        let shader = gl::CreateShader(shader);
        gl::ShaderSource(
            shader,
            1,
            [source.as_ptr().cast()].as_ptr(),
            std::ptr::null(),
        );
        gl::CompileShader(shader);
        shader
    }
}

struct Mesh {
    vbo: Rc<u32>,

    ibo: Rc<u32>,
    indices_len: u32,

    texture: Rc<u32>,

    shader: Rc<Shader>,
}

impl Mesh {
    pub fn new(vertex: &[f32], indices: &[u16], texture: Rc<u32>, shader: Rc<Shader>) -> Self {
        unsafe {
            let mut vbo = std::mem::zeroed();
            gl::GenBuffers(1, &mut vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (vertex.len() * std::mem::size_of::<f32>()) as gl::types::GLsizeiptr,
                vertex.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );

            let indices_len = indices.len();
            let mut ibo = std::mem::zeroed();
            gl::GenBuffers(1, &mut ibo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ibo);
            gl::BufferData(
                gl::ELEMENT_ARRAY_BUFFER,
                (indices_len * std::mem::size_of::<u16>()) as gl::types::GLsizeiptr,
                indices.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );

            Self {
                vbo: Rc::new(vbo),
                ibo: Rc::new(ibo),
                indices_len: indices_len as u32,

                texture,
                shader,
            }
        }
    }

    fn get_indices_len(&self) -> u32 {
        self.indices_len
    }

    fn draw(&self) {
        let shader = &self.shader;
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.texture.as_ref().clone());

            gl::UseProgram(shader.program);

            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo.as_ref().clone());

            gl::EnableVertexAttribArray(shader.pos_attrib as gl::types::GLuint);
            gl::VertexAttribPointer(
                shader.pos_attrib as gl::types::GLuint,
                2,
                gl::FLOAT,
                0,
                4 * std::mem::size_of::<f32>() as gl::types::GLsizei,
                std::ptr::null(),
            );

            gl::EnableVertexAttribArray(shader.texcoord_attrib as gl::types::GLuint);
            gl::VertexAttribPointer(
                shader.texcoord_attrib as gl::types::GLuint,
                2,
                gl::FLOAT,
                0,
                4 * std::mem::size_of::<f32>() as gl::types::GLsizei,
                (2 * std::mem::size_of::<f32>()) as *const () as *const _,
            );

            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.ibo.as_ref().clone());
            gl::DrawElements(
                gl::TRIANGLES,
                self.get_indices_len() as i32,
                gl::UNSIGNED_SHORT,
                std::ptr::null(),
            );
        }
    }
}

const VS_SOURCE: &[u8] = b"
#version 100
precision mediump float;

attribute vec2 position;
attribute vec2 texcoord;

varying vec2 v_texcoord;

void main() {
    v_texcoord = texcoord;
    gl_Position = vec4(position, 0.6, 1.0);
}
\0";

const FS_SOURCE: &[u8] = b"
#version 100
precision mediump float;

varying vec2 v_texcoord;

uniform sampler2D sampler;

void main() {
    gl_FragColor = vec4(texture2D(sampler, v_texcoord).rgb, 1.0);
}
\0";
