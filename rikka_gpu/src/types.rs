use std::sync::Arc;

use ash::vk;

use crate::{image::Image, sampler::Sampler};

pub enum PipelineStage {
    DrawIndirect,
    VertexInput,
    VertexShader,
    FragmentShader,
    RenderTarget,
    ComputeShader,
    Transfer,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ResourceUsageType {
    Immutable,
    Dynamic,
    Stream,
    Staging,
}

#[derive(Clone, Copy)]
pub struct BlendState {
    pub source_color: vk::BlendFactor,
    pub destination_color: vk::BlendFactor,
    pub color_operation: vk::BlendOp,

    pub source_alpha: vk::BlendFactor,
    pub destination_alpha: vk::BlendFactor,
    pub alpha_operation: vk::BlendOp,

    pub enable: bool,

    // If false, alpha blends are equal to color blends.
    pub separate_alpha: bool,
}

impl BlendState {
    pub fn new() -> Self {
        todo!()
    }
}

#[derive(Clone, Copy)]
pub struct VertexAttribute {
    pub location: u32,
    pub binding: u32,
    pub offset: u32,
    pub format: vk::Format,
}

#[derive(Clone, Copy)]
pub struct VertexStream {
    pub binding: u32,
    pub stride: u32,
    pub input_rate: vk::VertexInputRate,
}

#[derive(Clone)]
pub struct VertexInputState {
    pub vertex_attributes: Vec<VertexAttribute>,
    pub vertex_streams: Vec<VertexStream>,
}

impl VertexInputState {
    pub fn new() -> Self {
        Self {
            vertex_attributes: vec![],
            vertex_streams: vec![],
        }
    }

    pub fn add_vertex_attribute(
        mut self,
        location: u32,
        binding: u32,
        offset: u32,
        format: vk::Format,
    ) -> Self {
        self.vertex_attributes.push(VertexAttribute {
            location,
            binding,
            offset,
            format,
        });
        self
    }

    pub fn add_vertex_stream(
        mut self,
        binding: u32,
        stride: u32,
        input_rate: vk::VertexInputRate,
    ) -> Self {
        self.vertex_streams.push(VertexStream {
            binding,
            stride,
            input_rate,
        });
        self
    }
}

#[derive(Clone, Copy)]
pub struct RasterizationState {
    pub cull_mode: vk::CullModeFlags,
    pub front_face: vk::FrontFace,
    pub polygon_mode: vk::PolygonMode,
}

impl RasterizationState {
    pub fn new() -> Self {
        Self {
            cull_mode: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            polygon_mode: vk::PolygonMode::FILL,
        }
    }

    pub fn set_cull_mode(mut self, cull_mode: vk::CullModeFlags) -> Self {
        self.cull_mode = cull_mode;
        self
    }

    pub fn set_front_face(mut self, front_face: vk::FrontFace) -> Self {
        self.front_face = front_face;
        self
    }

    pub fn set_polygon_mode(mut self, polygon_mode: vk::PolygonMode) -> Self {
        self.polygon_mode = polygon_mode;
        self
    }
}

#[derive(Clone, Copy)]
pub struct DepthStencilState {
    pub depth_test_enable: bool,
    pub depth_write_enable: bool,
    pub depth_compare: vk::CompareOp,
    // XXX: Add stencil states
}

impl DepthStencilState {
    pub fn new() -> Self {
        Self {
            depth_test_enable: true,
            depth_write_enable: true,
            depth_compare: vk::CompareOp::LESS_OR_EQUAL,
        }
    }

    pub fn set_depth_test(mut self, enable: bool) -> Self {
        self.depth_test_enable = enable;
        self
    }

    pub fn set_depth_write(mut self, enable: bool) -> Self {
        self.depth_write_enable = enable;
        self
    }

    pub fn set_depth_compare(mut self, depth_compare: vk::CompareOp) -> Self {
        self.depth_compare = depth_compare;
        self
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

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
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
            image_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
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

#[derive(Clone)]
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

    pub fn new_dimensionless() -> Self {
        RenderingState {
            width: 1,
            height: 1,
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

#[derive(Clone)]
pub struct ImageResourceUpdate {
    pub frame: u64,

    // XXX: Need strong reference here?
    pub image: Option<Arc<Image>>,
    pub sampler: Option<Arc<Sampler>>,
}
