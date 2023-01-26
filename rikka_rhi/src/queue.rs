use std::sync::Arc;

use anyhow::Result;
use ash::vk;

use crate::{
    command_buffer::CommandBuffer,
    device::Device,
    synchronization::{Semaphore, SemaphoreType},
};

#[derive(Debug, Clone, Copy)]
pub struct QueueFamily {
    index: u32,
    properties: vk::QueueFamilyProperties,
    supports_present: bool,
}

impl QueueFamily {
    pub fn new(index: u32, properties: vk::QueueFamilyProperties, supports_present: bool) -> Self {
        Self {
            index,
            properties,
            supports_present,
        }
    }

    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn supports_graphics(&self) -> bool {
        self.properties
            .queue_flags
            .contains(vk::QueueFlags::GRAPHICS)
    }

    pub fn supports_present(&self) -> bool {
        self.supports_present
    }

    pub fn supports_compute(&self) -> bool {
        self.properties
            .queue_flags
            .contains(vk::QueueFlags::COMPUTE)
    }

    pub fn supports_transfer(&self) -> bool {
        self.properties
            .queue_flags
            .contains(vk::QueueFlags::TRANSFER)
    }

    pub fn supports_timestamps(&self) -> bool {
        self.properties.timestamp_valid_bits > 0
    }

    pub fn queue_count(&self) -> u32 {
        self.properties.queue_count
    }
}
pub struct QueueFamilyIndices {
    pub graphics: QueueFamily,
    pub present: QueueFamily,
    pub compute: QueueFamily,
    pub transfer: QueueFamily,
}

pub struct SemaphoreSubmitInfo<'a> {
    pub semaphore: &'a Semaphore,
    pub stage_mask: vk::PipelineStageFlags2,
    // For timeline semaphores.
    pub value: Option<u64>,
}
pub struct Queue {
    device: Arc<Device>,
    raw: vk::Queue,
}

impl Queue {
    pub fn new(device: Arc<Device>, raw: vk::Queue) -> Self {
        Self { device, raw }
    }

    pub fn submit(
        &self,
        command_buffer: &CommandBuffer,
        wait_semaphores: Vec<SemaphoreSubmitInfo>,
        signal_semaphores: Vec<SemaphoreSubmitInfo>,
    ) -> Result<()> {
        let wait_semaphores_info = wait_semaphores
            .iter()
            .map(|submit_info| {
                let mut semaphore_submit_info = vk::SemaphoreSubmitInfo::builder()
                    .semaphore(submit_info.semaphore.raw_clone())
                    .stage_mask(submit_info.stage_mask);

                if submit_info.semaphore.semaphore_type() == SemaphoreType::Timeline {
                    semaphore_submit_info = semaphore_submit_info.value(submit_info.value.unwrap());
                }
                semaphore_submit_info.build()
            })
            .collect::<Vec<_>>();

        let signal_semaphores_info = signal_semaphores
            .iter()
            .map(|submit_info| {
                let mut semaphore_submit_info = vk::SemaphoreSubmitInfo::builder()
                    .semaphore(submit_info.semaphore.raw_clone())
                    .stage_mask(submit_info.stage_mask);

                if submit_info.semaphore.semaphore_type() == SemaphoreType::Timeline {
                    semaphore_submit_info = semaphore_submit_info.value(submit_info.value.unwrap());
                }
                semaphore_submit_info.build()
            })
            .collect::<Vec<_>>();

        let command_buffer_submit_info =
            vk::CommandBufferSubmitInfo::builder().command_buffer(command_buffer.raw_clone());

        let submit_info = vk::SubmitInfo2::builder()
            .wait_semaphore_infos(&wait_semaphores_info[..])
            .signal_semaphore_infos(&signal_semaphores_info[..])
            .command_buffer_infos(std::slice::from_ref(&command_buffer_submit_info))
            .build();

        unsafe {
            self.device.raw().queue_submit2(
                self.raw,
                std::slice::from_ref(&submit_info),
                vk::Fence::null(),
            )?
        };

        Ok(())
    }

    pub fn raw_clone(&self) -> vk::Queue {
        self.raw.clone()
    }
}
