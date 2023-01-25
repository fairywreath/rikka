use std::{ffi::CString, sync::Arc};

use anyhow::Result;
use ash::vk;

use crate::{
    deletion_queue::{self, DeferredDeletionQueue},
    instance::Instance,
    physical_device::{self, PhysicalDevice},
    queue::{Queue, QueueFamily},
};

pub struct Device {
    raw: ash::Device,
    deletion_queue: DeferredDeletionQueue,
}

impl Device {
    pub fn new(
        instance: &Instance,
        physical_device: &PhysicalDevice,
        queue_families: &[QueueFamily],
    ) -> Result<Self> {
        let queue_priorities = [1.0f32];

        let queue_create_infos = {
            let mut indices = queue_families
                .iter()
                .map(|family| family.index())
                .collect::<Vec<_>>();

            indices.sort();
            indices.dedup();

            println!("Deduped indices: {:?}", indices);

            indices
                .iter()
                .map(|index| {
                    vk::DeviceQueueCreateInfo::builder()
                        .queue_family_index(*index)
                        .queue_priorities(&queue_priorities)
                        .build()
                })
                .collect::<Vec<_>>()
        };

        let device_extension_strs = ["VK_KHR_swapchain", "VK_NV_mesh_shader"];
        let device_extension_strs = device_extension_strs
            .iter()
            .map(|str| CString::new(*str))
            .collect::<Result<Vec<_>, _>>()?;
        let device_extension_strs = device_extension_strs
            .iter()
            .map(|ext| ext.as_ptr())
            .collect::<Vec<_>>();

        let mut vulkan11_features =
            vk::PhysicalDeviceVulkan11Features::builder().shader_draw_parameters(true);
        let mut vulkan12_features = vk::PhysicalDeviceVulkan12Features::builder()
            .descriptor_indexing(true)
            .runtime_descriptor_array(true)
            .descriptor_binding_partially_bound(true)
            .descriptor_binding_variable_descriptor_count(true)
            .timeline_semaphore(true)
            .shader_sampled_image_array_non_uniform_indexing(true)
            .buffer_device_address(true);
        let mut vulkan13_features = vk::PhysicalDeviceVulkan13Features::builder()
            .dynamic_rendering(true)
            .synchronization2(true);

        let mut mesh_shader_features = vk::PhysicalDeviceMeshShaderFeaturesNV::builder()
            .mesh_shader(true)
            .task_shader(true);

        // PhysicalDeviceFeatures 2 reports ALL of GPU's device features capabilies. Pass this along pNext chain to enable all.
        let mut device_features2 = vk::PhysicalDeviceFeatures2::builder();
        unsafe {
            instance
                .raw()
                .get_physical_device_features2(physical_device.raw_clone(), &mut device_features2);
        }

        // Set pNext chain.
        device_features2 = device_features2
            .push_next(&mut vulkan11_features)
            .push_next(&mut vulkan12_features)
            .push_next(&mut vulkan13_features)
            .push_next(&mut mesh_shader_features);

        let device_create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_extension_strs)
            .push_next(&mut device_features2);

        // Create vulkan logical device.
        let device = unsafe {
            instance
                .raw()
                .create_device(physical_device.raw_clone(), &device_create_info, None)?
        };

        // Create deletion queue.
        let deletion_queue = DeferredDeletionQueue {};

        Ok(Self {
            raw: device,
            deletion_queue,
        })
    }

    pub(crate) fn get_deletion_queue(&mut self) -> &mut DeferredDeletionQueue {
        &mut self.deletion_queue
    }

    pub(crate) fn raw(&self) -> &ash::Device {
        &self.raw
    }

    pub(crate) fn raw_clone(&self) -> ash::Device {
        self.raw.clone()
    }

    pub(crate) fn get_queue(
        self: &Arc<Self>,
        queue_family: QueueFamily,
        queue_index: u32,
    ) -> Queue {
        let raw = unsafe { self.raw.get_device_queue(queue_family.index(), queue_index) };
        Queue::new(self.clone(), raw)
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            self.raw.destroy_device(None);
        }
    }
}
