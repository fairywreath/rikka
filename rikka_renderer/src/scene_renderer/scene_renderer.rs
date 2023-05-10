use std::{collections::HashMap, mem::size_of, sync::Arc};

use anyhow::{Context, Result};
use serde_derive::{Deserialize, Serialize};

use rikka_core::vk;
use rikka_gpu::{barriers::*, buffer::*, descriptor_set::*, gpu::Gpu, image::Image, types::*};
use rikka_graph::{graph::Graph, types::Resource};

use crate::{
    loader::asynchronous::AsynchronousLoader,
    pass::{depth_of_field::*, depth_pre::*, gbuffer::*, pbr_lighting::*, simple_pbr::*},
    renderer::*,
    scene,
    scene_renderer::{gltf::*, types::*},
};

#[derive(Serialize, Deserialize)]
pub struct FilePathsConfig {
    pub render_graph_file_path: String,
    pub render_techniques_file_paths: Vec<String>,
    pub gtlf_model_file_path: String,
}

pub struct Config<'a> {
    pub file_paths_config: FilePathsConfig,
    pub gpu: Gpu,
    pub async_loader: &'a mut AsynchronousLoader,
}

struct RenderTechniqeFilePaths(&'static str);

impl RenderTechniqeFilePaths {
    const FULLSCREEN: &str = "data/fullscreen.json";
    const SIMPLE_PBR: &str = "data/simple_pbr.json";
}

// const FINAL_IMAGE_NODE_NAME: &str = "final";

pub struct SceneRenderer {
    renderer: Renderer,
    render_graph: Graph,

    meshes: Vec<Arc<Mesh>>,
    scene_graph: scene::Graph,

    final_image: Handle<Image>,

    scene_uniform_buffer: Handle<Buffer>,

    // XXX: Remove this `pub` and add accessors
    pub scene_uniform_data: GPUSceneUniformData,

    fullscreen_technique: Arc<RenderTechnique>,

    // Render passes
    // pbr_lighting_pass: PBRLightingPass,
    // gbuffer_pass: GBufferPass,
    // depth_pre_pass: DepthPrePass,

    // One-pass PBR
    simple_pbr_render_technique: Arc<RenderTechnique>,
    simple_pbr_pass: SimplePbrPass,
}

impl SceneRenderer {
    pub fn new(
        mut renderer: Renderer,
        mut render_graph: Graph,
        async_loader: &mut AsynchronousLoader,
        gltf_file_name: &str,
    ) -> Result<Self> {
        // Get final image to be copied to the swapchain from the render graph
        let final_image_graph_resource = render_graph
            // .access_node_by_name(FINAL_IMAGE_NODE_NAME)
            .access_node_by_name("simple_pbr_pass")
            .context("Failed to retrieve render graph final node")?
            .outputs[1];
        let final_image = render_graph
            .access_resource_by_handle(final_image_graph_resource)?
            .gpu_image()?;

        // Create final fullscreen technique
        let fullscreen_technique = renderer
            .create_technique_from_file(RenderTechniqeFilePaths::FULLSCREEN, &render_graph)?;

        // Setup per-frame uniform buffer
        let scene_uniform_buffer_desc = BufferDesc::new()
            .set_size(size_of::<GPUSceneUniformData>() as _)
            .set_device_only(false)
            .set_usage_flags(vk::BufferUsageFlags::UNIFORM_BUFFER);
        let scene_uniform_buffer = renderer.create_buffer(scene_uniform_buffer_desc)?;

        let scene_uniform_data = GPUSceneUniformData::new();
        scene_uniform_buffer.copy_data_to_buffer(&[scene_uniform_data])?;

        // Main render technique
        let simple_pbr_render_technique = renderer
            .create_technique_from_file(RenderTechniqeFilePaths::SIMPLE_PBR, &render_graph)?;

        // Load glTF scene
        log::trace!("Loading gltf file {}...", gltf_file_name);
        let gltf_scene = GltfScene::new_from_file(
            &mut renderer,
            gltf_file_name,
            &scene_uniform_buffer,
            &simple_pbr_render_technique,
            async_loader,
        )?;
        log::trace!("Successfully loaded gltf file {}", gltf_file_name);

        let meshes = gltf_scene
            .meshes
            .into_iter()
            .map(Arc::new)
            .collect::<Vec<_>>();
        let scene_graph = gltf_scene.scene_graph;

        // Create render passes
        let simple_pbr_pass = SimplePbrPass::new(
            &renderer,
            &render_graph,
            &meshes,
            renderer.gpu().bindless_descriptor_set().clone(),
        )?;

        // Register render passes
        render_graph
            .register_render_pass("simple_pbr_pass", simple_pbr_pass.create_render_pass())?;

        // Setup final image as input for fullscreen pass
        renderer
            .gpu_mut()
            .add_bindless_image_update(ImageResourceUpdate {
                frame: 0,
                image: Some(final_image.clone()),
                sampler: None,
            });
        renderer.gpu_mut().update_bindless_images();

        // Final image is transitioned from shader read to render target at the start of every frame,
        // transition it to shader resource here to cleanly setup the barriers
        renderer.gpu().transition_image_layout(
            &final_image,
            ResourceState::UNDEFINED,
            ResourceState::SHADER_RESOURCE,
        )?;

        Ok(Self {
            renderer,
            render_graph,
            meshes,
            scene_graph,
            final_image,
            scene_uniform_buffer,
            scene_uniform_data,
            fullscreen_technique,
            simple_pbr_render_technique,
            simple_pbr_pass,
        })
    }

    pub fn new_from_config(config: Config) -> Result<Self> {
        let mut renderer = Renderer::new(config.gpu);

        let mut render_graph = rikka_graph::parser::parse_from_file(
            config.file_paths_config.render_graph_file_path.as_str(),
        )?;
        render_graph.compile(renderer.gpu_mut())?;

        Self::new(
            renderer,
            render_graph,
            config.async_loader,
            config.file_paths_config.gtlf_model_file_path.as_str(),
        )
    }

    pub fn upload_data_to_gpu(&mut self) -> Result<()> {
        self.scene_graph.calculate_transforms()?;
        for mesh in &self.meshes {
            let mut mesh_data = mesh.create_gpu_data();
            mesh_data.set_matrices_from_scene_graph(mesh, &self.scene_graph);
            mesh.pbr_material
                .material_buffer
                .copy_data_to_buffer(&[mesh_data])?;
        }

        Ok(())
    }

    pub fn render(&mut self) -> Result<()> {
        // XXX: This call is useless because the uniform buffers that contain the model matrix will not be updated. Handle this nicer?
        // self.scene_graph.calculate_transforms()?;

        self.scene_uniform_buffer
            .copy_data_to_buffer(&[self.scene_uniform_data])?;

        self.renderer.begin_frame()?;

        let command_buffer = self.renderer.command_buffer(0)?;
        command_buffer.begin()?;
        let swapchain = self.renderer.gpu().swapchain();

        let barriers = Barriers::new().add_image(
            &self.final_image,
            ResourceState::SHADER_RESOURCE,
            ResourceState::RENDER_TARGET,
        );
        command_buffer.pipeline_barrier(barriers);

        self.render_graph.render(&command_buffer)?;

        let barriers = Barriers::new()
            .add_image(
                &self.final_image,
                ResourceState::RENDER_TARGET,
                ResourceState::SHADER_RESOURCE,
            )
            .add_image(
                swapchain.current_image(),
                ResourceState::UNDEFINED,
                ResourceState::RENDER_TARGET,
            );
        command_buffer.pipeline_barrier(barriers);

        {
            let color_attachment = RenderColorAttachment::new()
                .set_clear_value(vk::ClearColorValue {
                    float32: [1.0, 1.0, 1.0, 1.0],
                })
                .set_operation(RenderPassOperation::Clear)
                .set_image_view(swapchain.current_image_view())
                .set_image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

            let rendering_state =
                RenderingState::new(swapchain.extent().width, swapchain.extent().height)
                    .add_color_attachment(color_attachment);
            command_buffer.begin_rendering(rendering_state);

            let fullscreen_graphics_pipeline =
                &self.fullscreen_technique.passes[0].graphics_pipeline;
            command_buffer.bind_graphics_pipeline(fullscreen_graphics_pipeline);
            command_buffer.bind_descriptor_set(
                self.renderer.gpu().bindless_descriptor_set().as_ref(),
                fullscreen_graphics_pipeline.raw_layout(),
                0,
            );

            // XXX: Set scissor, viewport?

            // Set final image bindless index as the instance count parameter
            command_buffer.draw(3, 1, 0, self.final_image.bindless_index());

            command_buffer.end_rendering();
        }

        let barriers = Barriers::new().add_image(
            swapchain.current_image(),
            ResourceState::RENDER_TARGET,
            ResourceState::PRESENT,
        );
        command_buffer.pipeline_barrier(barriers);

        command_buffer.end()?;

        self.renderer.queue_command_buffer(command_buffer);

        self.renderer
            .gpu_mut()
            .update_image_transitions(0)
            .expect("Failed to update GPU image transitions");

        self.renderer.end_frame()?;

        Ok(())
    }

    pub fn wait_idle(&self) {
        self.renderer.gpu().wait_idle();
    }
}
