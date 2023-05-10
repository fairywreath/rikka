use anyhow::Result;
use std::sync::Arc;

use rikka_gpu::command_buffer::CommandBuffer;
use rikka_graph::types::RenderPass;

use crate::scene_renderer::types::*;

pub struct DepthPrePass {
    mesh_instances: Vec<MeshInstance>,
}

impl DepthPrePass {
    pub fn setup(&mut self, meshes: &[Arc<Mesh>]) {
        self.mesh_instances.clear();
        for mesh in meshes {
            // Depth pre pass is at index 0 of the render technique passes
            self.mesh_instances.push(MeshInstance::new(mesh.clone(), 0));
        }
    }
}

impl RenderPass for DepthPrePass {
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

            // mesh.draw(command_buffer);
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "DepthPrePass"
    }
}
