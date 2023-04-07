use std::sync::Arc;

use anyhow::{Context, Result};
use rikka_core::vk;

use crate::{
    constants,
    descriptor_set::*,
    device::Device,
    escape::Escape,
    factory::{DeviceGuard, Factory},
    shader_state::*,
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

    // pub shader_stages: Vec<vk::PipelineShaderStageCreateInfo>,
    pub shader_state: ShaderStateDesc,

    // XXX: Need a handle to the primary type `DescriptorSetLayout` here?
    // pub descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,

    // XXX: Handle to viewport state required?
    pub width: u32,
    pub height: u32,
    // XXX: pipeline cache somewhere? or handle this completely internally?
}

impl GraphicsPipelineDesc {
    pub fn new() -> Self {
        Self {
            vertex_input_state: VertexInputState::new(),
            rasterization_state: RasterizationState::new(),
            depth_stencil_state: DepthStencilState::new(),
            blend_states: vec![],
            primitive_topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            // XXX: Only need formats for this, maybe use a simpler version of this structure?
            rendering_state: RenderingState::new_dimensionless(),
            vertex_const_size: None,
            fragment_const_size: None,
            // shader_stages: vec![],
            // descriptor_set_layouts: vec![],
            width: 1,
            height: 1,

            shader_state: ShaderStateDesc::new(),
        }
    }

    pub fn set_shader_state(mut self, shader_state: ShaderStateDesc) -> Self {
        self.shader_state = shader_state;
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

    pub fn set_rasterization_state(mut self, rasterization_state: RasterizationState) -> Self {
        self.rasterization_state = rasterization_state;
        self
    }

    pub fn set_vertex_input_state(mut self, vertex_input_state: VertexInputState) -> Self {
        self.vertex_input_state = vertex_input_state;
        self
    }

    // Not used as shader and descriptor layout information is obtained through shader reflection.
    // pub fn set_shader_stages(
    //     mut self,
    //     shader_stages: &[vk::PipelineShaderStageCreateInfo],
    // ) -> Self {
    //     self.shader_stages = shader_stages.to_vec();
    //     self
    // }

    // pub fn set_descriptor_set_layouts(
    //     mut self,
    //     descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    // ) -> Self {
    //     self.descriptor_set_layouts = descriptor_set_layouts;
    //     self
    // }

    // pub fn add_descriptor_set_layout(
    //     mut self,
    //     descriptor_set_layout: vk::DescriptorSetLayout,
    // ) -> Self {
    //     self.descriptor_set_layouts.push(descriptor_set_layout);
    //     self
    // }
}

pub struct GraphicsPipeline {
    device: DeviceGuard,

    raw: vk::Pipeline,
    raw_layout: vk::PipelineLayout,

    // XXX: Do we need this?
    desc: GraphicsPipelineDesc,

    descriptor_set_layouts: Vec<Escape<DescriptorSetLayout>>,
}

impl GraphicsPipeline {
    pub unsafe fn create(
        device: DeviceGuard,
        factory: &Factory,
        desc: GraphicsPipelineDesc,
    ) -> Result<Self> {
        // Create shader modules
        let shader_state = ShaderState::new(device.clone(), desc.shader_state.clone())?;

        // Create descriptor set layouts
        let descriptor_sets = &shader_state.reflection().descriptor_sets;

        let mut layout_descs = Vec::with_capacity(descriptor_sets.len());
        for set in descriptor_sets {
            // XXX: Make this bindless texture array check nicer
            //      Need GPU class for this to work... use shared bindless texture layout for all pipelines
            if set.bindings[0].index == constants::BINDLESS_SET_SAMPLED_IMAGE_INDEX {
                let bindless_descriptor_set_layout_desc = DescriptorSetLayoutDesc::new()
                    .set_flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
                    .set_bindless(true)
                    .add_binding(DescriptorBinding::new(
                        vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                        constants::BINDLESS_SET_SAMPLED_IMAGE_INDEX,
                        constants::MAX_NUM_BINDLESS_RESOURCECS,
                        vk::ShaderStageFlags::FRAGMENT,
                    ))
                    .add_binding(DescriptorBinding::new(
                        vk::DescriptorType::STORAGE_IMAGE,
                        constants::BINDLESS_SET_STORAGE_IMAGE_INDEX,
                        constants::MAX_NUM_BINDLESS_RESOURCECS,
                        vk::ShaderStageFlags::FRAGMENT,
                    ));
                layout_descs.push(bindless_descriptor_set_layout_desc);
                continue;
            }

            let layout_desc = DescriptorSetLayoutDesc::new()
                .set_bindings(set.bindings.clone())
                .set_bindless(false)
                .set_dynamic(false);
            layout_descs.push(layout_desc);
        }

        let descriptor_set_layouts = layout_descs
            .into_iter()
            .map(|desc| Ok(factory.create_descriptor_set_layout(desc)?))
            .collect::<Result<Vec<_>>>()?;

        let vulkan_descriptor_set_layouts = descriptor_set_layouts
            .iter()
            .map(|layout| layout.raw())
            .collect::<Vec<_>>();

        // XXX: Read layout from cache

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
            .set_layouts(&vulkan_descriptor_set_layouts)
            .push_constant_ranges(&push_constant_ranges);

        let pipeline_layout = device
            .raw()
            .create_pipeline_layout(&pipeline_layout_info, None)
            .context("Failed to create vulkan pipeline layout!")?;

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
            .max_depth(1.0)
            .build()];
        let scissors = [vk::Rect2D::builder()
            .offset(vk::Offset2D { x: 0, y: 0 })
            .extent(vk::Extent2D {
                width: desc.width,
                height: desc.height,
            })
            .build()];

        // XXX: Add dynamic viewport(to handle window resizing)
        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&viewports)
            .scissors(&scissors);

        let color_blend_attachments = {
            if !desc.blend_states.is_empty() {
                assert_eq!(
                    desc.blend_states.len(),
                    desc.rendering_state.color_attachments.len(),
                );

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
            .stages(&shader_state.vulkan_shader_stages())
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

        let raw = device
            .raw()
            .create_graphics_pipelines(
                vk::PipelineCache::null(),
                std::slice::from_ref(&pipeline_info),
                None,
            )
            .map_err(|e| e.1)?[0];

        Ok(Self {
            raw,
            raw_layout: pipeline_layout,
            desc,
            device,
            descriptor_set_layouts,
        })
    }

    pub unsafe fn destroy(self) {
        self.device.raw().destroy_pipeline(self.raw, None);
        self.device
            .raw()
            .destroy_pipeline_layout(self.raw_layout, None);
    }

    pub fn raw(&self) -> vk::Pipeline {
        self.raw
    }

    pub fn raw_layout(&self) -> vk::PipelineLayout {
        self.raw_layout
    }

    pub fn descriptor_set_layouts(&self) -> &[Escape<DescriptorSetLayout>] {
        &self.descriptor_set_layouts
    }
}
