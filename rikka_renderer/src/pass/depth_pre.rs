use anyhow::Result;

use rikka_gpu::command_buffer::CommandBuffer;
use rikka_graph::types::RenderPass;

use crate::scene_renderer::types::*;

pub struct DepthPrePass {
    mesh_instances: Vec<MeshInstance>,
    // renderer: Arc<Renderer>,
}

impl DepthPrePass {
    pub fn new() -> Result<Self> {
        todo!()
    }

    pub fn prepare(&mut self) -> Result<()> {
        todo!()
    }
}

impl RenderPass for DepthPrePass {
    fn pre_render(&self, command_buffer: &CommandBuffer) -> Result<()> {
        Ok(())
    }

    fn render(&self, command_buffer: &CommandBuffer) -> Result<()> {
        for mesh_instance in &self.mesh_instances {
            let mesh = &mesh_instance.mesh;

            // XXX: Do not bind pipeline ber draw, sort based on material and bind sparringly
            // XXX FIXME: The process of obtaining the pipeline from the mesh and material
            command_buffer.bind_graphics_pipeline(
                &mesh.pbr_material.material.render_technique.passes
                    [mesh_instance.material_pass_index]
                    .graphics_pipeline,
            );

            mesh.draw(command_buffer);
        }

        Ok(())
    }

    fn resize(&self, width: u32, height: u32) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "DepthPrePass"
    }
}
