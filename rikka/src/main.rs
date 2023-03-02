use std::sync::{Arc, Weak};

use winit::{
    dpi,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use rikka_gpu::GpuDesc;

mod app;

fn main() {
    let env = env_logger::Env::default()
        .filter_or("MY_LOG_LEVEL", "trace")
        .write_style_or("MY_LOG_STYLE", "always");
    env_logger::init_from_env(env);

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Rikka Engine")
        .with_inner_size(dpi::PhysicalSize::new(1920, 1200))
        .with_position(dpi::PhysicalPosition::new(100, 100))
        .build(&event_loop)
        .unwrap();

    // let gltf_file = std::fs::File::open("assets/SunTemple-glTF/suntemple.gltf").unwrap();
    // let reader = std::io::BufReader::new(gltf_file);
    // let gltf = gltf::Gltf::from_reader(reader).unwrap();
    // println!("GTLF INFO:\n{:#?}", gltf);

    let mut rikka_app = app::RikkaApp::new(GpuDesc::new(&window, &window)).unwrap();
    rikka_app.prepare().unwrap();

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        },
        // Render.
        Event::MainEventsCleared => {
            rikka_app.render().unwrap();
        }
        _ => {}
    });
}
