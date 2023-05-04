use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;

use rikka_core::nalgebra::{Matrix4, Vector3, Vector4};

use rikka_core::vk;
use rikka_gpu::{
    barriers::*, buffer::*, escape::*, gpu::*, image::*, pipeline::*, shader_state::*, types::*,
};

use crate::renderer::{gltf::*, loader::*, renderer::*};

pub struct RikkaApp {
    // XXX: This needs to be the last object destructed (and is technically unsafe!). Make this nicer :)
    // gpu: Gpu,
    renderer: Renderer,

    uniform_buffer: Handle<Buffer>,

    zero_buffer: Handle<Buffer>,

    graphics_pipeline: Handle<GraphicsPipeline>,

    uniform_data: UniformData,

    gltf_scene: GltfScene,

    depth_image: Handle<Image>,

    gpu_transfers_thread_run: Arc<AtomicBool>,

    // _thread_pool: rayon::ThreadPool,
    background_thread_pool: threadpool::ThreadPool,
}

#[derive(Copy, Clone)]
#[repr(C)]
struct UniformData {
    model: Matrix4<f32>,
    view: Matrix4<f32>,
    projection: Matrix4<f32>,

    eye: Vector4<f32>,
    light: Vector4<f32>,
}

impl RikkaApp {
    pub fn new(gpu_desc: GpuDesc, gltf_file_name: &str) -> Result<Self> {
        let mut renderer = Renderer::new(Gpu::new(gpu_desc)?);

        // let model = Matrix4::new_scaling(0.004);
        let uniform_data = UniformData {
            model: Matrix4::identity(),
            view: Matrix4::identity(),
            projection: Matrix4::identity(),

            eye: Vector4::new(1.0, 1.0, 1.0, 1.0),
            light: Vector4::new(-1.5, 2.5, -0.5, 1.0),
        };

        let uniform_buffer = renderer.create_buffer(
            BufferDesc::new()
                .set_size(std::mem::size_of::<UniformData>() as _)
                .set_usage_flags(vk::BufferUsageFlags::UNIFORM_BUFFER)
                .set_device_only(false),
        )?;

        let zero_buffer_data = Vector4::<f32>::new(0.0, 0.0, 0.0, 0.0);
        let zero_buffer = renderer.create_buffer(
            BufferDesc::new()
                .set_size(std::mem::size_of_val(zero_buffer_data.as_slice()) as _)
                .set_usage_flags(vk::BufferUsageFlags::VERTEX_BUFFER)
                .set_device_only(false),
        )?;
        zero_buffer.copy_data_to_buffer(zero_buffer_data.as_slice())?;

        let depth_image_desc = ImageDesc::new(renderer.extent().width, renderer.extent().height, 1)
            .set_format(vk::Format::D32_SFLOAT)
            .set_usage_flags(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT);
        let depth_image = renderer.create_image(depth_image_desc)?;

        renderer.gpu().transition_image_layout(
            &depth_image,
            ResourceState::UNDEFINED,
            ResourceState::DEPTH_WRITE,
        )?;

        let graphics_pipeline: Handle<GraphicsPipeline> = {
            let shader_state_desc = ShaderStateDesc::new()
                .add_stage(ShaderStageDesc::new_from_source_file(
                    "shaders/simple_pbr.vert",
                    ShaderStageType::Vertex,
                ))
                .add_stage(ShaderStageDesc::new_from_source_file(
                    "shaders/simple_pbr.frag",
                    ShaderStageType::Fragment,
                ));

            let vertex_input_state = VertexInputState::new()
                // Position
                .add_vertex_attribute(0, 0, 0, vk::Format::R32G32B32_SFLOAT)
                .add_vertex_stream(0, 12, vk::VertexInputRate::VERTEX)
                // Tex coords
                .add_vertex_attribute(1, 1, 0, vk::Format::R32G32_SFLOAT)
                .add_vertex_stream(1, 8, vk::VertexInputRate::VERTEX)
                // Normals
                .add_vertex_attribute(2, 2, 0, vk::Format::R32G32B32_SFLOAT)
                .add_vertex_stream(2, 12, vk::VertexInputRate::VERTEX)
                // Tangents
                .add_vertex_attribute(3, 3, 0, vk::Format::R32G32B32A32_SFLOAT)
                .add_vertex_stream(3, 16, vk::VertexInputRate::VERTEX);

            renderer
                .gpu()
                .create_graphics_pipeline(
                    GraphicsPipelineDesc::new()
                        // .set_shader_stages(shader_state.vulkan_shader_stages())
                        .set_shader_state(shader_state_desc)
                        .set_extent(renderer.extent().width, renderer.extent().height)
                        .set_rendering_state(
                            RenderingState::new_dimensionless()
                                .add_color_attachment(
                                    RenderColorAttachment::new()
                                        .set_format(renderer.gpu().swapchain().format()),
                                )
                                .set_depth_attachment(
                                    RenderDepthStencilAttachment::new()
                                        .set_format(vk::Format::D32_SFLOAT),
                                ),
                        )
                        // .add_descriptor_set_layout(descriptor_set_layout.raw())
                        // .add_descriptor_set_layout(gpu.bindless_descriptor_set_layout().raw())
                        .set_vertex_input_state(vertex_input_state)
                        .set_rasterization_state(
                            RasterizationState::new()
                                .set_polygon_mode(vk::PolygonMode::FILL)
                                .set_cull_mode(vk::CullModeFlags::NONE),
                        ),
                )?
                .into()
        };

        let mut transfer_manager = renderer.gpu().new_transfer_manager()?;
        let mut async_loader =
            AsynchronousLoader::new(transfer_manager.new_image_upload_request_sender());

        let gltf_scene = GltfScene::from_file(
            &mut renderer.gpu_mut(),
            gltf_file_name,
            &uniform_buffer,
            &graphics_pipeline.descriptor_set_layouts()[0],
            &mut async_loader,
        )?;

        let background_thread_pool = threadpool::ThreadPool::new(3);
        let gpu_transfers_thread_run = Arc::new(AtomicBool::new(true));

        let load_resources = gpu_transfers_thread_run.clone();
        background_thread_pool.execute(move || {
            while load_resources.load(Ordering::Relaxed) {
                async_loader
                    .update()
                    .expect("Async loader failed to update!");
            }
        });

        let run_transfers = gpu_transfers_thread_run.clone();
        background_thread_pool.execute(move || {
            while run_transfers.load(Ordering::Relaxed) {
                transfer_manager
                    .perform_transfers()
                    .expect("GPU transfer manager failed to update!");
            }

            log::info!("Transfer manager exeuction stopped");
            transfer_manager.destroy();
        });

        // Test render graph compilation
        let mut deferred_graph = rikka_graph::parser::parse_from_file("data/deferred_graph.json")?;
        deferred_graph.compile(renderer.gpu_mut())?;

        Ok(Self {
            renderer,

            graphics_pipeline,

            uniform_buffer,
            uniform_data,

            gltf_scene,

            depth_image,
            zero_buffer,

            gpu_transfers_thread_run,
            background_thread_pool,
        })
    }

