use std::sync::{Arc, Mutex, Weak};

use anyhow::{Context, Result};
use gpu_allocator::{
    vulkan::{Allocator, AllocatorCreateDesc},
    AllocatorDebugSettings,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use rikka_core::{ash, vk};

use crate::{
    barriers::*,
    buffer::*,
    command_buffer::*,
    constants::{self, INVALID_BINDLESS_TEXTURE_INDEX},
    descriptor_set::*,
    device::Device,
    frame::*,
    image::ImageDesc,
    image::*,
    instance::Instance,
    physical_device::PhysicalDevice,
    pipeline::*,
    queue::{Queue, QueueFamily, QueueFamilyIndices},
    sampler::*,
    shader_state::*,
    surface::Surface,
    swapchain::{Swapchain, SwapchainDesc},
    synchronization::{Semaphore, SemaphoreType},
    types::ImageResourceUpdate,
};

pub struct Gpu {
    // XXX: Have an asynchronous transfer handler
    transfer_command_pool: CommandPool,

    // XXX: Use escape/terminals for this?
    global_descriptor_pool: Arc<DescriptorPool>,

    bindless_images_to_update: Vec<ImageResourceUpdate>,

    // XXX: Handle image destruction for bindless images
    // bindless_image_returned_indices: Vec<u32>,
    bindless_image_new_index: u32,

    bindless_descriptor_set: Arc<DescriptorSet>,
    bindless_descriptor_set_layout: Arc<DescriptorSetLayout>,
    bindless_descriptor_pool: Arc<DescriptorPool>,

    default_sampler: Arc<Sampler>,

    allocator: Arc<Mutex<Allocator>>,

    swapchain: Swapchain,

    queued_command_buffers: Vec<Arc<CommandBuffer>>,

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
                num_threads: 3,
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
                // Only 1 set for all bindless images?
                // .set_max_sets(1)
                .set_max_sets(constants::MAX_NUM_BINDLESS_RESOURCECS * 2)
                .add_pool_size(
                    vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                    constants::MAX_NUM_BINDLESS_RESOURCECS,
                )
                .add_pool_size(
                    vk::DescriptorType::STORAGE_IMAGE,
                    constants::MAX_NUM_BINDLESS_RESOURCECS,
                ),
        )?;
        let bindless_descriptor_pool = Arc::new(bindless_descriptor_pool);

        let bindless_descriptor_set_layout_desc = DescriptorSetLayoutDesc::new()
            .set_flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
            .set_bindless(true)
            .add_binding(DescriptorBinding::new(
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                constants::BINDLESS_SET_SAMPLED_IMAGE_INDEX,
                constants::MAX_NUM_BINDLESS_RESOURCECS,
                vk::ShaderStageFlags::FRAGMENT,
            ))
            .add_binding(DescriptorBinding::new(
                vk::DescriptorType::STORAGE_IMAGE,
                constants::BINDLESS_SET_STORAGE_IMAGE_INDEX,
                constants::MAX_NUM_BINDLESS_RESOURCECS,
                vk::ShaderStageFlags::FRAGMENT,
            ));

        let bindless_descriptor_set_layout =
            DescriptorSetLayout::new(device.clone(), bindless_descriptor_set_layout_desc)?;
        let bindless_descriptor_set_layout = Arc::new(bindless_descriptor_set_layout);

        let bindless_descriptor_set = DescriptorSet::new(
            device.clone(),
            DescriptorSetDesc::new(bindless_descriptor_set_layout.clone())
                .set_pool(bindless_descriptor_pool.clone()),
        )?;
        let bindless_descriptor_set = Arc::new(bindless_descriptor_set);

        let default_sampler = Arc::new(Sampler::new(device.clone(), SamplerDesc::new())?);

        // XXX: Actually use transfer command queue for this, currently use graphics since need different queues for resource state transitions
        let transfer_command_pool =
            CommandPool::new(device.clone(), graphics_queue.family_index())?;

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

            queued_command_buffers: Vec::new(),
            command_buffer_manager,
            frame_thread_pools_manager,
            frame_synchronization_manager,

            global_descriptor_pool: Arc::new(global_descriptor_pool),

            bindless_descriptor_pool,
            bindless_descriptor_set_layout,
            bindless_descriptor_set,

            bindless_images_to_update: Vec::new(),

            transfer_command_pool,

            default_sampler,

            bindless_image_new_index: 0,
        })
    }

    pub fn create_buffer(&self, desc: BufferDesc) -> Result<Buffer> {
        Buffer::new(self.device.clone(), self.allocator.clone(), desc)
    }

    pub fn create_image(&mut self, desc: ImageDesc) -> Result<Image> {
        let mut image = Image::new(self.device.clone(), self.allocator.clone(), desc)?;

        image.set_bindless_index(self.bindless_image_new_index);
        self.bindless_image_new_index += 1;

        Ok(image)
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

    pub fn submit_graphics_command_buffer(&self, command_buffer: &CommandBuffer) -> Result<()> {
        self.frame_synchronization_manager
            .submit_graphics_command_buffers(&[command_buffer], &self.graphics_queue)?;

        Ok(())
    }

    pub fn queue_graphics_command_buffer(&mut self, command_buffer: Arc<CommandBuffer>) {
        self.queued_command_buffers.push(command_buffer);
    }

    pub fn submit_queued_graphics_command_buffers(&mut self) -> Result<()> {
        let command_buffers = self
            .queued_command_buffers
            .iter()
            .map(|command_buffer| command_buffer.as_ref())
            .collect::<Vec<_>>();
        self.frame_synchronization_manager
            .submit_graphics_command_buffers(&command_buffers, &self.graphics_queue)?;
        self.queued_command_buffers.clear();
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
        self.swapchain = self
            .swapchain
            .recreate(
                &self.instance,
                &self.surface,
                &self.physical_device,
                &self.device,
            )
            .with_context(|| format!("recreate_swapchain: Failed to create new swapchain!"))?;

        log::info!(
            "Swapchain recreated with extent: {:?}",
            self.swapchain().extent()
        );

        Ok(())
    }

    pub fn set_present_mode(&mut self, present_mode: vk::PresentModeKHR) -> Result<()> {
        self.swapchain = self.swapchain.recreate_present_mode(
            &self.instance,
            &self.surface,
            &self.physical_device,
            &self.device,
            present_mode,
        )?;
        Ok(())
    }

    pub fn swapchain_extent(&self) -> vk::Extent2D {
        self.swapchain.extent()
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

        self.update_bindless_images();

        // Destroy deletion queue resources?

        Ok(present_result)
    }

    pub fn current_frame_index(&self) -> u64 {
        self.frame_synchronization_manager.current_frame_index()
    }

    pub fn current_command_buffer(&mut self, thread_index: u32) -> Result<Arc<CommandBuffer>> {
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

    // XXX: Remove these, ideally handled somewhere else
    pub fn set_swapchain_as_render_target(&mut self) -> Result<()> {
        // let mut command_buffer = self.current_command_buffer(0)?.upgrade().unwrap();

        // command_buffer.begin();

        // let mut barriers = Barriers::new();
        // barriers.add_image(
        //     self.swapchain.current_image_handle().as_ref(),
        //     ResourceState::RENDER_TARGET,
        //     ResourceState::PRESENT,
        // );
        // command_buffer.pipeline_barrier(barriers);

        Ok(())
    }

    pub fn copy_data_to_image<T: Copy>(
        // For command buffer manager mut access
        &mut self,
        image: Arc<Image>,
        staging_buffer: &Buffer,
        data: &[T],
    ) -> Result<()> {
        let command_buffer = self
            .transfer_command_pool
            .allocate_command_buffer(vk::CommandBufferLevel::PRIMARY)?;
        let command_buffer = CommandBuffer::new(
            self.device.clone(),
            command_buffer,
            // XXX: Implement trait default for this
            CommandBufferMetaData {
                array_index: 0,
                frame_index: 0,
                thread_index: 0,
            },
            false,
        );

        command_buffer.upload_data_to_image(image.as_ref(), staging_buffer, data)?;
        self.graphics_queue
            .submit(&[&command_buffer], Vec::new(), Vec::new())?;

        self.wait_idle();

        let update = ImageResourceUpdate {
            frame: self.frame_synchronization_manager.current_frame_index(),
            image: Some(image),
            sampler: None,
        };
        self.bindless_images_to_update.push(update);

        Ok(())
    }

    pub fn transition_image_layout(
        &self,
        image: &Image,
        old_state: ResourceState,
        new_state: ResourceState,
    ) -> Result<()> {
        let command_buffer = self
            .transfer_command_pool
            .allocate_command_buffer(vk::CommandBufferLevel::PRIMARY)?;
        let command_buffer = CommandBuffer::new(
            self.device.clone(),
            command_buffer,
            // XXX: Implement trait default for this
            CommandBufferMetaData {
                array_index: 0,
                frame_index: 0,
                thread_index: 0,
            },
            false,
        );

        let barriers = Barriers::new().add_image(image, old_state, new_state);

        command_buffer.begin()?;
        command_buffer.pipeline_barrier(barriers);
        command_buffer.end()?;

        self.graphics_queue
            .submit(&[&command_buffer], vec![], vec![])?;
        self.wait_idle();

        Ok(())
    }

    // XXX: Properly integrate this somewhere internally
    pub fn bindless_descriptor_set_layout(&self) -> &Arc<DescriptorSetLayout> {
        &self.bindless_descriptor_set_layout
    }

    pub fn bindless_descriptor_set(&self) -> &Arc<DescriptorSet> {
        &self.bindless_descriptor_set
    }

    fn add_bindless_image_update(&mut self, update: ImageResourceUpdate) {
        self.bindless_images_to_update.push(update);
    }

    pub fn update_bindless_images(&mut self) {
        // let mut write_descriptors = Vec::new();

        // Need this here to store image descriptors
        // XXX: This is dangerous!
        let mut image_descriptors = Vec::new();

        for update in self.bindless_images_to_update.drain(..) {
            if let Some(image) = update.image {
                assert!(image.bindless_index() != INVALID_BINDLESS_TEXTURE_INDEX);

                let mut image_descriptor = vk::DescriptorImageInfo::builder()
                    .image_view(image.raw_view())
                    .sampler(self.default_sampler.raw())
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                if let Some(sampler) = update.sampler {
                    image_descriptor = image_descriptor.sampler(sampler.raw());
                } else {
                    image_descriptor = image_descriptor.sampler(self.default_sampler.raw());
                }
                image_descriptors.push(image_descriptor);

                let write_descriptor = vk::WriteDescriptorSet::builder()
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER) //
                    .dst_array_element(image.bindless_index())
                    .dst_set(self.bindless_descriptor_set.raw())
                    .dst_binding(constants::BINDLESS_SET_SAMPLED_IMAGE_INDEX)
                    // XXX: This is dangerous, change this!
                    .image_info(std::slice::from_ref(image_descriptors.last().unwrap()));
                // write_descriptors.push(write_descriptor.build());

                unsafe {
                    self.device
                        .raw()
                        .update_descriptor_sets(std::slice::from_ref(&write_descriptor), &[]);
                }
            }
        }

        // if !write_descriptors.is_empty() {
        //     unsafe {
        //         self.device
        //             .raw()
        //             .update_descriptor_sets(&write_descriptors, &[]);
        //     }
        // }
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        unsafe {
            self.device
                .raw()
                .queue_wait_idle(self.graphics_queue.raw())
                .unwrap();
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
