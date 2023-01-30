use log::{debug, error, info, log_enabled, warn, Level};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use ash::vk;

use crate::{
    command_buffer,
    constants::{self, NUM_COMMAND_BUFFERS_PER_THREAD},
    device::Device,
    frame::{self, FrameThreadPoolsManager},
};

// Mimics VkRenderPass, used with dynamic rendering.
pub struct RenderPassState {}

// Mimics VkFramebuffer, used with dynamic rendering.
// Contains texture attachments.
pub struct FramebufferState {}

pub struct GraphicsPipelineDesc {}
pub struct GraphicsPipeline {
    raw: vk::Pipeline,
}

impl GraphicsPipeline {
    pub fn raw(&self) -> vk::Pipeline {
        self.raw
    }
}

pub struct GraphicsPipelineCreationError {}
