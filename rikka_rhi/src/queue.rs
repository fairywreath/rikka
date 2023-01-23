use std::sync::Arc;

use anyhow::Result;
use ash::vk;

#[derive(Debug, Clone, Copy)]
pub(crate) struct QueueFamily {
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
pub(crate) struct QueueFamilyIndices {
    pub graphics: QueueFamily,
    pub present: QueueFamily,
    pub compute: QueueFamily,
    pub transfer: QueueFamily,
}
