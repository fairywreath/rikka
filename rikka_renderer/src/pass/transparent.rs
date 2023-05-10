use anyhow::Result;

use rikka_gpu::command_buffer::CommandBuffer;
use rikka_graph::types::RenderPass;

use crate::scene_renderer::types::*;

pub struct TransparentPass {
    mesh_instances: Vec<MeshInstance>,
}

impl RenderPass for TransparentPass {
    fn render(&self, command_buffer: &CommandBuffer) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "TransparentPass"
    }
}
