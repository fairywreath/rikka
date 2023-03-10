use std::{
    mem::{align_of, size_of_val, swap},
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
    barriers::ResourceState,
    command_buffer,
    device::Device,
    frame::{self, FrameThreadPoolsManager},
    pipeline::*,
    sampler::Sampler,
    swapchain::Swapchain,
    types::*,
};

pub struct TransferManager {}
