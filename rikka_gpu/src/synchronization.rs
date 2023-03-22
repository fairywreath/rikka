use std::sync::Arc;

use anyhow::Result;
use rikka_core::vk;

use crate::device::Device;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SemaphoreType {
    Binary,
    Timeline,
}

pub struct Semaphore {
    device: Arc<Device>,
    raw: vk::Semaphore,

    semaphore_type: SemaphoreType,
}

impl Semaphore {
    pub fn new(device: Arc<Device>, semaphore_type: SemaphoreType) -> Result<Self> {
        let semaphore_info = vk::SemaphoreCreateInfo::builder();

        let mut semaphore_type_info =
            vk::SemaphoreTypeCreateInfo::builder().semaphore_type(vk::SemaphoreType::BINARY);
        if semaphore_type == SemaphoreType::Timeline {
            semaphore_type_info = semaphore_type_info.semaphore_type(vk::SemaphoreType::TIMELINE);
        }
        let semaphore_info = semaphore_info.push_next(&mut semaphore_type_info);

        let raw = unsafe { device.raw().create_semaphore(&semaphore_info, None)? };

        Ok(Self {
            device,
            raw,
            semaphore_type,
        })
    }

    pub fn raw(&self) -> vk::Semaphore {
        self.raw
    }

    pub fn raw_clone(&self) -> vk::Semaphore {
        self.raw.clone()
    }

    pub fn semaphore_type(&self) -> SemaphoreType {
        self.semaphore_type
    }
}

impl Drop for Semaphore {
    fn drop(&mut self) {
        unsafe {
            self.device.raw().destroy_semaphore(self.raw, None);
        }
    }
}
