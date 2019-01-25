#[macro_use] // 2018-style macro imports don't work so well
extern crate gfx;

use gfx::{
    Device,
};

use gfx::traits::FactoryExt;

use glutin::{
    Event,
    KeyboardInput,
    WindowEvent,
};

type ColorFmt = gfx::format::Rgba8;
type DepthFmt = gfx::format::DepthStencil;

gfx_defines! {
    vertex Vertex {
        pos: [f32; 3] = "a_Pos",
        color: [f32; 3] = "a_Color",
    }

    constant Locals {
        transform: [[f32; 4]; 4] = "u_Transform",
        highest_dim: f32 = "u_HighestDim",
        lacunarity: f32 = "u_Lacunarity",
        octaves: f32 = "u_Octaves",
        offset: f32 = "u_Offset",
        gain: f32 = "u_Gain",
    }

    pipeline pipe {
        vertex_buffer: gfx::VertexBuffer<Vertex> = (),
        locals: gfx::ConstantBuffer<Locals> = "Locals",
        main_color: gfx::RenderTarget<ColorFmt> = "Target0",
        main_depth: gfx::DepthTarget<DepthFmt> =
            gfx::preset::depth::LESS_EQUAL_WRITE,
    }
}

pub const CLEAR_COLOR: [f32; 4] = [0.1, 0.2, 0.3, 1.0];

pub struct Mesh {
    pub vertex_data: Vec<Vertex>,
    pub index_data: Vec<u16>,
}

impl Mesh {
    pub fn new_icosphere() -> Self {
        use genmesh::generators::*;

        let sphere = IcoSphere::subdivide(4);

        let color = [1.0, 1.0, 1.0];

        let vertex_data = sphere.shared_vertex_iter().map(|v| {
            let pos: [f32; 3] = v.pos.into();
            //pos.iter_mut().for_each(|n| *n /= 2.0);
            Vertex { pos, color }
        }).collect();

        let index_data = sphere.indexed_polygon_iter().flat_map(|t| {
            [t.x, t.y, t.z].to_vec().into_iter()
        }).map(|u| u as u16).collect();

        Self { vertex_data, index_data }
    }

    pub fn new_plane() -> Self {
        use genmesh::generators::*;

        use palette::*;

        let plane = Plane::new();

        let color_count = plane.shared_vertex_iter().count();

        let colors = (0 .. color_count).map(|i| {
            let hue = i as f32 / color_count as f32;
            let hsv = Hsv::new(hue, 1.0, 1.0);
            let (r, g, b) = Srgb::from(hsv).into_components();
            [r, g, b]
        });

        let vertex_data = plane.shared_vertex_iter()
            .zip(colors)
            .map(|(v, color)| {
                let pos: [f32; 3] = v.pos.into();
                Vertex { pos, color }
            }).collect();

        let index_data = plane.indexed_polygon_iter().flat_map(|t| {
            vec![t.x, t.y, t.z, t.w].into_iter()
        }).map(|u| u as u16).collect();

        Self { vertex_data, index_data }
    }
}

fn main() {
    let mut events = glutin::EventsLoop::new();

    let window_config = glutin::WindowBuilder::new()
        .with_title("Sphere maker".to_owned())
        .with_dimensions((800, 800).into());

    let (api, version, vert_src, frag_src) = (
        glutin::Api::OpenGl, (3, 2),
        include_bytes!("shader/vert_150_core.glsl").to_vec(),
        include_bytes!("shader/frag_150_core.glsl").to_vec(),
    );

    let ctx = glutin::ContextBuilder::new()
        .with_gl(glutin::GlRequest::Specific(api, version))
        .with_vsync(true);

    let (window, mut device, mut factory, main_color, main_depth) = {
        use gfx_window_glutin::init;

        init::<ColorFmt, DepthFmt>(window_config, ctx, &events)
            .expect("Failed to init GL")
    };

    let mut encoder = gfx::Encoder::from(factory.create_command_buffer());

    let pso = factory.create_pipeline_simple(&vert_src, &frag_src, pipe::new()).expect("Failed to init pipeline shader object");

    let mesh = Mesh::new_plane(); //Mesh::new_icosphere();

    let (vertex_buffer, slice) = factory.create_vertex_buffer_with_slice(
        mesh.vertex_data.as_slice(),
        mesh.index_data.as_slice(),
    );

    let locals = factory.create_constant_buffer(1);

    let data = pipe::Data {
        vertex_buffer,
        locals,
        main_color,
        main_depth,
    };

    let start = std::time::Instant::now();

    let default_camera = {
        use cgmath::{Matrix4, Point3, Vector3, Array};

        let source = Point3::new(1.5, -5.0, 3.0);
        let target = Point3::from_value(0.0);
        let up = Vector3::unit_z();

        Matrix4::look_at(source, target, up)
    };

    let projection = {
        use cgmath::*;

        perspective(Deg(45f32), 1.0, 1.0, 10.0)
    };

    let mut running = true;

    let highest_dim = 0.0;
    let lacunarity = 2.5;
    let octaves = 10.0;
    let offset = -0.625;
    let gain = 10.0;

    let transform = [[0.0; 4]; 4];

    let mut locals = Locals {
        transform,
        highest_dim,
        lacunarity,
        octaves,
        offset,
        gain,
    };

    while running {
        events.poll_events(|event| {
            let event = match event {
                Event::WindowEvent { event, .. } => event,
                _ => return,
            };

            match event {
                WindowEvent::CloseRequested => running = false,

                WindowEvent::KeyboardInput { input, .. } => {
                    let KeyboardInput {
                        virtual_keycode,
                        modifiers,
                        state,
                        ..
                    } = input;

                    use glutin::ElementState;

                    match state {
                        ElementState::Pressed => (),
                        _ => return,
                    }

                    if modifiers.shift { return; }
                    if modifiers.ctrl { return; }
                    if modifiers.alt { return; }
                    if modifiers.logo { return; }

                    let key = match virtual_keycode {
                        Some(key) => key,
                        _ => return,
                    };

                    let step = 1.0 / 8.0;

                    use glutin::VirtualKeyCode::*;

                    match key {
                        Q => running = false,
                        A => locals.highest_dim += step,
                        Z => locals.highest_dim -= step,
                        S => locals.lacunarity += step,
                        X => locals.lacunarity -= step,
                        D => locals.octaves += step,
                        C => locals.octaves -= step,
                        F => locals.offset += step,
                        V => locals.offset -= step,
                        G => locals.gain += step,
                        B => locals.gain -= step,
                        Space => println!("{:#?}", locals),

                        _ => (),
                    }
                },

                _ => (),
            }
        });

        let clock = {
            let time = std::time::Instant::now();
            let elapsed = time.duration_since(start);
            let seconds = elapsed.as_secs();
            let millis = elapsed.subsec_millis();
            (seconds as f32 * 1000.0 + millis as f32) / 10.0
        };

        use cgmath::{Deg, Matrix4};

        let rotation = Matrix4::from_angle_z(Deg(clock));

        let matrix = projection * default_camera * rotation;

        locals.transform = matrix.into();

        encoder.update_constant_buffer(&data.locals, &locals);
        encoder.clear(&data.main_color, CLEAR_COLOR);
        encoder.clear_depth(&data.main_depth, 1.0);
        encoder.draw(&slice, &pso, &data);
        encoder.flush(&mut device);
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}

