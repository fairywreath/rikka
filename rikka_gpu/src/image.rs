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
    barrier::ResourceState,
    command_buffer,
    device::Device,
    frame::{self, FrameThreadPoolsManager},
    pipeline::*,
    sampler::Sampler,
    types::*,
};

pub struct ImageDesc {
    pub width: u32,
    pub height: u32,
    pub depth: u32,

    pub array_layer_count: u32,
    pub mip_level_count: u32,

    format: vk::Format,
    image_type: vk::ImageType,
    flags: vk::ImageCreateFlags,
    // name: String,
}

pub struct Image {
    device: Arc<Device>,

    raw: vk::Image,
    raw_view: vk::ImageView,

    allocator: Arc<Mutex<Allocator>>,
    allocation: Option<Allocation>,

    resource_state: ResourceState,

    desc: ImageDesc,
    // linked_sampler: Option<Handle<Sampler>>,
    sampler: Option<Arc<Sampler>>,
}

impl Image {
    pub fn raw(&self) -> vk::Image {
        self.raw
    }

    pub fn raw_view(&self) -> vk::ImageView {
        self.raw_view
    }

    pub fn has_linked_sampler(&self) -> bool {
        self.sampler.is_some()
    }

    pub fn linked_sampler(&self) -> Option<Arc<Sampler>> {
        self.sampler.clone()
    }
}
