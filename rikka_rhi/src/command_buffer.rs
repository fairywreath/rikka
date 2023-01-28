use std::sync::Arc;

use anyhow::Result;
use ash::vk;

use crate::{device::Device, queue::QueueFamily, rhi::RHIContext};

pub struct CommandPool {
    raw: vk::CommandPool,
    device: Arc<Device>,
}

impl CommandPool {
    pub fn new(device: Arc<Device>, queue_family_index: u32) -> Result<Self> {
        let command_pool_info =
            vk::CommandPoolCreateInfo::builder().queue_family_index(queue_family_index);

        let command_pool = unsafe {
            let command_pool = device.raw().create_command_pool(&command_pool_info, None)?;
            device
                .raw()
                .reset_command_pool(command_pool, vk::CommandPoolResetFlags::empty())?;

            command_pool
        };

        Ok(Self {
            raw: command_pool,
            device: device,
        })
    }

    pub fn allocate_command_buffers(
        &self,
        level: vk::CommandBufferLevel,
        count: u32,
    ) -> Result<Vec<vk::CommandBuffer>> {
        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(self.raw)
            .level(level)
            .command_buffer_count(count);

        let command_buffers =
            unsafe { self.device.raw().allocate_command_buffers(&allocate_info)? };

        Ok(command_buffers)
    }
    pub fn allocate_command_buffer(
        &self,
        level: vk::CommandBufferLevel,
    ) -> Result<vk::CommandBuffer> {
        let command_buffers = self.allocate_command_buffers(level, 1)?;
        Ok(command_buffers[0])
    }

    pub fn reset(&self) {
        unsafe {
            self.device
                .raw()
                .reset_command_pool(self.raw, vk::CommandPoolResetFlags::empty())
                .expect("Failed to reset Vulkan command pool!");
        }
    }

    pub fn raw(&self) -> vk::CommandPool {
        self.raw
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        unsafe { self.device.raw().destroy_command_pool(self.raw, None) }
    }
}

pub struct CommandBuffer {
    device: Arc<Device>,
    raw: vk::CommandBuffer,

    pub(crate) is_recording: bool,
    pub(crate) is_secondary: bool,

    meta_data: CommandBufferMetaData,
    // Reference to pipeline?
    // pipeline: vk::Pipeline,
}

impl CommandBuffer {
    pub(crate) fn new(device: Arc<Device>, command_buffer: vk::CommandBuffer) -> Self {
        Self {
            device: device.clone(),
            raw: command_buffer,

            is_recording: false,
            is_secondary: false,

            meta_data: CommandBufferMetaData { index: 0 },
        }
    }

    pub fn raw(&self) -> vk::CommandBuffer {
        self.raw
    }
}

// Information for CommandBufferManager
pub struct CommandBufferMetaData {
    // index to command buffer array in CommandBufferManager
    pub(crate) index: u32,
}

pub struct CommandBufferManager {
    device: Arc<Device>,

    command_buffers: Vec<CommandBuffer>,
    secondary_command_buffers: Vec<CommandBuffer>,

    num_used_command_buffers: Vec<u32>,
    num_used_secondary_command_buffers: Vec<u32>,

    // Equal to number of threads.
    num_pools_per_frame: u32,

    num_command_buffers_per_thread: u32,
}

pub struct CommandBufferManagerDesc {}

impl CommandBufferManager {
    pub fn new(device: Arc<Device>) {
        todo!()
    }
}

impl Drop for CommandBufferManager {
    fn drop(&mut self) {}
}
