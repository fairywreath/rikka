use std::sync::{Arc, Weak};

use anyhow::{Context, Error, Result};

use rikka_gpu::{self, ash::vk, *};

pub struct RikkaApp {
    storage_buffer: Arc<Buffer>,
    descriptor_set_layout: Arc<DescriptorSetLayout>,
    descriptor_set: DescriptorSet,
    graphics_pipeline: GraphicsPipeline,

    // XXX: This needs to be the last object destructed (and is technically unsafe!). Make this nicer :)
    gpu: Gpu,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Vertex {
    pub positions: [f32; 3],
}

impl RikkaApp {
    pub fn new(gpu_desc: GpuDesc) -> Result<Self> {
        let mut gpu = Gpu::new(gpu_desc).context("Error creating Gpu!")?;

        let vertices = [
            Vertex {
                positions: [1.0, 1.0, 0.0],
            },
            Vertex {
                positions: [-1.0, 1.0, 0.0],
            },
            Vertex {
                positions: [1.0, -1.0, 0.0],
            },
            Vertex {
                positions: [-1.0, -1.0, 0.0],
            },
            Vertex {
                positions: [-1.0, 1.0, 0.0],
            },
            Vertex {
                positions: [1.0, -1.0, 0.0],
            },
        ];

        let buffer_size = std::mem::size_of_val(&vertices);

        let storage_buffer = gpu.create_buffer(
            BufferDesc::new()
                .set_size(buffer_size as u32)
                .set_usage_flags(ash::vk::BufferUsageFlags::STORAGE_BUFFER)
                .set_device_only(false),
        )?;
        storage_buffer.copy_data_to_buffer(&vertices)?;

        let storage_buffer = Arc::new(storage_buffer);

        let descriptor_set_layout = gpu
            .create_descriptor_set_layout(DescriptorSetLayoutDesc::new().add_binding(
                DescriptorBinding::new(
                    vk::DescriptorType::STORAGE_BUFFER,
                    0,
                    1,
                    vk::ShaderStageFlags::VERTEX,
                ),
            ))
            .unwrap();
        let descriptor_set_layout = Arc::new(descriptor_set_layout);

        let binding_resources = vec![DescriptorSetBindingResource::buffer(
            storage_buffer.clone(),
            0,
        )];

        let descriptor_set = gpu.create_descriptor_set(
            DescriptorSetDesc::new(descriptor_set_layout.clone())
                .set_binding_resources(binding_resources),
        )?;

        let graphics_pipeline = {
            let shader_state = gpu.create_shader_state(
                ShaderStateDesc::new()
                    .add_stage(ShaderStageDesc::new_from_source_file(
                        // "shaders/hardcoded_triangle.vert",
                        "shaders/simple.vert",
                        ShaderStageType::Vertex,
                    ))
                    .add_stage(ShaderStageDesc::new_from_source_file(
                        "shaders/simple.frag",
                        ShaderStageType::Fragment,
                    )),
            )?;

            gpu.create_graphics_pipeline(
                GraphicsPipelineDesc::new()
                    .set_shader_stages(shader_state.vulkan_shader_stages())
                    .set_extent(
                        gpu.swapchain().extent().width,
                        gpu.swapchain().extent().height,
                    )
                    .set_rendering_state(RenderingState::new_dimensionless().add_color_attachment(
                        RenderColorAttachment::new().set_format(gpu.swapchain().format()),
                    ))
                    .set_descriptor_set_layouts(vec![descriptor_set_layout.raw()]),
            )?
        };

        Ok(Self {
            gpu,

            storage_buffer,
            descriptor_set_layout,
            descriptor_set,
            graphics_pipeline,
        })
    }

    pub fn render(&mut self) -> Result<()> {
        self.gpu.new_frame().unwrap();

        let acquire_result = self.gpu.swapchain_acquire_next_image();

        match acquire_result {
            Ok(_) => {
                let command_buffer = self
                    .gpu
                    .current_command_buffer(0)
                    .unwrap()
                    .upgrade()
                    .unwrap();
                command_buffer.test_record_commands(
                    self.gpu.swapchain(),
                    &self.graphics_pipeline,
                    &self.descriptor_set,
                )?;

                self.gpu
                    .submit_graphics_command_buffer(Arc::downgrade(&command_buffer))?;
                self.gpu.present()?;
            }
            Err(_) => {
                self.gpu
                    .recreate_swapchain()
                    .expect("Failed to recreate swapchain!");
                self.gpu.advance_frame_counters();
            }
        }
        Ok(())
    }
}

impl Drop for RikkaApp {
    fn drop(&mut self) {
        // XXX: Ideally we should send dropped resources back to the GPU and transfer ownership, where
        //      the GPU will destroy them when deemed safe. This is a hack to make sure all operation are completed
        //      before dropping the app's gpu resources
        self.gpu.wait_idle();
    }
}
