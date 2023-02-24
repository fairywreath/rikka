use std::sync::Arc;

use anyhow::{Context, Result};
use ash::vk::{self, Win32KeyedMutexAcquireReleaseInfoKHRBuilder};

use crate::{
    command_buffer,
    constants::{self, NUM_COMMAND_BUFFERS_PER_THREAD},
    device::Device,
    frame::{self, FrameThreadPoolsManager},
    types::*,
};

pub struct GraphicsPipelineDesc {
    pub vertex_input_state: VertexInputState,
    pub rasterization_state: RasterizationState,
    pub depth_stencil_state: DepthStencilState,
    pub blend_states: Vec<BlendState>,
    pub primitive_topology: vk::PrimitiveTopology,

    // XXX: Is this required?
    pub rendering_state: RenderingState,

    pub vertex_const_size: Option<u32>,
    pub fragment_const_size: Option<u32>,

    pub shader_stages: Vec<vk::PipelineShaderStageCreateInfo>,
    // XXX: Descriptor set layouts here

    // XXX: Handle to viewport state required?
    pub width: u32,
    pub height: u32,
    // XXX: pipeline cache somewhere?
}

impl GraphicsPipelineDesc {
    pub fn new() -> Self {
        Self {
            vertex_input_state: VertexInputState::new(),
            rasterization_state: RasterizationState::new(),
            depth_stencil_state: DepthStencilState::new(),
            blend_states: vec![],
            primitive_topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            rendering_state: RenderingState::new_dimensionless(),
            vertex_const_size: None,
            fragment_const_size: None,
            shader_stages: vec![],
            width: 1,
            height: 1,
        }
    }

    pub fn set_shader_stages(
        mut self,
        shader_stages: &[vk::PipelineShaderStageCreateInfo],
    ) -> Self {
        self.shader_stages = shader_stages.to_vec();
        self
    }

    pub fn set_rendering_state(mut self, rendering_sate: RenderingState) -> Self {
        self.rendering_state = rendering_sate;
        self
    }

    pub fn set_extent(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

pub struct GraphicsPipeline {
    raw: vk::Pipeline,
    raw_layout: vk::PipelineLayout,
    desc: GraphicsPipelineDesc,
    device: Arc<Device>,
}

impl GraphicsPipeline {
    pub fn new(device: Arc<Device>, desc: GraphicsPipelineDesc) -> Result<Self> {
        // XXX: Create vulkan pipeline layout
        // XXX: Read layout from cache?

        // XXX: Properly initialize descriptor set layouts
        let descriptor_set_layouts = Vec::<vk::DescriptorSetLayout>::new();

        let push_constant_ranges = {
            let mut push_constant_ranges = Vec::<vk::PushConstantRange>::new();
            if desc.vertex_const_size.is_some() {
                push_constant_ranges.push(
                    vk::PushConstantRange::builder()
                        .stage_flags(vk::ShaderStageFlags::VERTEX)
                        .offset(0)
                        .size(desc.vertex_const_size.unwrap())
                        .build(),
                );
            }
            if desc.fragment_const_size.is_some() {
                push_constant_ranges.push(
                    vk::PushConstantRange::builder()
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                        .offset(0)
                        .size(desc.fragment_const_size.unwrap())
                        .build(),
                );
            }

            push_constant_ranges
        };

        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&descriptor_set_layouts)
            .push_constant_ranges(&push_constant_ranges);

        let pipeline_layout = unsafe {
            device
                .raw()
                .create_pipeline_layout(&pipeline_layout_info, None)
                .context("Failed to create vulkan pipeline layout!")?
        };

        // Create vulkan pipeline

        let vertex_attributes = desc
            .vertex_input_state
            .vertex_attributes
            .iter()
            .map(|attribute| {
                vk::VertexInputAttributeDescription::builder()
                    .location(attribute.location)
                    .binding(attribute.binding)
                    .format(attribute.format)
                    .build()
            })
            .collect::<Vec<_>>();
        let vertex_bindings = desc
            .vertex_input_state
            .vertex_streams
            .iter()
            .map(|stream| {
                vk::VertexInputBindingDescription::builder()
                    .binding(stream.binding)
                    .stride(stream.stride)
                    .input_rate(stream.input_rate)
                    .build()
            })
            .collect::<Vec<_>>();
        let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(&vertex_attributes)
            .vertex_binding_descriptions(&vertex_bindings);

        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(desc.primitive_topology)
            .primitive_restart_enable(false);

        let viewports = [vk::Viewport::builder()
            .x(0.0)
            .y(0.0)
            .width(desc.width as f32)
            .height(desc.height as f32)
            .min_depth(0.0)
            .max_depth(0.0)
            .build()];
        let scissors = [vk::Rect2D::builder()
            .offset(vk::Offset2D { x: 0, y: 0 })
            .extent(vk::Extent2D {
                width: desc.width,
                height: desc.height,
            })
            .build()];
        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&viewports)
            .scissors(&scissors);

        let color_blend_attachments = {
            if !desc.blend_states.is_empty() {
                // XXX: Check length of blend states is equal to length of rendering state color attachments.
                let color_blend_attachments = desc
                    .blend_states
                    .iter()
                    .map(|blend_state| {
                        let mut color_blend_attachment =
                            vk::PipelineColorBlendAttachmentState::builder()
                                .color_write_mask(vk::ColorComponentFlags::RGBA)
                                .blend_enable(blend_state.enable)
                                .src_color_blend_factor(blend_state.source_color)
                                .dst_color_blend_factor(blend_state.destination_color)
                                .color_blend_op(blend_state.color_operation);

                        if blend_state.separate_alpha {
                            color_blend_attachment = color_blend_attachment
                                .src_alpha_blend_factor(blend_state.source_alpha)
                                .dst_alpha_blend_factor(blend_state.destination_alpha)
                                .alpha_blend_op(blend_state.alpha_operation);
                        } else {
                            color_blend_attachment = color_blend_attachment
                                .src_alpha_blend_factor(blend_state.source_color)
                                .dst_alpha_blend_factor(blend_state.destination_color)
                                .alpha_blend_op(blend_state.color_operation);
                        }

                        color_blend_attachment.build()
                    })
                    .collect::<Vec<_>>();

                color_blend_attachments
            } else {
                vec![
                    vk::PipelineColorBlendAttachmentState::builder()
                        .blend_enable(false)
                        .color_write_mask(vk::ColorComponentFlags::RGBA)
                        .build();
                    desc.rendering_state.color_attachments.len()
                ]
            }
        };
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::COPY)
            .attachments(&color_blend_attachments)
            .blend_constants([0.0, 0.0, 0.0, 0.0]);

        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(desc.depth_stencil_state.depth_test_enable)
            .depth_write_enable(desc.depth_stencil_state.depth_write_enable)
            .depth_compare_op(desc.depth_stencil_state.depth_compare)
            .depth_bounds_test_enable(false)
            .min_depth_bounds(0.0)
            .max_depth_bounds(0.0);

