use std::{ops::Deref, sync::Arc};

use anyhow::Result;
use parking_lot::{RwLock, RwLockReadGuard};

use crate::{
    buffer::*, descriptor_set::*, device::*, escape::*, image::*, pipeline::*, sampler::*,
};

struct ResourceTracker<T> {
    terminal: Terminal<T>,
}

impl<T> ResourceTracker<T> {
    fn new() -> Self {
        Self {
            terminal: Terminal::new(),
        }
    }

    fn escape(&self, resource: T) -> Escape<T>
    where
        T: Sized,
    {
        Escape::escape(resource, &self.terminal)
    }

    fn destroy(&mut self, destroy: impl FnMut(T)) {
        self.terminal.drain().for_each(destroy);
    }
}

struct ResourceHub {
    buffers: ResourceTracker<Buffer>,
    images: ResourceTracker<Image>,
    samplers: ResourceTracker<Sampler>,
    graphics_pipelines: ResourceTracker<GraphicsPipeline>,
    descriptor_set_layouts: ResourceTracker<DescriptorSetLayout>,
}

impl ResourceHub {
    fn new() -> Self {
        Self {
            buffers: ResourceTracker::new(),
            images: ResourceTracker::new(),
            samplers: ResourceTracker::new(),
            graphics_pipelines: ResourceTracker::new(),
            descriptor_set_layouts: ResourceTracker::new(),
        }
    }

    unsafe fn cleanup(&mut self) {
        self.buffers.destroy(|b| b.destroy());
        self.images.destroy(|i| i.destroy());
        self.samplers.destroy(|s| s.destroy());
        self.graphics_pipelines.destroy(|p| p.destroy());
        self.descriptor_set_layouts.destroy(|l| l.destroy());
    }
}

struct ResourceGuard {
    device: Device,
    resource_hub: RwLock<ResourceHub>,
}

impl ResourceGuard {
    fn new(device: Device, resource_hub: ResourceHub) -> Self {
        Self {
            device,
            resource_hub: RwLock::new(resource_hub),
        }
    }

    fn cleanup_resources(&self) {
        unsafe {
            self.resource_hub.write().cleanup();
        }
    }
}

impl Drop for ResourceGuard {
    fn drop(&mut self) {
        self.cleanup_resources();
    }
}

#[derive(Clone)]
pub struct DeviceGuard {
    guard: Arc<ResourceGuard>,
}

impl DeviceGuard {
    pub fn new(device: Device) -> Self {
        Self {
            guard: Arc::new(ResourceGuard::new(device, ResourceHub::new())),
        }
    }

    fn hub(&self) -> RwLockReadGuard<ResourceHub> {
        self.guard.resource_hub.read()
    }

    pub fn device(&self) -> &Device {
        &self.guard.device
    }

    pub fn cleanup_resources(&mut self) {
        self.guard.cleanup_resources();
    }
}

impl Deref for DeviceGuard {
    type Target = Device;
    fn deref(&self) -> &Device {
        &self.guard.device
    }
}

pub struct Factory {
    device: DeviceGuard,
}

impl Factory {
    pub fn new(device: DeviceGuard) -> Self {
        Self { device }
    }

    pub fn create_buffer(&self, desc: BufferDesc) -> Result<Escape<Buffer>> {
        let buffer =
            unsafe { Buffer::create(self.device.clone(), self.device.allocator().clone(), desc)? };
        Ok(self.device.hub().buffers.escape(buffer))
    }

    pub fn create_image(&self, desc: ImageDesc) -> Result<Escape<Image>> {
        let image =
            unsafe { Image::create(self.device.clone(), self.device.allocator().clone(), desc)? };
        Ok(self.device.hub().images.escape(image))
    }

    pub fn create_sampler(&self, desc: SamplerDesc) -> Result<Escape<Sampler>> {
        let sampler = unsafe { Sampler::create(self.device.clone(), desc)? };
        Ok(self.device.hub().samplers.escape(sampler))
    }

    pub fn create_graphics_pipeline(
        &self,
        desc: GraphicsPipelineDesc,
    ) -> Result<Escape<GraphicsPipeline>> {
        let graphics_pipeline =
            unsafe { GraphicsPipeline::create(self.device.clone(), self, desc)? };
        Ok(self
            .device
            .hub()
            .graphics_pipelines
            .escape(graphics_pipeline))
    }

    pub fn create_descriptor_set_layout(
        &self,
        desc: DescriptorSetLayoutDesc,
    ) -> Result<Escape<DescriptorSetLayout>> {
        let layout = unsafe { DescriptorSetLayout::create(self.device.clone(), desc)? };
        Ok(self.device.hub().descriptor_set_layouts.escape(layout))
    }
}
