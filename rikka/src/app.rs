use std::sync::{Arc, Weak};

use anyhow::{Context, Error, Result};
use nalgebra::Matrix4;

use rikka_gpu::{
    self as gpu, ash::vk, barriers::*, buffer::*, descriptor_set::*, gpu::*, image::*, pipeline::*,
    sampler::*, shader_state::*, types::*,
};

use crate::renderer::{camera::*, gltf::*, *};

pub struct RikkaApp {
    uniform_buffer: Arc<Buffer>,

    vertex_buffer: Arc<Buffer>,

    descriptor_set_layout: Arc<DescriptorSetLayout>,
    descriptor_set: DescriptorSet,
    graphics_pipeline: GraphicsPipeline,

    uniform_data: UniformData,

    gltf_scene: GltfScene,

    depth_image: Arc<Image>,

    // XXX: This needs to be the last object destructed (and is technically unsafe!). Make this nicer :)
    gpu: Gpu,
}

#[derive(Copy, Clone)]
#[repr(C)]
struct UniformData {
    model: Matrix4<f32>,
    view: Matrix4<f32>,
    projection: Matrix4<f32>,
}

impl RikkaApp {
    pub fn new(gpu_desc: GpuDesc) -> Result<Self> {
        let gpu = Gpu::new(gpu_desc).context("Error creating Gpu!")?;

        let vertices = cube_vertices();
        let buffer_size = std::mem::size_of_val(&vertices);

        let vertex_buffer = gpu.create_buffer(
            BufferDesc::new()
                .set_size(buffer_size as u32)
                .set_usage_flags(vk::BufferUsageFlags::VERTEX_BUFFER)
                .set_device_only(false),
        )?;
        vertex_buffer.copy_data_to_buffer(&vertices)?;
        let vertex_buffer = Arc::new(vertex_buffer);

        let uniform_data = UniformData {
            model: Matrix4::new_scaling(0.02),
            // model: Matrix4::new_scaling(1.0),
            view: Matrix4::identity(),
            projection: Matrix4::identity(),
        };
        let uniform_buffer = gpu.create_buffer(
            BufferDesc::new()
                .set_size(std::mem::size_of::<UniformData>() as _)
                .set_usage_flags(vk::BufferUsageFlags::UNIFORM_BUFFER)
                .set_device_only(false),
        )?;
        let uniform_buffer = Arc::new(uniform_buffer);

        let depth_image_desc = ImageDesc::new(
            gpu.swapchain().extent().width,
            gpu.swapchain().extent().height,
            1,
        )
        .set_format(vk::Format::D32_SFLOAT)
        .set_usage_flags(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT);
        let depth_image = Arc::new(gpu.create_image(depth_image_desc)?);
        gpu.transition_image_layout(
            &depth_image,
            ResourceState::UNDEFINED,
            ResourceState::DEPTH_WRITE,
        )?;

        let descriptor_set_layout = gpu
            .create_descriptor_set_layout(
                DescriptorSetLayoutDesc::new()
                    .add_binding(DescriptorBinding::new(
                        vk::DescriptorType::UNIFORM_BUFFER,
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

        let binding_resources = vec![DescriptorSetBindingResource::buffer(
            uniform_buffer.clone(),
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
                        "shaders/simple_pbr.vert",
                        ShaderStageType::Vertex,
                    ))
                    .add_stage(ShaderStageDesc::new_from_source_file(
                        "shaders/simple_pbr.frag",
                        ShaderStageType::Fragment,
                    )),
            )?;

            let vertex_input_state = VertexInputState::new()
                // Position
                .add_vertex_attribute(0, 0, 0, vk::Format::R32G32B32_SFLOAT)
                .add_vertex_stream(0, 12, vk::VertexInputRate::VERTEX)
                // Tex coords
                .add_vertex_attribute(1, 1, 0, vk::Format::R32G32_SFLOAT)
                .add_vertex_stream(1, 8, vk::VertexInputRate::VERTEX);

            gpu.create_graphics_pipeline(
                GraphicsPipelineDesc::new()
                    .set_shader_stages(shader_state.vulkan_shader_stages())
                    .set_extent(
                        gpu.swapchain().extent().width,
                        gpu.swapchain().extent().height,
                    )
                    .set_rendering_state(
                        RenderingState::new_dimensionless()
                            .add_color_attachment(
                                RenderColorAttachment::new().set_format(gpu.swapchain().format()),
                            )
                            .set_depth_attachment(
                                RenderDepthStencilAttachment::new()
                                    .set_format(vk::Format::D32_SFLOAT),
                            ),
                    )
                    .set_descriptor_set_layouts(vec![descriptor_set_layout.raw()])
                    .set_vertex_input_state(vertex_input_state)
                    .set_rasterization_state(
                        RasterizationState::new().set_polygon_mode(vk::PolygonMode::FILL),
                    ),
            )?
        };

        // let gltf_scene =
        // GltfScene::from_file(&gpu, "assets/Sponza/glTF/Sponza.gltf", &uniform_buffer).unwrap();

        let gltf_scene = GltfScene::from_file(
            &gpu,
            "assets/SunTemple-glTF/suntemple.gltf",
            &uniform_buffer,
        )
        .unwrap();

        Ok(Self {
            gpu,

            descriptor_set_layout,
            descriptor_set,
            graphics_pipeline,

            uniform_buffer,
            uniform_data,

            vertex_buffer,

            gltf_scene,

            depth_image,
        })
    }

    pub fn render(&mut self) -> Result<()> {
        self.gpu.new_frame().unwrap();

        // Update camera uniforms
        self.uniform_buffer
            .copy_data_to_buffer(std::slice::from_ref(&self.uniform_data))?;

        let acquire_result = self.gpu.swapchain_acquire_next_image();

        match acquire_result {
            Ok(_) => {
                let command_buffer = self.gpu.current_command_buffer(0).unwrap();

                let swapchain = self.gpu.swapchain();

                {
                    command_buffer.begin()?;

                    let mut barriers = Barriers::new();
                    barriers.add_image(
                        swapchain.current_image_handle().as_ref(),
                        ResourceState::UNDEFINED,
                        ResourceState::RENDER_TARGET,
                    );
                    command_buffer.pipeline_barrier(barriers);

                    let color_attachment = RenderColorAttachment::new()
                        .set_clear_value(vk::ClearColorValue {
                            float32: [1.0, 1.0, 1.0, 1.0],
                        })
                        .set_operation(RenderPassOperation::Clear)
                        .set_image_view(swapchain.current_image_view())
                        .set_image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

                    let depth_attachment = RenderDepthStencilAttachment::new()
                        .set_clear_value(vk::ClearDepthStencilValue {
                            depth: 1.0,
                            stencil: 0,
                        })
                        .set_depth_operation(RenderPassOperation::Clear)
                        .set_image_view(self.depth_image.raw_view());

                    let rendering_state =
                        RenderingState::new(swapchain.extent().width, swapchain.extent().height)
                            .set_depth_attachment(depth_attachment)
                            .add_color_attachment(color_attachment);
                    command_buffer.begin_rendering(rendering_state);

                    command_buffer.bind_graphics_pipeline(&self.graphics_pipeline);
                    command_buffer.bind_descriptor_set(
                        &self.descriptor_set,
                        self.graphics_pipeline.raw_layout(),
                    );

                    for mesh_draw in &self.gltf_scene.mesh_draws {
                        command_buffer.bind_vertex_buffer(
                            mesh_draw.position_buffer.as_ref().unwrap(),
                            0,
                            mesh_draw.position_offset as _,
                        );
                        command_buffer.bind_vertex_buffer(
                            mesh_draw.tex_coords_buffer.as_ref().unwrap(),
                            1,
                            mesh_draw.tex_coords_offset as _,
                        );

                        command_buffer.bind_index_buffer(
                            mesh_draw.index_buffer.as_ref().unwrap(),
                            mesh_draw.index_offset as _,
                        );

                        command_buffer.bind_descriptor_set(
                            mesh_draw.descriptor_set.as_ref().unwrap(),
                            self.graphics_pipeline.raw_layout(),
                        );

                        command_buffer.draw_indexed(mesh_draw.count, 1, 0, 0, 0);
                    }

                    command_buffer.end_rendering();

                    let mut barriers = Barriers::new();
                    barriers.add_image(
                        swapchain.current_image_handle().as_ref(),
                        ResourceState::RENDER_TARGET,
                        ResourceState::PRESENT,
                    );
                    command_buffer.pipeline_barrier(barriers);

                    command_buffer.end()?;
                }

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
        // let texture_rgba8 = self.texture_data.clone().into_rgba8();
        // let texture_data_bytes = texture_rgba8.as_raw();
        // let texture_data_size = std::mem::size_of_val(texture_data_bytes.as_slice());

        // log::info!(
        //     "Texture data size: {:?}, dimensions: {:?}",
        //     texture_data_size,
        //     texture_rgba8.dimensions(),
        // );

        // let staging_buffer = self.gpu.create_buffer(
        //     BufferDesc::new()
        //         .set_device_only(false)
        //         .set_size(texture_data_size as _)
        //         .set_resource_usage(ResourceUsageType::Staging),
        // )?;

        // self.gpu.copy_data_to_image(
        //     self.texture_image.as_ref(),
        //     &staging_buffer,
        //     texture_data_bytes,
        // )?;

        // let gltf_scene =
        //     GltfScene::from_file(&mut self.gpu, "assets/SunTemple-glTF/suntemple.gltf").unwrap();

        Ok(())
    }

    pub fn update_view(&mut self, view: &Matrix4<f32>) {
        self.uniform_data.view = view.clone();
    }

    pub fn update_projection(&mut self, projection: &Matrix4<f32>) {
        self.uniform_data.projection = projection.clone();
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