        let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .sample_shading_enable(false)
            .min_sample_shading(1.0);

        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
            .polygon_mode(desc.rasterization_state.polygon_mode)
            .cull_mode(desc.rasterization_state.cull_mode)
            .front_face(desc.rasterization_state.front_face)
            .line_width(1.0)
            .depth_bias_enable(false)
            .depth_clamp_enable(false);

        // let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        // let dynamic_state =
        //     vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_states);

        // XXX: Tesselation state?

        let color_attachment_formats = desc
            .rendering_state
            .color_attachments
            .iter()
            .map(|color_attachment| color_attachment.format)
            .collect::<Vec<_>>();
        let mut pipeline_rendering_info = vk::PipelineRenderingCreateInfo::builder()
            .view_mask(0)
            .color_attachment_formats(&color_attachment_formats)
            .depth_attachment_format(match desc.rendering_state.depth_attachment {
                Some(depth_attachment) => depth_attachment.format,
                None => vk::Format::UNDEFINED,
            })
            .stencil_attachment_format(vk::Format::UNDEFINED);

        let pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&desc.shader_stages)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .color_blend_state(&color_blend_state)
            .depth_stencil_state(&depth_stencil_state)
            .multisample_state(&multisample_state)
            .rasterization_state(&rasterization_state)
            // .dynamic_state(&dynamic_state)
            .layout(pipeline_layout)
            .push_next(&mut pipeline_rendering_info)
            .build();

        let raw = unsafe {
            device
                .raw()
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    std::slice::from_ref(&pipeline_info),
                    None,
                )
                .map_err(|e| e.1)?[0]
        };

        Ok(Self {
            raw,
            raw_layout: pipeline_layout,
            desc,
            device,
        })
    }

    pub unsafe fn destroy(&self, device: &Device) {
        device.raw().destroy_pipeline(self.raw, None);
        device.raw().destroy_pipeline_layout(self.raw_layout, None);
    }

    pub fn raw(&self) -> vk::Pipeline {
        self.raw
    }
}

impl Drop for GraphicsPipeline {
    fn drop(&mut self) {
        unsafe {
            self.destroy(self.device.as_ref());
        }
    }
}
