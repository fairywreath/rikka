use std::{ffi::CString, mem::ManuallyDrop, sync::Arc};

use anyhow::Result;
use gpu_allocator::{
    vulkan::{Allocator, AllocatorCreateDesc},
    AllocatorDebugSettings,
};
use parking_lot::Mutex;

use rikka_core::{ash, vk};

use crate::{instance::Instance, physical_device::PhysicalDevice, queue::*, surface::Surface};

/// Device wrapper that acts as a lifeguard for the Gpu resources and the Vulkan instance.
pub struct Device {
    // XXX: Remove Arc<>
    allocator: ManuallyDrop<Arc<Mutex<Allocator>>>,
    queue_family_indices: QueueFamilyIndices,
    raw: ash::Device,
    physical_device: PhysicalDevice,
    surface: Surface,
    instance: Instance,
}

impl Device {
    pub fn new(instance: Instance, surface: Surface) -> Result<Self> {
        let physical_devices = instance.get_physical_devices(&surface)?;
        let physical_device = select_suitable_physical_device(&physical_devices)?;
        let queue_family_indices = select_queue_family_indices(&physical_device);

        log::info!("Gpu name: {}", physical_device.name);
        log::info!("Graphics family: {}", queue_family_indices.graphics.index());
        log::info!("Present family: {}", queue_family_indices.present.index());
        log::info!("Compute family: {}", queue_family_indices.compute.index());
        log::info!("Transfer family: {}", queue_family_indices.transfer.index());

        let raw = Self::new_vulkan_device(
            &instance,
            &physical_device,
            &[
                queue_family_indices.graphics,
                queue_family_indices.compute,
                queue_family_indices.transfer,
                queue_family_indices.compute,
            ],
        )?;

        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.raw().clone(),
            device: raw.clone(),
            physical_device: physical_device.raw(),
            debug_settings: AllocatorDebugSettings {
                log_memory_information: true,
                log_leaks_on_shutdown: true,
                ..Default::default()
            },
            buffer_device_address: true,
        })?;
        let allocator = Arc::new(Mutex::new(allocator));

        Ok(Self {
            allocator: ManuallyDrop::new(allocator),
            queue_family_indices,
            raw,
            physical_device,
            surface,
            instance,
        })
    }

    fn new_vulkan_device(
        instance: &Instance,
        physical_device: &PhysicalDevice,
        queue_family_indices: &[QueueFamily],
    ) -> Result<ash::Device> {
        let queue_priorities = [1.0f32];

        let queue_create_infos = {
            let mut indices = queue_family_indices
                .iter()
                .map(|family| family.index())
                .collect::<Vec<_>>();

            indices.sort();
            indices.dedup();

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

        // XXX: Properly check that these features are supported by the Gpu(done in physical device creation)

        let mut vulkan11_features =
            vk::PhysicalDeviceVulkan11Features::builder().shader_draw_parameters(true);
        let mut vulkan12_features = vk::PhysicalDeviceVulkan12Features::builder()
            .descriptor_indexing(true)
            .runtime_descriptor_array(true)
            .descriptor_binding_partially_bound(true)
            .descriptor_binding_variable_descriptor_count(true)
            .descriptor_binding_sampled_image_update_after_bind(true)
            .descriptor_binding_storage_image_update_after_bind(true)
            .timeline_semaphore(true)
            .shader_sampled_image_array_non_uniform_indexing(true)
            .buffer_device_address(true);
        let mut vulkan13_features = vk::PhysicalDeviceVulkan13Features::builder()
            .dynamic_rendering(true)
            .synchronization2(true);

        let mut mesh_shader_features = vk::PhysicalDeviceMeshShaderFeaturesNV::builder()
            .mesh_shader(true)
            .task_shader(true);

        // PhysicalDeviceFeatures 2 reports ALL of Gpu's device features capabilies. Pass this along pNext chain to enable all.
        let mut device_features2 = vk::PhysicalDeviceFeatures2::builder();
        unsafe {
            instance
                .raw()
                .get_physical_device_features2(physical_device.raw(), &mut device_features2);
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
                .create_device(physical_device.raw(), &device_create_info, None)?
        };

        Ok(device)
    }

    pub fn raw(&self) -> &ash::Device {
        &self.raw
    }

    pub fn queue_family(&self, queue_type: QueueType) -> &QueueFamily {
        match queue_type {
            QueueType::Graphics => &self.queue_family_indices.graphics,
            QueueType::Transfer => &self.queue_family_indices.transfer,
            QueueType::Compute => &self.queue_family_indices.compute,
        }
    }

    pub fn get_queue(&self, queue_type: QueueType, queue_index: u32) -> Queue {
        let queue_family = self.queue_family(queue_type);
        let raw = unsafe { self.raw.get_device_queue(queue_family.index(), queue_index) };
        unsafe { Queue::new(self.raw.clone(), raw, queue_family.index()) }
    }

    pub fn instance(&self) -> &Instance {
        &self.instance
    }

    pub fn physical_device(&self) -> &PhysicalDevice {
        &self.physical_device
    }

    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    pub fn allocator(&self) -> &Arc<Mutex<Allocator>> {
        &self.allocator
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            log::info!("Device dropped");
            // XXX: Queue wait idle here for ALL queues
            // self.allocator.
            ManuallyDrop::drop(&mut self.allocator);
            self.raw.destroy_device(None);
        }
    }
}

fn select_suitable_physical_device(devices: &[PhysicalDevice]) -> Result<PhysicalDevice> {
    let device = devices
        .iter()
        .find(|device| device.device_type == vk::PhysicalDeviceType::DISCRETE_GPU)
        .ok_or_else(|| anyhow::anyhow!("Could not find suitable Gpu!"))?;

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
