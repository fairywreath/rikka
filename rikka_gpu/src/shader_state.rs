use std::{any, ffi::CString, str::FromStr, sync::Arc};

use anyhow::{Context, Result};

use rikka_core::{ash, vk};
use rikka_shader::{compiler, reflect::*, types::*};

use crate::{
    command_buffer,
    constants::{self, NUM_COMMAND_BUFFERS_PER_THREAD},
    device::Device,
    frame::{self, FrameThreadPoolsManager},
    types::*,
};

pub use rikka_shader::types::ShaderStageType;

pub fn shader_stage_type_to_vk_flags(shader_type: ShaderStageType) -> vk::ShaderStageFlags {
    match shader_type {
        ShaderStageType::Vertex => vk::ShaderStageFlags::VERTEX,
        ShaderStageType::Fragment => vk::ShaderStageFlags::FRAGMENT,
        ShaderStageType::Geometry => vk::ShaderStageFlags::GEOMETRY,
        ShaderStageType::Compute => vk::ShaderStageFlags::COMPUTE,
        ShaderStageType::Mesh => vk::ShaderStageFlags::MESH_NV,
        ShaderStageType::Task => vk::ShaderStageFlags::TASK_NV,
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ShaderStageDataReadType {
    Bytes,
    BytesFromFile,
    SourceFromString,
    SourceFromFile,
}

#[derive(Clone, Debug)]
pub struct ShaderStageDesc {
    // XXX: Make this private
    pub read_type: ShaderStageDataReadType,
    pub file_name: Option<String>,
    pub source: Option<String>,
    pub bytes: Option<Vec<u8>>,
    pub shader_type: ShaderStageType,
}

impl ShaderStageDesc {
    pub fn new_from_source_file(file_name: &str, shader_type: ShaderStageType) -> Self {
        Self {
            read_type: ShaderStageDataReadType::SourceFromFile,
            file_name: Some(String::from_str(file_name).unwrap()),
            source: None,
            bytes: None,
            shader_type,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ShaderStateDesc {
    pub stages: Vec<ShaderStageDesc>,
}

impl ShaderStateDesc {
    pub fn new() -> Self {
        Self { stages: vec![] }
    }

    pub fn add_stage(mut self, stage: ShaderStageDesc) -> Self {
        self.stages.push(stage);
        self
    }
}

pub struct ShaderState {
    device: Arc<Device>,
    raw_stages: Vec<vk::PipelineShaderStageCreateInfo>,

    // XXX: Remove this hack and add entry point when creating the actual pipeline itself.
    entry_point_name: CString,

    reflection: ShaderReflection,
}

impl ShaderState {
    pub fn new(device: Arc<Device>, desc: ShaderStateDesc) -> Result<Self> {
        if desc.stages.is_empty() {
            return Err(anyhow::anyhow!("Shader stages from description is empty!"));
        }

        let mut raw_stages = Vec::<vk::PipelineShaderStageCreateInfo>::new();
        let entry_point_name = CString::new("main").unwrap();

        let mut reflections = Vec::new();

        for stage in &desc.stages {
            let (shader_module, reflection) =
                unsafe { Self::create_shader_module(device.as_ref(), stage)? };

            raw_stages.push(
                vk::PipelineShaderStageCreateInfo::builder()
                    .stage(shader_stage_type_to_vk_flags(stage.shader_type))
                    .module(shader_module)
                    .name(&entry_point_name)
                    .build(),
            );
            reflections.push(reflection);
        }

        let reflection = merge_reflections(&reflections)?;

        Ok(Self {
            device,
            raw_stages,
            entry_point_name,
            reflection,
        })
    }

    pub fn vulkan_shader_stages(&self) -> &[vk::PipelineShaderStageCreateInfo] {
        &self.raw_stages
    }

    pub fn num_stages(&self) -> u32 {
        self.raw_stages.len() as u32
    }

    pub fn reflection(&self) -> &ShaderReflection {
        &self.reflection
    }

    unsafe fn create_shader_module(
        device: &Device,
        desc: &ShaderStageDesc,
    ) -> Result<(vk::ShaderModule, ShaderReflection)> {
        let bytes = {
            match desc.read_type {
                ShaderStageDataReadType::SourceFromFile => {
                    let source_file_name = desc.file_name.as_ref().unwrap();
                    let destination_file_name = source_file_name.to_owned() + ".spv";
                    let shader_data = compiler::compile_shader_through_glslangvalidator_cli(
                        source_file_name,
                        destination_file_name.as_str(),
                        desc.shader_type,
                    )
                    .context("Failed to compile shader through glslangvalidator cli!")?;
                    shader_data.bytes
                }
                ShaderStageDataReadType::SourceFromString => {
                    todo!()
                }
                ShaderStageDataReadType::BytesFromFile => {
                    let shader_data = compiler::read_shader_binary_file(
                        desc.file_name.as_ref().unwrap().as_str(),
                    )?;
                    shader_data.bytes
                }
                ShaderStageDataReadType::Bytes => desc.bytes.as_ref().unwrap().clone(),
            }
        };

        let reflection = reflect_spirv_data(&bytes)?;

        let mut cursor = std::io::Cursor::new(bytes);
        let code = ash::util::read_spv(&mut cursor)?;

        let create_info = vk::ShaderModuleCreateInfo::builder().code(&code);
        let shader_module = device.raw().create_shader_module(&create_info, None)?;

        Ok((shader_module, reflection))
    }

    unsafe fn destroy_shader_modules(
        device: &Device,
        stages: &[vk::PipelineShaderStageCreateInfo],
    ) {
        for stage in stages {
            unsafe { device.raw().destroy_shader_module(stage.module, None) };
        }
    }
}

impl Drop for ShaderState {
    fn drop(&mut self) {
        unsafe { Self::destroy_shader_modules(self.device.as_ref(), &self.raw_stages) };
    }
}
