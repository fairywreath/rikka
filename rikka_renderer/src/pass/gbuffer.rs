use anyhow::Result;
use std::sync::Arc;

use rikka_gpu::command_buffer::CommandBuffer;
use rikka_graph::types::RenderPass;

use crate::scene_renderer::types::*;

pub struct GBufferPass {
    mesh_instances: Vec<MeshInstance>,
}

impl GBufferPass {
    pub fn setup(&mut self, meshes: &[Arc<Mesh>]) {
        self.mesh_instances.clear();
        for mesh in meshes {
            self.mesh_instances.push(MeshInstance::new(mesh.clone(), 1));
        }
    }
}

impl RenderPass for GBufferPass {
    fn pre_render(&self, command_buffer: &CommandBuffer) -> Result<()> {
        todo!()
    }

    fn render(&self, command_buffer: &CommandBuffer) -> Result<()> {
        for mesh_instance in &self.mesh_instances {
            let mesh = &mesh_instance.mesh;

            if mesh.transparent() {
                continue;
            }

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
        todo!()
    }

    fn name(&self) -> &str {
        "GBufferPass"
    }
}