    pub fn render(&mut self) -> Result<()> {
        self.renderer.begin_frame()?;

        self.gltf_scene.scene_graph.calculate_transforms()?;

        let command_buffer = self.renderer.command_buffer(0)?;

        let swapchain = self.renderer.gpu().swapchain();

        {
            command_buffer.begin()?;

            let barriers = Barriers::new().add_image(
                swapchain.current_image(),
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
            //    XXX: Bind this automatically in the GPU layer
            command_buffer.bind_descriptor_set(
                self.renderer.gpu().bindless_descriptor_set().as_ref(),
                self.graphics_pipeline.raw_layout(),
                1,
            );

            for mesh_draw in &self.gltf_scene.mesh_draws {
                // XXX FIXME: This does not work, we cannot copy to uniform buffers in-between draw calls
                self.uniform_data.model =
                    self.gltf_scene.scene_graph.global_matrices[mesh_draw.scene_graph_node_index];
                self.uniform_buffer
                    .copy_data_to_buffer(std::slice::from_ref(&self.uniform_data))?;

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
                command_buffer.bind_vertex_buffer(
                    mesh_draw.normal_buffer.as_ref().unwrap(),
                    2,
                    mesh_draw.normal_offset as _,
                );

                if let Some(tangent_buffer) = &mesh_draw.tangent_buffer {
                    command_buffer.bind_vertex_buffer(
                        mesh_draw.tangent_buffer.as_ref().unwrap(),
                        3,
                        mesh_draw.tangent_offset as _,
                    );
                } else {
                    command_buffer.bind_vertex_buffer(&self.zero_buffer, 3, 0);
                }

                command_buffer.bind_index_buffer(
                    mesh_draw.index_buffer.as_ref().unwrap(),
                    mesh_draw.index_offset as _,
                );

                command_buffer.bind_descriptor_set(
                    mesh_draw.descriptor_set.as_ref().unwrap(),
                    self.graphics_pipeline.raw_layout(),
                    0,
                );

                command_buffer.draw_indexed(mesh_draw.count, 1, 0, 0, 0);
            }

            command_buffer.end_rendering();

            let barriers = Barriers::new().add_image(
                swapchain.current_image(),
                ResourceState::RENDER_TARGET,
                ResourceState::PRESENT,
            );
            command_buffer.pipeline_barrier(barriers);

            command_buffer.end()?;
        }

        self.renderer.queue_command_buffer(command_buffer);

        self.renderer
            .gpu_mut()
            .update_image_transitions(0)
            .expect("Failed to update GPU image transitions");

        self.renderer.end_frame()?;

        Ok(())
    }

    pub fn prepare(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn update_view(&mut self, view: &Matrix4<f32>, eye_position: &Vector3<f32>) {
        self.uniform_data.view = view.clone();
        self.uniform_data.eye = Vector4::new(eye_position.x, eye_position.y, eye_position.z, 1.0);
    }

    pub fn update_projection(&mut self, projection: &Matrix4<f32>) {
        self.uniform_data.projection = projection.clone();
    }
}

impl Drop for RikkaApp {
    fn drop(&mut self) {
        self.renderer.wait_idle();
        // self.gltf_scene.mesh_draws.clear();
        // self.gltf_scene._gpu_images.clear();

        self.gpu_transfers_thread_run
            .fetch_and(false, Ordering::Relaxed);

        self.background_thread_pool.join();

        log::info!("App dropped");
    }
}
