use anyhow::Result;

use rikka_gpu::command_buffer::CommandBuffer;
use rikka_graph::types::RenderPass;

use crate::scene_renderer::types::*;

pub struct PBRLightingPass {
    mesh: Mesh,
    // renderer: Arc<Renderer>,
}

impl RenderPass for PBRLightingPass {
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
        "PBRLightingPass"
    }
}
