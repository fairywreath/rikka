use std::{
    mem::{align_of, size_of_val, swap},
    ops::Deref,
    sync::Arc,
};

use anyhow::{Context, Error, Result};
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc, Allocator},
    MemoryLocation,
};
use rikka_core::vk;

use crate::{
    barriers::ResourceState,
    command_buffer,
    device::Device,
    frame::{self, FrameThreadPoolsManager},
    pipeline::*,
    sampler::Sampler,
    swapchain::Swapchain,
    types::*,
};

pub struct TransferManager {
    device: Arc<Device>,
    queue: Queue,
    command_pools: Vec<CommandPool>,
    command_buffers: Vec<CommandBuffer>,
}
