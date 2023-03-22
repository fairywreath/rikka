use std::hash::Hash;

use rikka_core::vk;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShaderStageType {
    Vertex,
    Fragment,
    Geometry,
    Compute,
    Mesh,
    Task,
}

impl ShaderStageType {
    // XXX: Change to using into
    pub fn to_glslang_compiler_extension(&self) -> String {
        match self {
            Self::Vertex => String::from("vert"),
            Self::Fragment => String::from("frag"),
            Self::Geometry => String::from("geom"),
            Self::Compute => String::from("comp"),
            Self::Mesh => String::from("mesh"),
            Self::Task => String::from("task"),
        }
    }

    pub fn to_glslang_stage_defines(&self) -> String {
        match self {
            Self::Vertex => String::from("VERTEX"),
            Self::Fragment => String::from("FRAGMENT"),
            Self::Geometry => String::from("GEOMETRY"),
            Self::Compute => String::from("COMPUTE"),
            Self::Mesh => String::from("MESH"),
            Self::Task => String::from("TASK"),
        }
    }

    pub fn to_vulkan_shader_stage_flag(&self) -> vk::ShaderStageFlags {
        use vk::ShaderStageFlags;

        match self {
            Self::Vertex => ShaderStageFlags::VERTEX,
            Self::Fragment => ShaderStageFlags::FRAGMENT,
            Self::Geometry => ShaderStageFlags::GEOMETRY,
            Self::Compute => ShaderStageFlags::COMPUTE,
            Self::Mesh => ShaderStageFlags::MESH_NV,
            Self::Task => ShaderStageFlags::TASK_NV,
        }
    }
}

pub struct ShaderData {
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct DescriptorBinding {
    pub descriptor_type: vk::DescriptorType,
    pub index: u32,
    pub count: u32,
    pub shader_stage_flags: vk::ShaderStageFlags,
}

impl DescriptorBinding {
    pub fn new(
        descriptor_type: vk::DescriptorType,
        index: u32,
        count: u32,
        shader_stage_flags: vk::ShaderStageFlags,
    ) -> Self {
        Self {
            descriptor_type,
            index,
            count,
            shader_stage_flags,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DescriptorSet {
    pub bindings: Vec<DescriptorBinding>,
    pub index: u32,
    pub shader_stages: vk::ShaderStageFlags,
}

#[derive(Debug)]
pub struct ShaderReflection {
    pub descriptor_sets: Vec<DescriptorSet>,
}
