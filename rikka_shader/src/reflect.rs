use std::collections::HashMap;

use anyhow::Result;
use spirv_reflect::{types::*, ShaderModule};

use rikka_core::vk;

use crate::types::*;

pub(crate) trait ReflectInto<T>: Sized {
    fn reflect_into(&self) -> Result<T> {
        Err(anyhow::anyhow!("Unsupported reflect format conversion!"))
    }
}

pub(crate) fn convert_shader_stage(stage: ReflectShaderStageFlags) -> vk::ShaderStageFlags {
    use vk::ShaderStageFlags;

    let mut bits = ShaderStageFlags::empty();

    if stage.contains(ReflectShaderStageFlags::VERTEX) {
        bits |= ShaderStageFlags::VERTEX;
    }
    if stage.contains(ReflectShaderStageFlags::FRAGMENT) {
        bits |= ShaderStageFlags::FRAGMENT;
    }
    if stage.contains(ReflectShaderStageFlags::GEOMETRY) {
        bits |= ShaderStageFlags::GEOMETRY;
    }
    if stage.contains(ReflectShaderStageFlags::TESSELLATION_CONTROL) {
        bits |= ShaderStageFlags::TESSELLATION_CONTROL;
    }
    if stage.contains(ReflectShaderStageFlags::TESSELLATION_EVALUATION) {
        bits |= ShaderStageFlags::TESSELLATION_EVALUATION;
    }
    if stage.contains(ReflectShaderStageFlags::COMPUTE) {
        bits |= ShaderStageFlags::COMPUTE;
    }
    if stage.contains(ReflectShaderStageFlags::MESH_BIT_NV) {
        bits |= ShaderStageFlags::MESH_NV;
    }
    if stage.contains(ReflectShaderStageFlags::TASK_BIT_NV) {
        bits |= ShaderStageFlags::TASK_NV;
    }
    if stage.contains(ReflectShaderStageFlags::RAYGEN_BIT_NV) {
        bits |= ShaderStageFlags::RAYGEN_NV
    }
    if stage.contains(ReflectShaderStageFlags::ANY_HIT_BIT_NV) {
        bits |= ShaderStageFlags::ANY_HIT_NV;
    }
    if stage.contains(ReflectShaderStageFlags::CALLABLE_BIT_NV) {
        bits |= ShaderStageFlags::CALLABLE_NV;
    }
    if stage.contains(ReflectShaderStageFlags::CLOSEST_HIT_BIT_NV) {
        bits |= ShaderStageFlags::CLOSEST_HIT_NV;
    }
    if stage.contains(ReflectShaderStageFlags::INTERSECTION_BIT_NV) {
        bits |= ShaderStageFlags::INTERSECTION_NV;
    }

    bits
}

impl ReflectInto<vk::DescriptorType> for ReflectDescriptorType {
    fn reflect_into(&self) -> Result<vk::DescriptorType> {
        use vk::DescriptorType;
        use ReflectDescriptorType::*;

        match *self {
            Sampler => Ok(DescriptorType::SAMPLER),
            CombinedImageSampler => Ok(DescriptorType::COMBINED_IMAGE_SAMPLER),
            SampledImage => Ok(DescriptorType::SAMPLED_IMAGE),
            StorageImage => Ok(DescriptorType::STORAGE_IMAGE),
            UniformTexelBuffer => Ok(DescriptorType::UNIFORM_TEXEL_BUFFER),
            StorageTexelBuffer => Ok(DescriptorType::STORAGE_TEXEL_BUFFER),
            UniformBuffer => Ok(DescriptorType::UNIFORM_BUFFER),
            StorageBuffer => Ok(DescriptorType::STORAGE_BUFFER),
            UniformBufferDynamic => Ok(DescriptorType::UNIFORM_BUFFER_DYNAMIC),
            StorageBufferDynamic => Ok(DescriptorType::STORAGE_BUFFER_DYNAMIC),
            InputAttachment => Ok(DescriptorType::INPUT_ATTACHMENT),
            AccelerationStructureKHR => Ok(DescriptorType::ACCELERATION_STRUCTURE_NV),
            Undefined => Err(anyhow::anyhow!("Undfined descriptor type!")),
        }
    }
}

