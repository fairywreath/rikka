use anyhow::Result;

use rikka_gpu::command_buffer::CommandBuffer;
use rikka_graph::types::RenderPass;

use crate::scene_renderer::types::*;

pub struct GBufferPass {
    mesh_instances: Vec<MeshInstance>,
    // renderer: Arc<Renderer>,
}

impl RenderPass for GBufferPass {
    fn pre_render(&self, command_buffer: &CommandBuffer) -> Result<()> {
        Ok(())
    }

    fn render(&self, command_buffer: &CommandBuffer) -> Result<()> {
        Ok(())
    }

    fn resize(&self, width: u32, height: u32) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "GBufferPass"
    }
}
