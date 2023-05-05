use anyhow::Result;

use rikka_gpu::{command_buffer::CommandBuffer, escape::Handle, image::Image};
use rikka_graph::types::RenderPass;

use crate::scene_renderer::types::*;

pub struct GPUDoFData {
    textures_indices: [u32; 4],
    znear: f32,
    zfar: f32,
    aperture: f32,
    focal_length: f32,
    plane_in_focus: f32,
}

pub struct DepthOfFieldPass {
    mesh: Mesh,
    // renderer: Arc<Renderer>,
    scene_mips: Option<Handle<Image>>,

    znear: f32,
    zfar: f32,
    aperture: f32,
    focal_length: f32,
    plane_in_focus: f32,
}

impl RenderPass for DepthOfFieldPass {
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
        "DepthOfFieldPass"
    }
}
