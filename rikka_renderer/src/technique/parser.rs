use anyhow::Result;
use serde_derive::{Deserialize, Serialize};

use rikka_core::vk;
use rikka_gpu::{pipeline::*, shader_state::*, types as gpu_types};
use rikka_graph::graph::*;

use crate::renderer::*;

// XXX: Put some of these types in the GPU layer instead of using raw (unserializable) vulkan types

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum VertexStreamRate {
    Vertex,
    Instance,
}

impl Into<vk::VertexInputRate> for VertexStreamRate {
    fn into(self) -> vk::VertexInputRate {
        match self {
            Self::Vertex => vk::VertexInputRate::VERTEX,
            Self::Instance => vk::VertexInputRate::INSTANCE,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VertexInput {
    pub attribute_location: u32,
    pub attribute_binding: u32,
    pub attribute_offset: u32,
    // XXX: use more specific enum/format
    pub attribute_format: i32,

    pub stream_binding: u32,
    pub stream_stride: u32,
    pub stream_rate: VertexStreamRate,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CompareOp {
    Never,
    Less,
    Equal,
    LessOrEqual,
    Greater,
    NotEqual,
    GreaterOrEqual,
    Always,
}

impl Into<vk::CompareOp> for CompareOp {
    fn into(self) -> vk::CompareOp {
        match self {
            Self::Never => vk::CompareOp::NEVER,
            Self::Less => vk::CompareOp::LESS,
            Self::Equal => vk::CompareOp::EQUAL,
            Self::LessOrEqual => vk::CompareOp::LESS_OR_EQUAL,
            Self::Greater => vk::CompareOp::GREATER,
            Self::NotEqual => vk::CompareOp::NOT_EQUAL,
            Self::GreaterOrEqual => vk::CompareOp::GREATER_OR_EQUAL,
            Self::Always => vk::CompareOp::ALWAYS,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DepthState {
    pub write_enable: bool,
    pub test_enable: bool,
    pub compare_op: CompareOp,
}

impl Into<gpu_types::DepthStencilState> for DepthState {
    fn into(self) -> gpu_types::DepthStencilState {
        gpu_types::DepthStencilState {
            depth_test_enable: self.write_enable,
            depth_write_enable: self.write_enable,
            depth_compare: self.compare_op.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shader {
    pub shader_type: ShaderStageType,
    pub file_name: String,
    // XXX: Properly handle shader source file includes
    // pub includes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CullMode {
    None,
    Front,
    Back,
    FrontAndBack,
}

impl Into<vk::CullModeFlags> for CullMode {
    fn into(self) -> vk::CullModeFlags {
        match self {
            Self::None => vk::CullModeFlags::NONE,
            Self::Front => vk::CullModeFlags::FRONT,
            Self::Back => vk::CullModeFlags::BACK,
            Self::FrontAndBack => vk::CullModeFlags::FRONT_AND_BACK,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FrontFace {
    Clockwise,
    CounterClockwise,
}

impl Into<vk::FrontFace> for FrontFace {
    fn into(self) -> vk::FrontFace {
        match self {
            Self::Clockwise => vk::FrontFace::CLOCKWISE,
            Self::CounterClockwise => vk::FrontFace::COUNTER_CLOCKWISE,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PolygonMode {
    Fill,
    Line,
    Point,
}

impl Into<vk::PolygonMode> for PolygonMode {
    fn into(self) -> vk::PolygonMode {
        match self {
            Self::Fill => vk::PolygonMode::FILL,
            Self::Line => vk::PolygonMode::LINE,
            Self::Point => vk::PolygonMode::POINT,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RasterizationState {
    pub cull_mode: CullMode,
    pub front_face: FrontFace,
    pub polygon_mode: PolygonMode,
}

impl Into<gpu_types::RasterizationState> for RasterizationState {
    fn into(self) -> gpu_types::RasterizationState {
        gpu_types::RasterizationState {
            cull_mode: self.cull_mode.into(),
            front_face: self.front_face.into(),
            polygon_mode: self.polygon_mode.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pipeline {
    pub name: String,
    pub render_pass: String,
    pub shaders: Vec<Shader>,
    pub vertex_inputs: Vec<VertexInput>,
    pub depth_state: Option<DepthState>,
    pub rasterization_state: Option<RasterizationState>,
    // pub blend_state: Vec<BlendState>,
}

impl Pipeline {
    pub fn into_graphics_pipeline_desc(
        self,
        renderer: &Renderer,
        render_graph: &Graph,
    ) -> Result<GraphicsPipelineDesc> {
        let mut desc = GraphicsPipelineDesc::new()
            .set_extent(renderer.extent().width, renderer.extent().height);

        let mut shader_state = ShaderStateDesc::new();
        for shader in self.shaders {
            shader_state = shader_state.add_stage(ShaderStageDesc::new_from_source_file(
                shader.file_name.as_str(),
                shader.shader_type,
            ));
        }
        desc = desc.set_shader_state(shader_state);

        let mut vertex_input_state = gpu_types::VertexInputState::new();
        for vertex_input in self.vertex_inputs {
            vertex_input_state = vertex_input_state
                .add_vertex_attribute(
                    vertex_input.attribute_location,
                    vertex_input.attribute_binding,
                    vertex_input.attribute_location,
                    vk::Format::from_raw(vertex_input.attribute_format),
                )
                .add_vertex_stream(
                    vertex_input.stream_binding,
                    vertex_input.stream_stride,
                    vertex_input.stream_rate.into(),
                );
        }
        desc = desc.set_vertex_input_state(vertex_input_state);

        if self.render_pass == "swapchain" {
            log::debug!(
                "Set up swapchain rendering state for pipeline {}",
                self.name.as_str()
            );
            desc = desc.set_rendering_state(
                gpu_types::RenderingState::new_dimensionless()
                    .add_color_attachment(
                        gpu_types::RenderColorAttachment::new()
                            .set_format(renderer.gpu().swapchain().format()),
                    )
                    // XXX: Swapchain does not generally have depth attahcment
                    //      Removs this hardcoded depth attachment
                    .set_depth_attachment(
                        gpu_types::RenderDepthStencilAttachment::new()
                            .set_format(vk::Format::D32_SFLOAT),
                    ),
            )
        } else {
            desc = desc.set_rendering_state(
                render_graph
                    .access_node_by_name(self.render_pass.as_str())?
                    .rendering_state
                    .clone()
                    .unwrap(),
            );
        }

        if let Some(depth_state) = self.depth_state {
            desc = desc.set_depth_stencil_state(depth_state.into());
        }

        if let Some(rasterization_state) = self.rasterization_state {
            desc = desc.set_rasterization_state(rasterization_state.into());
        }

        Ok(desc)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Technique {
    pub name: String,
    pub pipelines: Vec<Pipeline>,
}

impl Technique {
    pub fn into_render_technique_desc(
        self,
        renderer: &Renderer,
        render_graph: &Graph,
    ) -> Result<RenderTechniqueDesc> {
        let mut desc = RenderTechniqueDesc::new();

        for pipeline in self.pipelines {
            desc = desc.add_graphics_pipeline(
                pipeline.into_graphics_pipeline_desc(renderer, render_graph)?,
            );
        }

        Ok(desc)
    }
}

pub fn parse_from_string(
    string: &str,
    renderer: &Renderer,
    render_graph: &Graph,
) -> Result<RenderTechniqueDesc> {
    let technique: Technique = serde_json::from_str(string)?;
    Ok(technique.into_render_technique_desc(renderer, render_graph)?)
}

pub fn parse_from_file(
    file_name: &str,
    renderer: &Renderer,
    render_graph: &Graph,
) -> Result<RenderTechniqueDesc> {
    let file_contents = std::fs::read_to_string(file_name)?;
    parse_from_string(&file_contents, renderer, render_graph)
}