pub fn reflect_spirv_data(spirv_data: &[u8]) -> Result<ShaderReflection> {
    if let Ok(ref mut module) = ShaderModule::load_u8_data(spirv_data) {
        let shader_stages = convert_shader_stage(module.get_shader_stage());
        let descriptor_sets = module.enumerate_descriptor_sets(None).unwrap();

        let descriptor_sets = descriptor_sets
            .into_iter()
            .map(|set| {
                let bindings = set
                    .bindings
                    .into_iter()
                    .map(|binding| {
                        Ok(DescriptorBinding {
                            descriptor_type: binding.descriptor_type.reflect_into()?,
                            index: binding.binding,
                            count: binding.count,
                            shader_stage_flags: shader_stages,
                        })
                    })
                    .collect::<Result<Vec<_>>>();

                Ok(DescriptorSet {
                    bindings: bindings?,
                    index: set.set,
                    shader_stages,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(ShaderReflection { descriptor_sets })
    } else {
        Err(anyhow::anyhow!("Failed to load spirv data"))
    }
}

pub fn merge_reflections(parse_results: &[ShaderReflection]) -> Result<ShaderReflection> {
    let mut merged_sets = Vec::new();

    for parse_result in parse_results {
        let descriptor_sets = &parse_result.descriptor_sets;
        for (n, set) in descriptor_sets.iter().enumerate() {
            match merged_sets
                .get(n)
                .map(|existing| compare_set(set, existing))
            {
                None => merged_sets.push(set.clone()),
                Some(SetEquality::NotEqual) => {
                    return Err(anyhow::anyhow!("Mismatched bindings!"));
                }
                Some(SetEquality::SupersetOf) => {
                    let existing_set = &merged_sets[n];

                    let mut new_set = set.clone();
                    new_set.shader_stages |= existing_set.shader_stages;
                    // Descriptor binding stage flags is set on a per-set level now rather than per-binding
                    // XXX: Handle this to make it per-binding
                    for binding in new_set.bindings.iter_mut() {
                        binding.shader_stage_flags |= new_set.shader_stages;
                    }
                    merged_sets[n] = new_set;
                }
                Some(SetEquality::Equal) | Some(SetEquality::SubsetOf) => {
                    for binding in merged_sets[n].bindings.iter_mut() {
                        binding.shader_stage_flags |= set.shader_stages;
                    }
                }
            }
        }
    }

    Ok(ShaderReflection {
        descriptor_sets: merged_sets,
    })
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
enum SetEquality {
    Equal,
    SubsetOf,
    SupersetOf,
    NotEqual,
}

fn compare_set(lhv: &DescriptorSet, rhv: &DescriptorSet) -> SetEquality {
    if lhv.index != rhv.index {
        return SetEquality::NotEqual;
    }

    let mut lhv_bindings = HashMap::new();
    lhv.bindings.iter().for_each(|binding| {
        lhv_bindings.insert(binding.index, binding);
    });

    let mut rhv_bindings = HashMap::new();
    rhv.bindings.iter().for_each(|binding| {
        rhv_bindings.insert(binding.index, binding);
    });

    let predicate = if lhv.bindings.len() == rhv.bindings.len() {
        SetEquality::Equal
    } else if lhv.bindings.len() > rhv.bindings.len() {
        SetEquality::SupersetOf
    } else {
        SetEquality::SubsetOf
    };

    for (key, lhv_value) in lhv_bindings {
        if let Some(rhv_value) = rhv_bindings.get(&key) {
            match compare_bindings(lhv_value, rhv_value) {
                BindingEquality::Equal => {}
                BindingEquality::NotEqual | BindingEquality::SameBindingNonEqual => {
                    return SetEquality::NotEqual;
                }
            }
        } else if predicate == SetEquality::Equal || predicate == SetEquality::SubsetOf {
            return SetEquality::NotEqual;
        }
    }

    predicate
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
enum BindingEquality {
    Equal,
    SameBindingNonEqual,
    NotEqual,
}

fn compare_bindings(lhv: &DescriptorBinding, rhv: &DescriptorBinding) -> BindingEquality {
    if lhv.index == rhv.index
        && lhv.count == rhv.count
        && lhv.descriptor_type == rhv.descriptor_type
    {
        return BindingEquality::Equal;
    } else if lhv.index == rhv.index {
        return BindingEquality::SameBindingNonEqual;
    }

    BindingEquality::NotEqual
}
