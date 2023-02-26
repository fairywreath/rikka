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
    pub min_flter: vk::Filter,
    pub mag_filter: vk::Filter,
    pub mipmap_mode: vk::SamplerMipmapMode,
    pub address_mode_u: vk::SamplerAddressMode,
    pub address_mode_v: vk::SamplerAddressMode,
    pub address_mode_w: vk::SamplerAddressMode,
    pub reduction_mode: vk::SamplerReductionMode,
}

impl SamplerDesc {
    pub fn new() -> Self {
        todo!()
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
            .min_filter(desc.min_flter)
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
