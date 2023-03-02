use std::sync::{Arc, Weak};

use anyhow::{Context, Error, Result};

use rikka_gpu::{self, ash::vk, *};

pub struct RikkaApp {
    storage_buffer: Arc<Buffer>,
    descriptor_set_layout: Arc<DescriptorSetLayout>,
    descriptor_set: DescriptorSet,
    graphics_pipeline: GraphicsPipeline,

    texture_image: Arc<Image>,
    texture_data: image::DynamicImage,
    texture_sampler: Arc<Sampler>,

    // XXX: This needs to be the last object destructed (and is technically unsafe!). Make this nicer :)
    gpu: Gpu,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Vertex {
    pub positions: [f32; 3],
    pub tex_coords: [f32; 2],
}

impl RikkaApp {
    pub fn new(gpu_desc: GpuDesc) -> Result<Self> {
        let gpu = Gpu::new(gpu_desc).context("Error creating Gpu!")?;

        let vertices = [
            Vertex {
                positions: [1.0, 1.0, 0.0],
                tex_coords: [1.0, 1.0],
            },
            Vertex {
                positions: [-1.0, 1.0, 0.0],
                tex_coords: [0.0, 1.0],
            },
            Vertex {
                positions: [1.0, -1.0, 0.0],
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                positions: [-1.0, -1.0, 0.0],
                tex_coords: [0.0, 0.0],
            },
            Vertex {
                positions: [-1.0, 1.0, 0.0],
                tex_coords: [0.0, 1.0],
            },
            Vertex {
                positions: [1.0, -1.0, 0.0],
                tex_coords: [1.0, 0.0],
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

        let texture_data =
            image::open("assets/ononoki.jpg").context("Failed to open image file")?;
        log::info!(
            "Loaded image info -  color: {:?}, dimensions: {} x {}",
            texture_data.color(),
            texture_data.width(),
            texture_data.height()
        );

        let texture_sampler = Arc::new(gpu.create_sampler(SamplerDesc::new())?);

        let image_desc = ImageDesc::new(texture_data.width(), texture_data.height(), 1)
            .set_format(vk::Format::R8G8B8A8_UNORM)
            .set_usage_flags(vk::ImageUsageFlags::SAMPLED);
        let mut texture_image = gpu.create_image(image_desc)?;
        texture_image.set_linked_sampler(texture_sampler.clone());
        let texture_image = Arc::new(texture_image);

        let descriptor_set_layout = gpu
            .create_descriptor_set_layout(
                DescriptorSetLayoutDesc::new()
                    .add_binding(DescriptorBinding::new(
                        vk::DescriptorType::STORAGE_BUFFER,
                        0,
                        1,
                        vk::ShaderStageFlags::VERTEX,
                    ))
                    .add_binding(DescriptorBinding::new(
                        vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                        1,
                        1,
                        vk::ShaderStageFlags::FRAGMENT,
                    )),
            )
            .unwrap();
        let descriptor_set_layout = Arc::new(descriptor_set_layout);

        let binding_resources = vec![
            DescriptorSetBindingResource::buffer(storage_buffer.clone(), 0),
            DescriptorSetBindingResource::image(texture_image.clone(), 1),
        ];

        let descriptor_set = gpu.create_descriptor_set(
            DescriptorSetDesc::new(descriptor_set_layout.clone())
                .set_binding_resources(binding_resources),
        )?;

        let graphics_pipeline = {
            let shader_state = gpu.create_shader_state(
                ShaderStateDesc::new()
                    .add_stage(ShaderStageDesc::new_from_source_file(
                        "shaders/simple_texture.vert",
                        ShaderStageType::Vertex,
                    ))
                    .add_stage(ShaderStageDesc::new_from_source_file(
                        "shaders/simple_texture.frag",
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

            texture_data,
            texture_image,
            texture_sampler,

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
                let command_buffer = self.gpu.current_command_buffer(0).unwrap();

                command_buffer.test_record_commands(
                    self.gpu.swapchain(),
                    &self.graphics_pipeline,
                    &self.descriptor_set,
                )?;

                self.gpu
                    .submit_graphics_command_buffer(command_buffer.as_ref())?;

                // XXX: So we don't panic when we need to recreate swapchain...
                //      Need to wait for all command pools to complete before reset if need to recreate swapchain
                let present_result = self.gpu.present();

                match present_result {
                    // XXX: gpu.new_frame will reset command pools, hence on a failed presentation
                    //      we need to wait on the submitted command buffers before presenting.
                    //      There has to be a better way of handling this? Right now we do not
                    //      advance the frame counters on failed presentation(since we would need to wait on the
                    //      next set of semaphores if we did, and those cannot be signaled). Maybe we manually
                    //      signal these when present failes and wait on the current submitted command buffers inside
                    //      present() as well
                    Err(_) => {
                        log::debug!("Swapchain presentation failed, waiting for command buffer(s) submitted in the current frame to finish");
                        self.gpu.wait_idle();
                    }
                    _ => {}
                }
            }
            Err(_) => {
                log::trace!("Recreating swapchain...");
                self.gpu
                    .recreate_swapchain()
                    .expect("Failed to recreate swapchain!");
                self.gpu.advance_frame_counters();
            }
        }

        Ok(())
    }

    pub fn prepare(&mut self) -> Result<()> {
        let texture_rgba8 = self.texture_data.clone().into_rgba8();
        let texture_data_bytes = texture_rgba8.as_raw();
        let texture_data_size = std::mem::size_of_val(texture_data_bytes.as_slice());

        log::info!(
            "Texture data size: {:?}, dimensions: {:?}",
            texture_data_size,
            texture_rgba8.dimensions(),
        );

        let staging_buffer = self.gpu.create_buffer(
            BufferDesc::new()
                .set_device_only(false)
                .set_size(texture_data_size as _)
                .set_resource_usage(ResourceUsageType::Staging),
        )?;

        self.gpu.copy_data_to_image(
            self.texture_image.as_ref(),
            &staging_buffer,
            texture_data_bytes,
        )?;

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
