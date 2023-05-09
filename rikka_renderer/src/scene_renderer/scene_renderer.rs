use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use serde_derive::{Deserialize, Serialize};

use rikka_core::vk;
use rikka_gpu::{descriptor_set::*, types::*};
use rikka_graph::graph::Graph;

use crate::{gltf::GltfScene, renderer::*, scene, scene_renderer::types::*};

#[derive(Serialize, Deserialize)]
pub struct config {
    pub render_graph: String,
    pub render_techniques: Vec<String>,
    pub gtlf_model: String,
}

pub struct SceneRenderer {
    renderer: Renderer,
    render_graph: Graph,

    scene_graph: scene::Graph,

    render_techniques: HashMap<String, Arc<RenderTechnique>>,

    meshes: Vec<Mesh>,

    fullscreen_technique: Arc<RenderTechnique>,
    fullscreen_descriptor_set: Handle<DescriptorSet>,

    gltf_scene: GltfScene,
}

impl SceneRenderer {
    pub fn new() -> Result<Self> {
        todo!()
    }

    pub fn prepare(&mut self) -> Result<()> {
        Ok(())
    }

    /// Assigns render passes to the render graph
    fn register_render_passes(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn upload_data_to_gpu(&mut self) -> Result<()> {
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
        let command_buffer = self.renderer.command_buffer(0)?;
        command_buffer.begin()?;

        self.render_graph.render(&command_buffer)?;

        let swapchain = self.renderer.gpu().swapchain();

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

        let fullscreen_graphics_pipeline = &self.fullscreen_technique.passes[0].graphics_pipeline;
        command_buffer.bind_graphics_pipeline(fullscreen_graphics_pipeline);
        command_buffer.bind_descriptor_set(
            &self.fullscreen_descriptor_set,
            fullscreen_graphics_pipeline.raw_layout(),
            0,
        );

        // XXX: Set final texture index as the first instance parameter?
        command_buffer.draw(3, 1, 0, 0);
        // XXX: Set scissor, viewport?

        command_buffer.end_rendering();

        command_buffer.end()?;

        self.renderer.queue_command_buffer(command_buffer);

        Ok(())
    }
}
