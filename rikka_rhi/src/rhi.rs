use std::sync::{Arc, Mutex};

use anyhow::Result;
use ash::{vk, Entry};
use gpu_allocator::{
    vulkan::{Allocator, AllocatorCreateDesc},
    AllocatorDebugSettings,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use crate::*;
use crate::{
    physical_device::PhysicalDevice,
    queue::{QueueFamily, QueueFamilyIndices},
    surface::Surface,
};

pub struct RHI {
    surface: Surface,
    instance: Instance,

    queue_families: QueueFamilyIndices,

    entry: ash::Entry,
}

pub struct RHICreationDesc<'a> {
    window_handle: &'a dyn HasRawWindowHandle,
    display_handle: &'a dyn HasRawDisplayHandle,
}

impl<'a> RHICreationDesc<'a> {
    pub fn new(
        window_handle: &'a dyn HasRawWindowHandle,
        display_handle: &'a dyn HasRawDisplayHandle,
    ) -> Self {
        Self {
            window_handle,
            display_handle,
        }
    }
}

impl RHI {
    pub fn new(desc: RHICreationDesc) -> Result<Self> {
        let entry = unsafe { ash::Entry::load()? };
        let mut instance = Instance::new(&entry, &desc.display_handle)?;
        let surface = Surface::new(&entry, &instance, &desc.window_handle, &desc.display_handle)?;

        let physical_devices = instance.get_physical_devices(&surface)?;
        let physical_device = select_suitable_physical_device(&physical_devices)?;

        println!("GPU name: {}", physical_device.name);

        let queue_families = select_queue_family_indices(&physical_device);

        println!("Graphics family: {}", queue_families.graphics.index());
        println!("Present family: {}", queue_families.present.index());
        println!("Compute family: {}", queue_families.compute.index());
        println!("Transfer family: {}", queue_families.transfer.index());

        Ok(Self {
            surface,
            instance,
            entry,
            queue_families,
        })
    }

    pub fn create_buffer(&self, desc: BufferDesc) -> Result<Buffer, BufferCreationError> {
        todo!()
    }

    pub fn create_texture(&self, desc: TextureDesc) -> Result<Texture, TextureCreationError> {
        todo!()
    }

    pub fn create_sampler(&self, desc: SamplerDesc) -> Result<Sampler, SamplerCreationError> {
        todo!()
    }

    pub fn create_shader_state(
        &self,
        desc: ShaderStateDesc,
    ) -> Result<ShaderState, ShaderStateCreationError> {
        todo!()
    }

    pub fn create_descriptor_set(
        &self,
        desc: DescriptorSetDesc,
    ) -> Result<DescriptorSetDesc, DescriptorSetCreationError> {
        todo!()
    }

    pub fn create_graphics_pipeline(
        &self,
        desc: GraphicsPipelineDesc,
    ) -> Result<GraphicsPipeline, GraphicsPipelineCreationError> {
        todo!()
    }
}

fn select_suitable_physical_device(devices: &[PhysicalDevice]) -> Result<PhysicalDevice> {
    // XXX TODO: Check required extensions and queue support

    let device = devices
        .iter()
        .find(|device| device.device_type == vk::PhysicalDeviceType::DISCRETE_GPU)
        .ok_or_else(|| anyhow::anyhow!("Could not find suitable GPU!"))?;

    Ok(device.clone())
}

fn select_queue_family_indices(device: &PhysicalDevice) -> QueueFamilyIndices {
    let mut graphics = None;
    let mut present = None;
    let mut compute = None;
    let mut transfer = None;

    // 1 graphics + present family, 1 compute family and 1 transfer only family
    for family in device
        .queue_families
        .iter()
        .filter(|family| family.queue_count() > 0)
    {
        if family.supports_graphics() && graphics.is_none() {
            graphics = Some(*family);
            assert!(family.supports_present());
            present = Some(*family);
        } else if family.supports_compute() && compute.is_none() {
            compute = Some(*family);
        } else if family.supports_transfer() && !family.supports_compute() && transfer.is_none() {
            transfer = Some(*family);
        }
    }

    QueueFamilyIndices {
        graphics: graphics.unwrap(),
        present: present.unwrap(),
        compute: compute.unwrap(),
        transfer: transfer.unwrap(),
    }
}
