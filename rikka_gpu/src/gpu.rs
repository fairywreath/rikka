use std::sync::{Arc, Mutex, Weak};

use anyhow::{Context, Result};
use ash::vk;
use gpu_allocator::{
    vulkan::{Allocator, AllocatorCreateDesc},
    AllocatorDebugSettings,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use crate::{
    buffer::*,
    command_buffer::*,
    constants,
    descriptor_set::*,
    device::Device,
    frame::*,
    instance::Instance,
    physical_device::PhysicalDevice,
    pipeline::*,
    queue::{Queue, QueueFamily, QueueFamilyIndices},
    sampler::*,
    shader_state::*,
    surface::Surface,
    swapchain::{Swapchain, SwapchainDesc},
    synchronization::{Semaphore, SemaphoreType},
};

pub struct Gpu {
    // XXX: Use escape/terminals for this?
    global_descriptor_pool: Arc<DescriptorPool>,
    bindless_descriptor_pool: Arc<DescriptorPool>,

    allocator: Arc<Mutex<Allocator>>,

    swapchain: Swapchain,

    command_buffer_manager: CommandBufferManager,
    frame_thread_pools_manager: FrameThreadPoolsManager,
    frame_synchronization_manager: FrameSynchronizationManager,

    physical_device: PhysicalDevice,
    device: Arc<Device>,

    queue_families: QueueFamilyIndices,
    graphics_queue: Queue,
    present_queue: Queue,
    compute_queue: Queue,
    transfer_queue: Queue,

    surface: Surface,
    instance: Instance,

    entry: ash::Entry,
}

pub struct GpuDesc<'a> {
    window_handle: &'a dyn HasRawWindowHandle,
    display_handle: &'a dyn HasRawDisplayHandle,
}

impl<'a> GpuDesc<'a> {
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

impl Gpu {
    pub fn new(desc: GpuDesc) -> Result<Self> {
        let entry = unsafe { ash::Entry::load()? };
        let mut instance = Instance::new(&entry, &desc.display_handle)?;
        let surface = Surface::new(&entry, &instance, &desc.window_handle, &desc.display_handle)?;

        let physical_devices = instance.get_physical_devices(&surface)?;
        let physical_device = select_suitable_physical_device(&physical_devices)?;

        log::info!("GPU name: {}", physical_device.name);

        let queue_families = select_queue_family_indices(&physical_device);

        log::info!("Graphics family: {}", queue_families.graphics.index());
        log::info!("Present family: {}", queue_families.present.index());
        log::info!("Compute family: {}", queue_families.compute.index());
        log::info!("Transfer family: {}", queue_families.transfer.index());

        let device = Arc::new(Device::new(
            &instance,
            &physical_device,
            &[
                queue_families.graphics,
                queue_families.compute,
                queue_families.transfer,
                queue_families.compute,
            ],
        )?);

        let graphics_queue = device.get_queue(queue_families.graphics, 0);
        let present_queue = device.get_queue(queue_families.present, 0);
        let compute_queue = device.get_queue(queue_families.compute, 0);
        let transfer_queue = device.get_queue(queue_families.transfer, 0);

        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.raw().clone(),
            device: device.raw().clone(),
            physical_device: physical_device.raw(),
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

        let frame_thread_pools_manager = FrameThreadPoolsManager::new(
            device.clone(),
            FrameThreadPoolsDesc {
                num_threads: 1,
                num_frames: constants::MAX_FRAMES,
                time_queries_per_frame: 32,
                graphics_queue_family_index: graphics_queue.family_index(),
            },
        )?;

        let command_buffer_manager =
            CommandBufferManager::new(device.clone(), &frame_thread_pools_manager)?;

        let frame_synchronization_manager = FrameSynchronizationManager::new(device.clone())?;

        let global_descriptor_pool = DescriptorPool::new(
            device.clone(),
            DescriptorPoolDesc::new()
                .set_max_sets(constants::GLOBAL_DESCRIPTOR_POOL_MAX_SETS)
                .add_pool_size(
                    vk::DescriptorType::SAMPLER,
                    constants::GLOBAL_DESCRIPTOR_POOL_ELEMENT_SIZE,
                )
                .add_pool_size(
                    vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                    constants::GLOBAL_DESCRIPTOR_POOL_ELEMENT_SIZE,
                )
                .add_pool_size(
                    vk::DescriptorType::SAMPLED_IMAGE,
                    constants::GLOBAL_DESCRIPTOR_POOL_ELEMENT_SIZE,
                )
                .add_pool_size(
                    vk::DescriptorType::STORAGE_IMAGE,
                    constants::GLOBAL_DESCRIPTOR_POOL_ELEMENT_SIZE,
                )
                .add_pool_size(
                    vk::DescriptorType::UNIFORM_BUFFER,
                    constants::GLOBAL_DESCRIPTOR_POOL_ELEMENT_SIZE,
                )
                .add_pool_size(
                    vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                    constants::GLOBAL_DESCRIPTOR_POOL_ELEMENT_SIZE,
                )
                .add_pool_size(
                    vk::DescriptorType::UNIFORM_TEXEL_BUFFER,
                    constants::GLOBAL_DESCRIPTOR_POOL_ELEMENT_SIZE,
                )
                .add_pool_size(
                    vk::DescriptorType::STORAGE_BUFFER,
                    constants::GLOBAL_DESCRIPTOR_POOL_ELEMENT_SIZE,
                )
                .add_pool_size(
                    vk::DescriptorType::STORAGE_BUFFER_DYNAMIC,
                    constants::GLOBAL_DESCRIPTOR_POOL_ELEMENT_SIZE,
                )
                .add_pool_size(
                    vk::DescriptorType::STORAGE_TEXEL_BUFFER,
                    constants::GLOBAL_DESCRIPTOR_POOL_ELEMENT_SIZE,
                )
                .add_pool_size(
                    vk::DescriptorType::INPUT_ATTACHMENT,
                    constants::GLOBAL_DESCRIPTOR_POOL_ELEMENT_SIZE,
                ),
        )?;

        let bindless_descriptor_pool = DescriptorPool::new(
            device.clone(),
            DescriptorPoolDesc::new()
                .set_flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND)
                // Only 1 set for all bindless images.
                .set_max_sets(1)
                .add_pool_size(
                    vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                    constants::MAX_NUM_BINDLESS_RESOURCECS,
                )
                .add_pool_size(
                    vk::DescriptorType::SAMPLER,
                    constants::MAX_NUM_BINDLESS_RESOURCECS,
                ),
        )?;

