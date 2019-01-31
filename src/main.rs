#[macro_use] // 2018-style macro imports don't work so well
extern crate gfx;

use gfx::{
    Device,
    Factory,
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
        tex_pos: [f32; 2] = "a_TexPos",
        tex_layer: u16 = "a_TexLayer",
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
        texture: gfx::TextureSampler<f32> = "t_Value",
        locals: gfx::ConstantBuffer<Locals> = "Locals",
        main_color: gfx::RenderTarget<ColorFmt> = "Target0",
        main_depth: gfx::DepthTarget<DepthFmt> =
            gfx::preset::depth::LESS_EQUAL_WRITE,
    }
}

impl Vertex {
    pub fn lerp(a: Self, b: Self, amount: f32) -> Self {
        assert_eq!(a.tex_layer, b.tex_layer);

        let blend = |u: f32, v: f32| -> f32 {
            amount * u + (1.0 - amount) * v
        };

        let pos = {
            let [ax, ay, az] = a.pos;
            let [bx, by, bz] = b.pos;
            [blend(ax, bx), blend(ay, by), blend(az, bz)]
        };

        let tex_pos = {
            let [au, av] = a.tex_pos;
            let [bu, bv] = b.tex_pos;
            [blend(au, bu), blend(av, bv)]
        };

        let tex_layer = a.tex_layer;

        let color = {
            let [ar, ag, ab] = a.color;
            let [br, bg, bb] = b.color;
            [blend(ar, br), blend(ag, bg), blend(ab, bb)]
        };

        Vertex { pos, tex_pos, tex_layer, color }
    }
}

pub const CLEAR_COLOR: [f32; 4] = [0.1, 0.2, 0.3, 1.0];

#[derive(Clone)]
pub struct QuadMesh {
    pub vertex_data: Vec<Vertex>,
    pub index_data: Vec<[usize; 4]>,
}

impl QuadMesh {
    pub fn new_cube() -> Self {
        use cgmath::prelude::*;

        type Vec3f = cgmath::Vector3<f32>;

        let proto: [_; 4] = {
            let z = 1.0;

            let corner = |x, y| Vec3f::new(x, y, z);

            [
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
            ]
        };

        let rotations: [_; 6] = {
            type M = cgmath::Matrix4<f32>;

            use cgmath::Deg;

            let mx = |n| M::from_angle_x(Deg(n));
            let my = |n| M::from_angle_y(Deg(n));

            [my(0.0), my(90.0), my(180.0), my(270.0), mx(90.0), mx(270.0)]
        };

        let colors = [
            [1.0, 0.3, 0.3],
            [0.3, 1.0, 0.3],
            [0.3, 0.3, 1.0],
            [1.0, 1.0, 0.3],
            [1.0, 0.3, 1.0],
            [0.3, 1.0, 1.0],
        ];

        let quads = rotations.iter().enumerate().map(|(layer, &matrix)| {
            let tex_layer = layer as u16;

            let color = colors[layer];

            let vertex = |i: usize| {
                let pos = matrix.transform_vector(proto[i]).into();
                let tex_pos = proto[i].truncate().into();
                Vertex { color, tex_layer, tex_pos, pos }
            };

            [vertex(0), vertex(1), vertex(2), vertex(3)]
        }).collect::<Vec<_>>();

        let mut vertex_data = vec![];

        let mut put = |vertex| {
            let index = vertex_data.len();
            vertex_data.push(vertex);
            index
        };

        let index_data = quads.iter().map(|&[a, b, c, d]| {
            [put(a), put(b), put(c), put(d)]
        }).collect();

        Self { vertex_data, index_data }
    }

    pub fn subdivide(&self) -> Self {
        let v = &self.vertex_data;

        let mut vertex_data = v.clone();

        let mut index_data = vec![];

        for &[a, b, c, d] in self.index_data.iter() {
            let ab = Vertex::lerp(v[a], v[b], 0.5);
            let bc = Vertex::lerp(v[b], v[c], 0.5);
            let cd = Vertex::lerp(v[c], v[d], 0.5);
            let da = Vertex::lerp(v[d], v[a], 0.5);
            let mid = Vertex::lerp(ab, cd, 0.5);

            let mut put = |vertex| {
                let index = vertex_data.len();
                vertex_data.push(vertex);
                index
            };

            let ab = put(ab);
            let bc = put(bc);
            let cd = put(cd);
            let da = put(da);
            let mid = put(mid);

            index_data.push([a, ab, mid, da]);
            index_data.push([ab, b, bc, mid]);
            index_data.push([mid, bc, c, cd]);
            index_data.push([da, mid, cd, d]);
        }

        Self { vertex_data, index_data }
    }

    pub fn to_sphere(&self) -> Self {
        let mut mesh = self.clone();

        for vertex in mesh.vertex_data.iter_mut() {
            use cgmath::prelude::*;

            type Vec3f = cgmath::Vector3<f32>;

            vertex.pos = Vec3f::from(vertex.pos).normalize().into();
        }

        mesh
    }

