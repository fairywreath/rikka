use std::sync::{Arc, Mutex};

use anyhow::Result;
use ash::vk;
use gpu_allocator::{
    vulkan::{Allocator, AllocatorCreateDesc},
    AllocatorDebugSettings,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use crate::{
    device::Device,
    frame::{ThreadFramePools, ThreadFramePoolsManager},
    physical_device::PhysicalDevice,
    queue::{Queue, QueueFamily, QueueFamilyIndices},
    surface::Surface,
    swapchain::{Swapchain, SwapchainDesc},
    synchronization::{Semaphore, SemaphoreType},
    *,
};

pub struct RHIContext {
    swapchain: Swapchain,

    allocator: Arc<Mutex<Allocator>>,

    graphics_queue: Queue,
    present_queue: Queue,
    compute_queue: Queue,
    transfer_queue: Queue,

    thread_frame_pools_manager: ThreadFramePoolsManager,

    device: Arc<Device>,
    queue_families: QueueFamilyIndices,

    surface: Surface,
    instance: Instance,

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

impl RHIContext {
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

        let queue_families_array = [
            queue_families.graphics,
            queue_families.compute,
            queue_families.transfer,
            queue_families.compute,
        ];

        let device = Arc::new(Device::new(
            &instance,
            &physical_device,
            &queue_families_array,
        )?);

        let graphics_queue = device.get_queue(queue_families.graphics, 0);
        let present_queue = device.get_queue(queue_families.present, 0);
        let compute_queue = device.get_queue(queue_families.compute, 0);
        let transfer_queue = device.get_queue(queue_families.transfer, 0);

        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.raw_clone(),
            device: device.raw_clone(),
            physical_device: physical_device.raw_clone(),
            debug_settings: AllocatorDebugSettings {
                log_memory_information: true,
                log_leaks_on_shutdown: true,
                ..Default::default()
            },
            buffer_device_address: true,
        })?;
        let allocator = Arc::new(Mutex::new(allocator));

        let swapchain = Swapchain::new(
            &instance,
            &surface,
            &physical_device,
            &device,
            SwapchainDesc::new(
                u32::MAX,
                u32::MAX,
                queue_families.graphics.index(),
                queue_families.present.index(),
            ),
        )?;

        let thread_frame_pools_manager = ThreadFramePoolsManager::new(
            device.clone(),
            frame::ThreadFramePoolsDesc {
                num_threads: 1,
                time_queries_per_frame: 32,
                graphics_queue_family_index: graphics_queue.family_index(),
            },
        )?;

        Ok(Self {
            surface,
            instance,
            entry,
            queue_families,
            device,

            graphics_queue,
            present_queue,
            compute_queue,
            transfer_queue,

            allocator,
            swapchain,

            thread_frame_pools_manager,
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

    pub(crate) fn device(&self) -> &Arc<Device> {
        &self.device
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
