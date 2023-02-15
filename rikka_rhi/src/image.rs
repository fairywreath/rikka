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
    constants::{self, NUM_COMMAND_BUFFERS_PER_THREAD},
    device::Device,
    frame::{self, FrameThreadPoolsManager},
    graphics_pipeline::*,
    rhi::RHIContext,
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

    aw: vk::Image,
    raw_view: vk::ImageView,

    allocator: Arc<Mutex<Allocator>>,
    allocation: Option<Allocation>,

    resource_state: ResourceState,

    desc: ImageDesc,
    // linked_sampler: Option<Handle<Sampler>>,
}
