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

pub struct BlendState {
    pub source_color: vk::BlendFactor,
    pub destination_color: vk::BlendFactor,
    pub color_operation: vk::BlendOp,

    pub source_alpha: vk::BlendFactor,
    pub dest_alpha: vk::BlendFactor,
    pub blend_operation: vk::BlendOp,

    pub enable: bool,

    // If false, alpha blends are equal to color blends.
    pub separate_alpha: bool,
}

impl BlendState {
    pub fn default() -> Self {
        todo!()
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum RenderPassOperation {
    DontCare,
    Load,
    Clear,
}

impl RenderPassOperation {
    pub fn vk_attachment_load_op(&self) -> vk::AttachmentLoadOp {
        match self {
            Self::DontCare => vk::AttachmentLoadOp::DONT_CARE,
            Self::Load => vk::AttachmentLoadOp::LOAD,
            Self::Clear => vk::AttachmentLoadOp::CLEAR,
        }
    }
}

pub struct RenderColorAttachment {
    // XXX: Not needed for dynamic rendering(needed for VkRenderPass however). Remove?
    pub format: vk::Format,

    pub image_layout: vk::ImageLayout,
    pub operation: RenderPassOperation,
    pub clear_value: vk::ClearColorValue,

    // XXX: Need struct for non-owning view?
    pub image_view: vk::ImageView,
}

impl RenderColorAttachment {
    pub fn new() -> Self {
        Self {
            format: vk::Format::UNDEFINED,
            image_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            operation: RenderPassOperation::DontCare,
            clear_value: vk::ClearColorValue::default(),
            image_view: vk::ImageView::null(),
        }
    }

    pub fn set_format(mut self, format: vk::Format) -> Self {
        self.format = format;
        self
    }

    pub fn set_image_layout(mut self, image_layout: vk::ImageLayout) -> Self {
        self.image_layout = image_layout;
        self
    }

    pub fn set_operation(mut self, operation: RenderPassOperation) -> Self {
        self.operation = operation;
        self
    }

    pub fn set_clear_value(mut self, clear_value: vk::ClearColorValue) -> Self {
        self.clear_value = clear_value;
        self
    }

    pub fn set_image_view(mut self, image_view: vk::ImageView) -> Self {
        self.image_view = image_view;
        self
    }
}

pub struct RenderDepthStencilAttachment {
    // XXX: Not needed for dynamic rendering(needed for VkRenderPass however). Remove?
    pub format: vk::Format,

    pub image_layout: vk::ImageLayout,
    pub depth_operation: RenderPassOperation,
    pub stencil_operation: RenderPassOperation,
    pub clear_value: vk::ClearDepthStencilValue,

    // XXX: Need struct for non-owning view?
    pub image_view: vk::ImageView,
}

impl RenderDepthStencilAttachment {
    pub fn new() -> Self {
        Self {
            format: vk::Format::UNDEFINED,
            image_layout: vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
            depth_operation: RenderPassOperation::DontCare,
            stencil_operation: RenderPassOperation::DontCare,
            clear_value: vk::ClearDepthStencilValue::default(),
            image_view: vk::ImageView::null(),
        }
    }

    pub fn set_format(mut self, format: vk::Format) -> Self {
        self.format = format;
        self
    }

    pub fn set_image_layout(mut self, image_layout: vk::ImageLayout) -> Self {
        self.image_layout = image_layout;
        self
    }

    pub fn set_depth_operation(mut self, operation: RenderPassOperation) -> Self {
        self.depth_operation = operation;
        self
    }

    pub fn set_stencil_operation(mut self, operation: RenderPassOperation) -> Self {
        self.stencil_operation = operation;
        self
    }

    pub fn set_clear_value(mut self, clear_value: vk::ClearDepthStencilValue) -> Self {
        self.clear_value = clear_value;
        self
    }

    pub fn set_image_view(mut self, image_view: vk::ImageView) -> Self {
        self.image_view = image_view;
        self
    }
}

pub struct RenderingState {
    pub color_attachments: Vec<RenderColorAttachment>,
    pub depth_attachment: Option<RenderDepthStencilAttachment>,

    // XXX: Framebuffer info. Need FramebufferState that also contains non owning image views?
    pub width: u32,
    pub height: u32,
}

impl RenderingState {
    pub fn new(width: u32, height: u32) -> Self {
        RenderingState {
            width,
            height,
            color_attachments: Vec::new(),
            depth_attachment: None,
        }
    }

    pub fn add_color_attachment(mut self, attachment: RenderColorAttachment) -> Self {
        self.color_attachments.push(attachment);
        self
    }

    pub fn set_depth_attachment(mut self, attachment: RenderDepthStencilAttachment) -> Self {
        self.depth_attachment = Some(attachment);
        self
    }
}

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