        Ok(Self {
            surface,
            instance,
            entry,
            queue_families,
            device,
            physical_device,

            graphics_queue,
            present_queue,
            compute_queue,
            transfer_queue,

            allocator,
            swapchain,

            command_buffer_manager,
            frame_thread_pools_manager,
            frame_synchronization_manager,

            global_descriptor_pool: Arc::new(global_descriptor_pool),
            bindless_descriptor_pool: Arc::new(bindless_descriptor_pool),
        })
    }

    pub fn create_buffer(&self, desc: BufferDesc) -> Result<Buffer> {
        Buffer::new(self.device.clone(), self.allocator.clone(), desc)
    }

    pub fn create_sampler(&self, desc: SamplerDesc) -> Result<Sampler> {
        Sampler::new(self.device.clone(), desc)
    }

    pub fn create_shader_state(&self, desc: ShaderStateDesc) -> Result<ShaderState> {
        ShaderState::new(self.device.clone(), desc)
    }

    pub fn create_graphics_pipeline(&self, desc: GraphicsPipelineDesc) -> Result<GraphicsPipeline> {
        GraphicsPipeline::new(self.device.clone(), desc)
    }

    pub fn create_descriptor_set_layout(
        &self,
        desc: DescriptorSetLayoutDesc,
    ) -> Result<DescriptorSetLayout> {
        DescriptorSetLayout::new(self.device.clone(), desc)
    }

    pub fn create_descriptor_set(&self, desc: DescriptorSetDesc) -> Result<DescriptorSet> {
        // XXX: Always use internal global descriptor pool for now
        let desc = desc.set_pool(self.global_descriptor_pool.clone());
        DescriptorSet::new(self.device.clone(), desc)
    }

    pub fn new_frame(&mut self) -> Result<()> {
        self.frame_synchronization_manager
            .wait_graphics_compute_semaphores()?;

        self.command_buffer_manager.reset_pools(
            &self.frame_thread_pools_manager,
            self.frame_synchronization_manager.current_frame_index() as u32,
        )?;

        // XXX: Update descriptor sets.

        // XXX: Reset queries.

        Ok(())
    }

    pub fn submit_graphics_command_buffer(
        &self,
        command_buffer: Weak<CommandBuffer>,
    ) -> Result<()> {
        let command_buffer = command_buffer.upgrade().unwrap();
        let command_buffers = vec![command_buffer.as_ref()];

        self.frame_synchronization_manager
            .submit_graphics_command_buffers(&command_buffers, &self.graphics_queue)?;

        Ok(())
    }

    // XXX: Do not expose this? queue command buffer and call this during present before submitting queued command buffers.
    pub fn swapchain_acquire_next_image(&mut self) -> Result<bool> {
        // XXX: Handle this in FrameSynchronizationManager?
        let acquire_result = self.swapchain.acquire_next_image(
            self.frame_synchronization_manager
                .swapchain_image_acquired_semaphore(),
        )?;

        Ok(acquire_result)
    }

    pub fn recreate_swapchain(&mut self) -> Result<()> {
        self.swapchain.destroy();
        self.swapchain = Swapchain::new(
            &self.instance,
            &self.surface,
            &self.physical_device,
            &self.device,
            SwapchainDesc::new(u32::MAX, u32::MAX, 0, 0),
        )
        .with_context(|| format!("recreate_swapchain: Failed to create new swapchain!"))?;

        Ok(())
    }

    pub fn present(&mut self) -> Result<bool> {
        let wait_semaphores = [self
            .frame_synchronization_manager
            .current_render_complete_semaphore()];

        let present_result = self
            .swapchain
            .queue_present(&wait_semaphores, &self.graphics_queue)
            .with_context(|| (format!("Failed swapchain presentation!")))?;

        // XXX: Properly handle failed presentation case.
        // assert!(present_result);

        self.frame_synchronization_manager.advance_frame_counters();

        // XXX:
        // Update bindless textures?
        // Destroy deletion queue resources?

        Ok(present_result)
    }

    pub fn current_frame_index(&self) -> u64 {
        self.frame_synchronization_manager.current_frame_index()
    }

    pub fn current_command_buffer(&mut self, thread_index: u32) -> Result<Weak<CommandBuffer>> {
        let command_buffer = self.command_buffer_manager.command_buffer(
            self.frame_synchronization_manager.current_frame_index() as u32,
            thread_index,
        )?;

        Ok(command_buffer)
    }

    // XXX: Remove this
    pub fn swapchain(&self) -> &Swapchain {
        &self.swapchain
    }

    pub fn advance_frame_counters(&mut self) {
        self.frame_synchronization_manager.advance_frame_counters();
    }

    pub fn wait_idle(&self) {
        unsafe {
            self.device
                .raw()
                .queue_wait_idle(self.graphics_queue.raw())
                .unwrap();
        };
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        unsafe {
            self.device
                .raw()
                .queue_wait_idle(self.graphics_queue.raw())
                .unwrap()
        }

        log::info!("GPU dropped");
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
