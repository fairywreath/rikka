use std::{
    mem::{align_of, size_of_val},
    ops::Deref,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Error, Result};
use ash::vk;
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc, Allocator},
    MemoryLocation,
};

use crate::{
    command_buffer,
    constants::{self, NUM_COMMAND_BUFFERS_PER_THREAD},
    device::Device,
    frame::{self, FrameThreadPoolsManager},
    pipeline::*,
    types::*,
};

pub struct SamplerDesc {
    pub min_filter: vk::Filter,
    pub mag_filter: vk::Filter,
    pub mipmap_mode: vk::SamplerMipmapMode,
    pub address_mode_u: vk::SamplerAddressMode,
    pub address_mode_v: vk::SamplerAddressMode,
    pub address_mode_w: vk::SamplerAddressMode,
    pub reduction_mode: vk::SamplerReductionMode,
}

impl SamplerDesc {
    pub fn new() -> Self {
        Self {
            min_filter: vk::Filter::LINEAR,
            mag_filter: vk::Filter::LINEAR,
            mipmap_mode: vk::SamplerMipmapMode::LINEAR,
            address_mode_u: vk::SamplerAddressMode::REPEAT,
            address_mode_v: vk::SamplerAddressMode::REPEAT,
            address_mode_w: vk::SamplerAddressMode::REPEAT,
            reduction_mode: vk::SamplerReductionMode::WEIGHTED_AVERAGE,
        }
    }

    pub fn set_min_filter(mut self, min_filter: vk::Filter) -> Self {
        self.min_filter = min_filter;
        self
    }

    pub fn set_mag_filter(mut self, mag_filter: vk::Filter) -> Self {
        self.mag_filter = mag_filter;
        self
    }
}

pub struct Sampler {
    device: Arc<Device>,
    raw: vk::Sampler,
    desc: SamplerDesc,
}

impl Sampler {
    pub fn new(device: Arc<Device>, desc: SamplerDesc) -> Result<Sampler> {
        let mut create_info = vk::SamplerCreateInfo::builder()
            .min_filter(desc.min_filter)
            .mag_filter(desc.mag_filter)
            .mipmap_mode(desc.mipmap_mode)
            .address_mode_u(desc.address_mode_u)
            .address_mode_v(desc.address_mode_v)
            .address_mode_u(desc.address_mode_u)
            .mip_lod_bias(1.0)
            .anisotropy_enable(false)
            .compare_enable(false)
            .compare_op(vk::CompareOp::ALWAYS)
            .min_lod(1.0)
            .max_lod(16.0)
            .border_color(vk::BorderColor::INT_OPAQUE_WHITE)
            .unnormalized_coordinates(false);

        let mut sampler_reduction_info = vk::SamplerReductionModeCreateInfo::builder();
        if desc.reduction_mode != vk::SamplerReductionMode::WEIGHTED_AVERAGE {
            sampler_reduction_info = sampler_reduction_info.reduction_mode(desc.reduction_mode);
            create_info = create_info.push_next(&mut sampler_reduction_info);
        }

        let raw = unsafe {
            device
                .raw()
                .create_sampler(&create_info, None)
                .with_context(|| format!("Failed to create sampler!"))?
        };

        Ok(Self { device, raw, desc })
    }

    pub fn raw(&self) -> vk::Sampler {
        self.raw
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        unsafe {
            self.device.raw().destroy_sampler(self.raw, None);
        }
    }
}

impl Deref for Sampler {
    type Target = vk::Sampler;
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}
