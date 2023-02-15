use std::sync::{Arc, Weak};

use winit::{
    dpi,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

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

    let mut rhi = rikka_rhi::RHIContext::new(rikka_rhi::RHICreationDesc::new(&window, &window))
        .expect("Error creating RHIContext!");

    {
        let buffer = rhi
            .create_buffer(
                rikka_rhi::BufferDesc::new()
                    .set_size(512)
                    .set_usage_flags(rikka_rhi::ash::vk::BufferUsageFlags::STORAGE_BUFFER),
            )
            .unwrap();
    }

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
            rhi.new_frame().unwrap();

            let acquire_result = rhi.swapchain_acquire_next_image();

            match acquire_result {
                Ok(_) => {
                    let command_buffer = rhi.current_command_buffer(0).unwrap().upgrade().unwrap();

                    command_buffer.test_record_commands(rhi.swapchain());

                    rhi.submit_graphics_command_buffer(Arc::downgrade(&command_buffer));

                    rhi.present();
                }
                Err(_) => {
                    rhi.recreate_swapchain()
                        .expect("Failed to recreate swapchain!");
                    rhi.advance_frame_counters();
                }
            }
        }
        _ => {}
    });
}
