use std::sync::{Arc, Weak};

use winit::{
    dpi,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use rikka_gpu as gpu;
use rikka_gpu::rikka_shader as shader;

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

    let mut gpu = rikka_gpu::Gpu::new(rikka_gpu::GpuDesc::new(&window, &window))
        .expect("Error creating Gpu!");

    // let buffer = gpu
    //     .create_buffer(
    //         rikka_gpu::BufferDesc::new()
    //             .set_size(512)
    //             .set_usage_flags(rikka_gpu::ash::vk::BufferUsageFlags::STORAGE_BUFFER),
    //     )
    //     .unwrap();

    let graphics_pipeline = {
        let shader_state = gpu
            .create_shader_state(
                rikka_gpu::ShaderStateDesc::new()
                    .add_stage(gpu::ShaderStageDesc::new_from_source_file(
                        "shaders/hardcoded_triangle.vert",
                        gpu::ShaderStageType::Vertex,
                    ))
                    .add_stage(gpu::ShaderStageDesc::new_from_source_file(
                        "shaders/simple.frag",
                        gpu::ShaderStageType::Fragment,
                    )),
            )
            .unwrap();

        gpu.create_graphics_pipeline(
            gpu::GraphicsPipelineDesc::new()
                .set_shader_stages(shader_state.vulkan_shader_stages())
                .set_extent(
                    gpu.swapchain().extent().width,
                    gpu.swapchain().extent().height,
                )
                .set_rendering_state(
                    gpu::RenderingState::new_dimensionless().add_color_attachment(
                        gpu::RenderColorAttachment::new().set_format(gpu.swapchain().format()),
                    ),
                ),
        )
        .unwrap()
    };

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
            gpu.new_frame().unwrap();

            let acquire_result = gpu.swapchain_acquire_next_image();

            match acquire_result {
                Ok(_) => {
                    let command_buffer = gpu.current_command_buffer(0).unwrap().upgrade().unwrap();

                    command_buffer
                        .test_record_commands(gpu.swapchain(), &graphics_pipeline)
                        .unwrap();

                    gpu.submit_graphics_command_buffer(Arc::downgrade(&command_buffer))
                        .unwrap();

                    gpu.present().unwrap();
                }
                Err(_) => {
                    gpu.recreate_swapchain()
                        .expect("Failed to recreate swapchain!");
                    gpu.advance_frame_counters();
                }
            }
        }
        _ => {}
    });
}
