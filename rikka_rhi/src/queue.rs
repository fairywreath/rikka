use std::sync::{Arc, Weak};

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
    family_index: u32,
}

impl Queue {
    pub fn new(device: Arc<Device>, raw: vk::Queue, family_index: u32) -> Self {
        Self {
            device,
            raw,
            family_index,
        }
    }

    pub fn submit(
        &self,
        command_buffers: &Vec<Weak<CommandBuffer>>,
        wait_semaphores: Vec<SemaphoreSubmitInfo>,
        signal_semaphores: Vec<SemaphoreSubmitInfo>,
    ) -> Result<()> {
        let wait_semaphores_info = wait_semaphores
            .iter()
            .map(|submit_info| {
                let mut semaphore_submit_info = vk::SemaphoreSubmitInfo::builder()
                    .semaphore(submit_info.semaphore.raw_clone())
                    .stage_mask(submit_info.stage_mask)
                    .value(0);

                if submit_info.semaphore.semaphore_type() == SemaphoreType::Timeline {
                    semaphore_submit_info = semaphore_submit_info.value(
                        submit_info
                            .value
                            .expect("Timeline wait semaphore requires a value!"),
                    );
                }
                semaphore_submit_info.build()
            })
            .collect::<Vec<_>>();

        let signal_semaphores_info = signal_semaphores
            .iter()
            .map(|submit_info| {
                let mut semaphore_submit_info = vk::SemaphoreSubmitInfo::builder()
                    .semaphore(submit_info.semaphore.raw_clone())
                    .stage_mask(submit_info.stage_mask)
                    .value(0);

                if submit_info.semaphore.semaphore_type() == SemaphoreType::Timeline {
                    semaphore_submit_info = semaphore_submit_info.value(
                        submit_info
                            .value
                            .expect("Timeline signal semaphore requires a value!"),
                    );
                }
                semaphore_submit_info.build()
            })
            .collect::<Vec<_>>();

        let command_buffer_submit_infos = command_buffers
            .iter()
            .map(|command_buffer| {
                vk::CommandBufferSubmitInfo::builder()
                    .command_buffer(command_buffer.upgrade().unwrap().raw())
                    .build()
            })
            .collect::<Vec<_>>();

        let submit_info = vk::SubmitInfo2::builder()
            .wait_semaphore_infos(&wait_semaphores_info[..])
            .signal_semaphore_infos(&signal_semaphores_info[..])
            .command_buffer_infos(&command_buffer_submit_infos[..])
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

    pub fn family_index(&self) -> u32 {
        self.family_index
    }
}
