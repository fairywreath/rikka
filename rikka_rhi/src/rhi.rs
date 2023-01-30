use std::sync::{Arc, Mutex, Weak};

use anyhow::Result;
use ash::vk;
use gpu_allocator::{
    vulkan::{Allocator, AllocatorCreateDesc},
    AllocatorDebugSettings,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use crate::{
    command_buffer::*,
    device::Device,
    frame::*,
    frame::{FrameThreadPools, FrameThreadPoolsManager},
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

    command_buffer_manager: CommandBufferManager,
    frame_thread_pools_manager: FrameThreadPoolsManager,
    frame_synchronization_manager: FrameSynchronizationManager,

    physical_device: PhysicalDevice,
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

        let frame_thread_pools_manager = FrameThreadPoolsManager::new(
            device.clone(),
            frame::FrameThreadPoolsDesc {
                num_threads: 3,
                time_queries_per_frame: 32,
                graphics_queue_family_index: graphics_queue.family_index(),
            },
        )?;

        let command_buffer_manager =
            CommandBufferManager::new(device.clone(), &frame_thread_pools_manager)?;

        let frame_synchronization_manager = FrameSynchronizationManager::new(device.clone())?;

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
        })
    }

    // pub fn create_buffer(&self, desc: BufferDesc) -> Result<Buffer, BufferCreationError> {
    //     todo!()
    // }

    // pub fn create_texture(&self, desc: TextureDesc) -> Result<Texture, TextureCreationError> {
    //     todo!()
    // }

    // pub fn create_sampler(&self, desc: SamplerDesc) -> Result<Sampler, SamplerCreationError> {
    //     todo!()
    // }

    // pub fn create_shader_state(
    //     &self,
    //     desc: ShaderStateDesc,
    // ) -> Result<ShaderState, ShaderStateCreationError> {
    //     todo!()
    // }

    // pub fn create_descriptor_set(
    //     &self,
    //     desc: DescriptorSetDesc,
    // ) -> Result<DescriptorSetDesc, DescriptorSetCreationError> {
    //     todo!()
    // }

    // pub fn create_graphics_pipeline(
    //     &self,
    //     desc: GraphicsPipelineDesc,
    // ) -> Result<GraphicsPipeline, GraphicsPipelineCreationError> {
    //     todo!()
    // }

    pub fn new_frame(&mut self) -> Result<()> {
        log::info!("Waiting for graphics semaphore!");
        self.frame_synchronization_manager
            .wait_graphics_compute_semaphores()?;

        // XXX: Update descriptor sets.

        // XXX: Reset queries.

        Ok(())
    }

    pub fn submit_graphics_command_buffer(
        &self,
        command_buffer: Weak<CommandBuffer>,
    ) -> Result<()> {
        let command_buffers = vec![command_buffer];
        self.frame_synchronization_manager
            .submit_graphics_command_buffers(&command_buffers, &self.graphics_queue)?;

        Ok(())
    }

    // pub fn submit_current_graphics_command_buffer(&mut self) -> Result<()> {
    //     let command_buffer = self.command_buffer_manager.command_buffer(
    //         self.frame_synchronization_manager.current_frame_index() as u32,
    //         0,
    //     )?;

    //     self.submit_graphics_command_buffer(command_buffer);

    //     Ok(())
    // }

    // XXX: Do not expose this? queue command buffer and call this during present before submitting queued command buffers.
    pub fn swapchain_acquire_next_image(&mut self) -> Result<bool> {
        // XXX: Handle this in FrameSynchronizationManager?
        let acquire_result = self.swapchain.acquire_next_image(
            self.frame_synchronization_manager
                .swapchain_image_acquired_semaphore(),
        )?;

        // XXX: Properly handle swapchain re-creation.
        // assert!(acquire_result);

        log::debug!("Swapchain acquire image result is {}", acquire_result);

        // if !acquire_result {
        //     self.recreate_swapchain()?;

        //     let acquire_result = self.swapchain.acquire_next_image(
        //         self.frame_synchronization_manager
        //             .swapchain_image_acquired_semaphore(),
        //     )?;

        //     log::info!("Acquire result 2 is {}", acquire_result);
        // }

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
        )?;

        // Wait on image acquired semaphore.
        let semaphores = [self
            .frame_synchronization_manager
            .swapchain_image_acquired_semaphore()
            .raw()];
        // let wait_info = vk::SemaphoreWaitInfo::builder().semaphores(&semaphores);
        // unsafe { self.device.raw().wait_semaphores(&wait_info, u64::MAX)? };

        // unsafe {
        //     self.device
        //         .raw()
        //         .queue_wait_idle(self.graphics_queue.raw_clone())?;

        //     self.device.rese
        // }

        //         const VkPipelineStageFlags psw = VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT;
        // VkSubmitInfo submit_info = {};
        // submit_info.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO;
        // submit_info.waitSemaphoreCount = 1;
        // submit_info.pWaitSemaphores = &semaphore;
        // submit_info.pWaitDstStageMask;

        // vkQueueSubmit( queue, 1, &submit_info, VK_NULL_HANDLE );

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&semaphores)
            .wait_dst_stage_mask(&[vk::PipelineStageFlags::BOTTOM_OF_PIPE])
            .build();

        unsafe {
            self.device.raw().queue_submit(
                self.graphics_queue.raw_clone(),
                &[submit_info],
                vk::Fence::null(),
            )?
        }

        Ok(())
    }

    pub fn present(&mut self) -> Result<(bool)> {
        let wait_semaphores = [self
            .frame_synchronization_manager
            .current_render_complete_semaphore()];

        let present_result = self
            .swapchain
            .queue_present(&wait_semaphores, &self.graphics_queue)?;

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

    // XXX: Remove these.
    pub fn swapchain(&self) -> &Swapchain {
        &self.swapchain
    }

    pub fn advance_frame_counters(&mut self) {
        self.frame_synchronization_manager.advance_frame_counters();
    }
}

impl Drop for RHIContext {
    fn drop(&mut self) {
        unsafe {
            self.device
                .raw()
                .queue_wait_idle(self.graphics_queue.raw_clone())
                .unwrap()
        }
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
