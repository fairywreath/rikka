use std::sync::Arc;

use anyhow::Result;
use ash::vk;

use crate::{device::Device, queue::QueueFamily};

pub struct CommandPool {}

pub struct CommandBuffer {
    device: Arc<Device>,
    raw: vk::CommandBuffer,
}

impl CommandBuffer {
    pub fn raw(&self) -> &vk::CommandBuffer {
        &self.raw
    }
    pub fn raw_clone(&self) -> vk::CommandBuffer {
        self.raw.clone()
    }
}
