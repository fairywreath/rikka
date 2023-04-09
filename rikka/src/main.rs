use std::{
    mem::ManuallyDrop,
    time::{Duration, Instant},
};

use winit::{
    dpi,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use rikka_core::nalgebra;
use rikka_gpu::gpu::GpuDesc;

use crate::renderer::camera::*;

mod app;
mod renderer;

fn main() {
    let env = env_logger::Env::default()
        .filter_or("MY_LOG_LEVEL", "trace")
        .write_style_or("MY_LOG_STYLE", "always");
    env_logger::init_from_env(env);

    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        log::error!("Argument to gltf file required!");
        std::process::exit(1);
    }

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Rikka Engine")
        .with_inner_size(dpi::PhysicalSize::new(1920, 1200))
        .with_position(dpi::PhysicalPosition::new(100, 100))
        .build(&event_loop)
        .unwrap();

    let mut rikka_app =
        app::RikkaApp::new(GpuDesc::new(&window, &window), args[1].as_str()).unwrap();
    // let mut rikka_app = ManuallyDrop::new(rikka_app);

    rikka_app.prepare().unwrap();

    let mut camera_view = View::new(nalgebra::Vector3::new(0.0, 2.5, 2.0), 0.0, 0.0);
    let camera_projection = Projection::new(
        window.inner_size().width,
        window.inner_size().height,
        45.0_f32.to_radians(),
        0.1,
        100.0,
    );

    let mut camera_controller = FirstPersonCameraController::new(4.0, 0.4);

    rikka_app.update_view(camera_view.matrix(), camera_view.position());
    rikka_app.update_projection(camera_projection.matrix());

    let mut last_render_time = Instant::now();

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
            } => {
                // unsafe {
                // ManuallyDrop::drop(&mut rikka_app);
                // }

                *control_flow = ControlFlow::Exit;
                // std::thread::sleep(Duration::new(2, 0));
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        virtual_keycode: Some(key),
                        state,
                        ..
                    },
                ..
            } => {
                camera_controller.process_keyboard(*key, *state);
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                camera_controller.set_mouse_pressed(*state == ElementState::Pressed);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                camera_controller.process_scroll(delta);
            }
            _ => {}
        },
        Event::DeviceEvent {
            event: DeviceEvent::MouseMotion { delta },
            ..
        } => {
            camera_controller.process_mouse_motion(delta.0, delta.1);
        }
        Event::MainEventsCleared => {
            let now = Instant::now();
            let dt = now - last_render_time;
            last_render_time = now;

            let fps = Duration::new(1, 0).as_secs_f64() / dt.as_secs_f64();

            // log::info!("FPS: {}, frame time: {}", fps, dt.as_secs_f64());

            camera_controller.update_view(&mut camera_view, dt);
            rikka_app.update_view(camera_view.matrix(), camera_view.position());

            rikka_app.render().unwrap();
        }
        _ => {}
    });
}