    pub fn triangulate(&self) -> Mesh {
        let vertex_data = self.vertex_data.clone();

        let mut index_data = {
            let triangle_count = self.index_data.len() * 2;
            Vec::with_capacity(triangle_count * 3)
        };

        for &[a, b, c, d] in self.index_data.iter() {
            use cgmath::prelude::*;

            type Pos = cgmath::Vector3<f32>;

            let a_c_distance = {
                let a_pos: Pos = vertex_data[a].pos.into();
                let c_pos: Pos = vertex_data[c].pos.into();
                (a_pos - c_pos).magnitude()
            };

            let b_d_distance = {
                let b_pos: Pos = vertex_data[b].pos.into();
                let d_pos: Pos = vertex_data[d].pos.into();
                (b_pos - d_pos).magnitude()
            };

            let [a, b, c, d] = [a as u16, b as u16, c as u16, d as u16];

            // Cut the longer edge
            if a_c_distance > b_d_distance {
                index_data.extend_from_slice(&[a, b, d, d, b, c]);
            } else {
                index_data.extend_from_slice(&[a, b, c, c, d, a]);
            }
        }

        Mesh { vertex_data, index_data }
    }
}

pub struct Mesh {
    pub vertex_data: Vec<Vertex>,
    pub index_data: Vec<u16>,
}

impl Mesh {
    pub fn new_cubesphere() -> Self {
        let mut mesh = QuadMesh::new_cube();

        for _ in 0 .. 4 {
            mesh = mesh.subdivide();
        }

        mesh.to_sphere().triangulate()
    }

    pub fn new_icosphere() -> Self {
        use genmesh::generators::*;

        let sphere = IcoSphere::subdivide(4);

        let color = [1.0, 1.0, 1.0];

        let vertex_data = sphere.shared_vertex_iter().map(|v| {
            let pos: [f32; 3] = v.pos.into();
            //pos.iter_mut().for_each(|n| *n /= 2.0);

            let tex_pos = [0.5, 0.5];
            let tex_layer = 0;
            Vertex { pos, color, tex_pos, tex_layer }
        }).collect();

        let index_data = sphere.indexed_polygon_iter().flat_map(|t| {
            [t.x, t.y, t.z].to_vec().into_iter()
        }).map(|u| u as u16).collect();

        Self { vertex_data, index_data }
    }

    pub fn new_plane() -> Self {
        use palette::*;

        let color_count = 4;

        let colors = (0 .. color_count).map(|i| {
            let hue = i as f32 / color_count as f32;
            let hsv = Hsv::new(hue, 1.0, 1.0);
            let (r, g, b) = Srgb::from(hsv).into_components();
            [r, g, b]
        });

        let vertex_data = [
            (1, -1),
            (1, 1),
            (-1, 1),
            (-1, -1),
        ].iter().zip(colors).map(|(&(x, y), color)| {
            let x = x as f32;
            let y = y as f32;
            let pos = [x, y, 0.0];
            let tex_pos = [x*0.5 + 0.5, y*0.5 + 0.5];
            let tex_layer = 0;
            Vertex { pos, color, tex_pos, tex_layer }
        }).collect();

        let index_data = vec![
            0, 1, 3,
            1, 3, 2,
        ];

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

    //let mesh = Mesh::new_icosphere();
    //let mesh = Mesh::new_plane();
    let mesh = Mesh::new_cubesphere();

    let (vertex_buffer, slice) = factory.create_vertex_buffer_with_slice(
        mesh.vertex_data.as_slice(),
        mesh.index_data.as_slice(),
    );

    type TexFmt = gfx::format::Depth32F;

    let texel_data = &[
        0.0,
    ];

    let (_, texture_view) = factory.create_texture_immutable::<TexFmt>(
        gfx::texture::Kind::D2(1, 1, gfx::texture::AaMode::Single),
        gfx::texture::Mipmap::Provided,
        &[texel_data],
    ).expect("Failed to create texture");

    let sampler = factory.create_sampler({
        use gfx::texture::*;

        SamplerInfo::new(FilterMethod::Bilinear, WrapMode::Clamp)
    });

    let texture = (texture_view, sampler);

    let locals = factory.create_constant_buffer(1);

    let data = pipe::Data {
        vertex_buffer,
        locals,
        texture,
        main_color,
        main_depth,
    };

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
    let mut spinning = true;

    let mut angle = 0.0;
    let mut previous_time = std::time::Instant::now();

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
                        Period => spinning = false,

                        _ => (),
                    }
                },

                _ => (),
            }
        });

        let clock = {
            let time = std::time::Instant::now();
            let elapsed = time.duration_since(previous_time);
            previous_time = time;

            let seconds = elapsed.as_secs();
            let millis = elapsed.subsec_millis();
            (seconds as f32 * 1000.0 + millis as f32) / 10.0
        };

        if spinning {
            angle += clock;
            angle %= 360.0;
        }

        use cgmath::{Deg, Matrix4};

        let rotation = Matrix4::from_angle_z(Deg(angle));

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

