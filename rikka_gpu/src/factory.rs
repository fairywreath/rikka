use std::{ops::Deref, sync::Arc};

use anyhow::Result;
use parking_lot::RwLock;

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
    descriptor_pools: ResourceTracker<DescriptorPool>,
}

impl ResourceHub {
    fn new() -> Self {
        Self {
            buffers: ResourceTracker::new(),
            images: ResourceTracker::new(),
            samplers: ResourceTracker::new(),
            graphics_pipelines: ResourceTracker::new(),
            descriptor_set_layouts: ResourceTracker::new(),
            descriptor_pools: ResourceTracker::new(),
        }
    }

    unsafe fn cleanup(&mut self) {
        self.buffers.destroy(|b| b.destroy());
        self.images.destroy(|i| i.destroy());
        self.samplers.destroy(|s| s.destroy());
        self.graphics_pipelines.destroy(|p| p.destroy());
        self.descriptor_set_layouts.destroy(|l| l.destroy());
        self.descriptor_pools.destroy(|p| p.destroy());
    }
}

impl Drop for ResourceHub {
    fn drop(&mut self) {
        unsafe { self.cleanup() }
    }
}

#[derive(Clone)]
pub struct HubGuard {
    hub: Arc<RwLock<ResourceHub>>,
}

impl HubGuard {
    pub fn new() -> Self {
        Self {
            hub: Arc::new(RwLock::new(ResourceHub::new())),
        }
    }
}

#[derive(Clone)]
pub struct DeviceGuard {
    device: Arc<Device>,
}

impl DeviceGuard {
    pub fn new(device: Device) -> Self {
        Self {
            device: Arc::new(device),
        }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }
}

impl Deref for DeviceGuard {
    type Target = Device;
    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

pub struct Factory {
    device: DeviceGuard,
    resource_hub: HubGuard,
}

impl Factory {
    pub fn new(device: DeviceGuard, resource_hub: HubGuard) -> Self {
        Self {
            device,
            resource_hub,
        }
    }

    pub fn create_buffer(&self, desc: BufferDesc) -> Result<Escape<Buffer>> {
        let buffer =
            unsafe { Buffer::create(self.device.clone(), self.device.allocator().clone(), desc)? };
        Ok(self.resource_hub.hub.read().buffers.escape(buffer))
    }

    pub fn create_image(&self, desc: ImageDesc) -> Result<Escape<Image>> {
        let image =
            unsafe { Image::create(self.device.clone(), self.device.allocator().clone(), desc)? };
        Ok(self.resource_hub.hub.read().images.escape(image))
    }

    pub fn create_sampler(&self, desc: SamplerDesc) -> Result<Escape<Sampler>> {
        let sampler = unsafe { Sampler::create(self.device.clone(), desc)? };
        Ok(self.resource_hub.hub.read().samplers.escape(sampler))
    }

    pub fn create_graphics_pipeline(
        &self,
        desc: GraphicsPipelineDesc,
    ) -> Result<Escape<GraphicsPipeline>> {
        let graphics_pipeline =
            unsafe { GraphicsPipeline::create(self.device.clone(), self, desc)? };
        Ok(self
            .resource_hub
            .hub
            .read()
            .graphics_pipelines
            .escape(graphics_pipeline))
    }

    pub fn create_descriptor_set_layout(
        &self,
        desc: DescriptorSetLayoutDesc,
    ) -> Result<Escape<DescriptorSetLayout>> {
        let layout = unsafe { DescriptorSetLayout::create(self.device.clone(), desc)? };
        Ok(self
            .resource_hub
            .hub
            .read()
            .descriptor_set_layouts
            .escape(layout))
    }

    pub fn create_descriptor_pool(
        &self,
        desc: DescriptorPoolDesc,
    ) -> Result<Escape<DescriptorPool>> {
        let pool = unsafe { DescriptorPool::create(self.device.clone(), desc)? };
        Ok(self.resource_hub.hub.read().descriptor_pools.escape(pool))
    }

    pub fn cleanup_resources(&self) {
        unsafe {
            self.resource_hub.hub.write().cleanup();
        }
    }

    pub fn hub_guard(&self) -> HubGuard {
        self.resource_hub.clone()
    }
}
