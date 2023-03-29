use std::{
    mem::swap,
    sync::{Arc, Mutex, Weak},
};

use anyhow::{anyhow, Result};
use rikka_core::vk::{self, RenderingAttachmentInfo};

use crate::{
    barriers::*,
    buffer::*,
    command_buffer,
    constants::{self, NUM_COMMAND_BUFFERS_PER_THREAD},
    descriptor_set::DescriptorSet,
    device::Device,
    frame::{self, FrameThreadPoolsManager},
    image::*,
    pipeline::*,
    swapchain::Swapchain,
    types::*,
};

pub struct CommandPool {
    raw: vk::CommandPool,
    device: Arc<Device>,
}

impl CommandPool {
    pub fn new(device: Arc<Device>, queue_family_index: u32) -> Result<Self> {
        let command_pool_info =
            vk::CommandPoolCreateInfo::builder().queue_family_index(queue_family_index);

        let command_pool = unsafe {
            let command_pool = device.raw().create_command_pool(&command_pool_info, None)?;
            device
                .raw()
                .reset_command_pool(command_pool, vk::CommandPoolResetFlags::empty())?;

            command_pool
        };

        Ok(Self {
            raw: command_pool,
            device: device,
        })
    }

    pub fn allocate_command_buffers(
        &self,
        level: vk::CommandBufferLevel,
        count: u32,
    ) -> Result<Vec<vk::CommandBuffer>> {
        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(self.raw)
            .level(level)
            .command_buffer_count(count);

        let command_buffers =
            unsafe { self.device.raw().allocate_command_buffers(&allocate_info)? };

        Ok(command_buffers)
    }
    pub fn allocate_command_buffer(
        &self,
        level: vk::CommandBufferLevel,
    ) -> Result<vk::CommandBuffer> {
        let command_buffers = self.allocate_command_buffers(level, 1)?;
        Ok(command_buffers[0])
    }

    pub fn reset(&self) {
        unsafe {
            self.device
                .raw()
                .reset_command_pool(self.raw, vk::CommandPoolResetFlags::empty())
                .expect("Failed to reset Vulkan command pool!");
        }
    }

    pub fn raw(&self) -> vk::CommandPool {
        self.raw
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        unsafe { self.device.raw().destroy_command_pool(self.raw, None) }
    }
}
pub struct CommandBufferManager {
    device: Arc<Device>,

    command_buffers: Vec<Arc<CommandBuffer>>,
    secondary_command_buffers: Vec<Arc<CommandBuffer>>,
    // XXX: For a "safe" implementation, we technically need to make sure the command pools are always valid/not destroyed

    // Size equal to number of command pools.
    num_used_command_buffers: Vec<u32>,
    num_used_secondary_command_buffers: Vec<u32>,

    num_frames: u32,
    // Equal to number of pools per frame
    num_threads_per_frame: u32,
    num_command_buffers_per_thread: u32,
}

impl CommandBufferManager {
    pub fn new(
        device: Arc<Device>,
        frame_thread_pools_manager: &FrameThreadPoolsManager,
    ) -> Result<Self> {
        let num_frames = constants::MAX_FRAMES;
        let num_threads_per_frame = frame_thread_pools_manager.num_threads();
        let num_command_buffers_per_thread = constants::NUM_COMMAND_BUFFERS_PER_THREAD;

        let num_total_pools = num_threads_per_frame * num_frames;

        let num_used_command_buffers: Vec<u32> = vec![0; num_total_pools as usize];
        let num_used_secondary_command_buffers: Vec<u32> = vec![0; num_total_pools as usize];

        let num_command_buffers = num_total_pools * num_command_buffers_per_thread;
        let mut command_buffers = Vec::<CommandBuffer>::with_capacity(num_command_buffers as usize);

        // XXX: Do we need these actually? On same graphics queue?
        let num_secondary_command_buffers =
            num_total_pools * constants::NUM_SECONDARY_COMMAND_BUFFERS_PER_THREAD;
        let mut secondary_command_buffers =
            Vec::<CommandBuffer>::with_capacity(num_secondary_command_buffers as usize);

        for frame_index in 0..num_frames {
            for thread_index in 0..num_threads_per_frame {
                let command_pool =
                    frame_thread_pools_manager.command_pool_at(frame_index, thread_index);

                // Create primary command buffers.
                for _ in 0..num_command_buffers_per_thread {
                    let array_index = command_buffers.len();
                    let meta_data = CommandBufferMetaData {
                        array_index: array_index as u32,
                        frame_index,
                        thread_index,
                    };

                    let command_buffer =
                        command_pool.allocate_command_buffer(vk::CommandBufferLevel::PRIMARY)?;
                    command_buffers.push(CommandBuffer::new(
                        device.clone(),
                        command_buffer,
                        meta_data,
                        false,
                    ));
                }

                // Create secondary command buffers.
                let current_secondary_buffers = command_pool.allocate_command_buffers(
                    vk::CommandBufferLevel::SECONDARY,
                    constants::NUM_COMMAND_BUFFERS_PER_THREAD,
                )?;
                for i in 0..constants::NUM_SECONDARY_COMMAND_BUFFERS_PER_THREAD {
                    let array_index = secondary_command_buffers.len() as u32;
                    let meta_data = CommandBufferMetaData {
                        array_index,
                        frame_index: u32::MAX,
                        thread_index: u32::MAX,
                    };
                    secondary_command_buffers.push(CommandBuffer::new(
                        device.clone(),
                        current_secondary_buffers[i as usize],
                        meta_data,
                        true,
                    ));
                }
            }
        }

        log::info!(
            "Total number of primary (graphics) command buffers: {}",
            command_buffers.len()
        );
        log::info!(
            "Total number of secondary (graphics) command buffers: {}",
            secondary_command_buffers.len()
        );

        let command_buffers = command_buffers
            .into_iter()
            .map(|command_buffer| Arc::new(command_buffer))
            .collect::<Vec<_>>();

        let secondary_command_buffers = secondary_command_buffers
            .into_iter()
            .map(|command_buffer| Arc::new(command_buffer))
            .collect::<Vec<_>>();

        Ok(Self {
            device,
            command_buffers,
            secondary_command_buffers,
            num_used_command_buffers,
            num_used_secondary_command_buffers,

            num_frames,
            num_threads_per_frame,
            num_command_buffers_per_thread,
        })
    }

    pub fn reset_pools(
        &mut self,
        pools_manager: &FrameThreadPoolsManager,
        frame_index: u32,
    ) -> Result<()> {
        for thread_index in 0..self.num_threads_per_frame {
            let command_pool = pools_manager.command_pool_at(frame_index, thread_index);
            command_pool.reset();

            let pool_index = self.pool_index_from_indices(frame_index, thread_index) as usize;
            self.num_used_command_buffers[pool_index] = 0;
            self.num_used_secondary_command_buffers[pool_index] = 0;
        }

        Ok(())
    }

    // XXX: Do not use Arc to pass around CommandBuffers, return a lightweight structure that queues the command buffer to a submission pool automatically upon destruction?
    //      Have some kind of Guard<CommandBufferManager> for resource safety
    // XXX: Use some kind of RAII guard object
    pub fn command_buffer(
        &mut self,
        frame_index: u32,
        thread_index: u32,
    ) -> Result<Arc<CommandBuffer>> {
        let pool_index = self.pool_index_from_indices(frame_index, thread_index);
        let num_used_buffers = self.num_used_command_buffers[pool_index as usize];

        if num_used_buffers > self.num_command_buffers_per_thread {
            return Err(anyhow!(
                "All command buffers in current frame thread are already used!"
            ));
        }

        self.num_used_command_buffers[pool_index as usize] += 1;

        let index = (pool_index * self.num_command_buffers_per_thread) + num_used_buffers;

        Ok(self.command_buffers[index as usize].clone())
    }

    pub fn secondary_command_buffer(
        &mut self,
        frame_index: u32,
        thread_index: u32,
    ) -> Result<Arc<CommandBuffer>> {
        let pool_index = self.pool_index_from_indices(frame_index, thread_index);
        let num_used_buffers = self.num_used_secondary_command_buffers[pool_index as usize];

        if num_used_buffers > constants::NUM_SECONDARY_COMMAND_BUFFERS_PER_THREAD {
            return Err(anyhow!(
                "All secondary command buffers in current frame thread are already used!"
            ));
        }

        self.num_used_secondary_command_buffers[pool_index as usize] += 1;
        let index =
            (pool_index * constants::NUM_SECONDARY_COMMAND_BUFFERS_PER_THREAD) * num_used_buffers;

        Ok(self.secondary_command_buffers[index as usize].clone())
    }

    fn pool_index_from_indices(&self, frame_index: u32, thread_index: u32) -> u32 {
        assert!(frame_index < constants::MAX_FRAMES);
        assert!(thread_index < self.num_threads_per_frame);

        (frame_index * self.num_threads_per_frame) + thread_index
    }
}

impl Drop for CommandBufferManager {
    fn drop(&mut self) {}
}

// Information for CommandBufferManager
pub struct CommandBufferMetaData {
    // index to command buffer array in CommandBufferManager
    pub array_index: u32,

    pub frame_index: u32,
    pub thread_index: u32,
}
pub struct CommandBuffer {
    device: Arc<Device>,
    raw: vk::CommandBuffer,

    // pub(crate) is_recording: bool,
    pub(crate) is_secondary: bool,
    meta_data: CommandBufferMetaData,
    // Reference to pipeline?
    // pipeline: vk::Pipeline,
    // mesh_shading_context
}

impl CommandBuffer {
    pub(crate) fn new(
        device: Arc<Device>,
        command_buffer: vk::CommandBuffer,
        meta_data: CommandBufferMetaData,
        is_secondary: bool,
    ) -> Self {
        Self {
            device: device.clone(),
            raw: command_buffer,
            // is_recording: false,
            is_secondary,
            meta_data,
        }
    }

    pub fn raw(&self) -> vk::CommandBuffer {
        self.raw
    }

    pub fn begin(&self) -> Result<()> {
        // if !self.is_recording {
        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .raw()
                .begin_command_buffer(self.raw, &begin_info)?
        };
        // self.is_recording = true;
        // } else {
        // log::warn!("Called begin to command buffer that is already recording!");
        // }

        Ok(())
    }

    pub fn end(&self) -> Result<()> {
        // if self.is_recording {
        unsafe { self.device.raw().end_command_buffer(self.raw)? };
        // self.is_recording = false;
        // } else {
        // log::warn!("Called end to command buffer that is not recording!");
        // }

        Ok(())
    }

    pub fn begin_rendering(&self, rendering_state: RenderingState) {
        let mut color_attachments_info = Vec::<vk::RenderingAttachmentInfo>::with_capacity(
            rendering_state.color_attachments.len(),
        );

        for attachment in &rendering_state.color_attachments {
            let rendering_attachment = vk::RenderingAttachmentInfo::builder()
                .image_view(attachment.image_view)
                .image_layout(attachment.image_layout)
                .resolve_mode(vk::ResolveModeFlags::NONE)
                .load_op(attachment.operation.vk_attachment_load_op())
                .store_op(vk::AttachmentStoreOp::STORE)
                .clear_value(if attachment.operation == RenderPassOperation::Clear {
                    vk::ClearValue {
                        color: attachment.clear_value,
                    }
                } else {
                    vk::ClearValue::default()
                });

            color_attachments_info.push(rendering_attachment.build());
        }

        let depth_attachment_info = {
            if let Some(attachment) = rendering_state.depth_attachment {
                vk::RenderingAttachmentInfo::builder()
                    .image_view(attachment.image_view)
                    .image_layout(attachment.image_layout)
                    .resolve_mode(vk::ResolveModeFlags::NONE)
                    .load_op(attachment.depth_operation.vk_attachment_load_op())
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(
                        if attachment.depth_operation == RenderPassOperation::Clear {
                            vk::ClearValue {
                                depth_stencil: attachment.clear_value,
                            }
                        } else {
                            vk::ClearValue::default()
                        },
                    )
            } else {
                vk::RenderingAttachmentInfo::builder()
            }
        };

        let rendering_info = vk::RenderingInfo::builder()
            .flags(if self.is_secondary {
                vk::RenderingFlags::CONTENTS_SECONDARY_COMMAND_BUFFERS
            } else {
                vk::RenderingFlags::empty()
            })
            .color_attachments(&color_attachments_info)
            .depth_attachment(&depth_attachment_info)
            .render_area(vk::Rect2D {
                extent: vk::Extent2D {
                    width: rendering_state.width,
                    height: rendering_state.height,
                },
                offset: vk::Offset2D { x: 0, y: 0 },
            })
            .layer_count(1);

        unsafe {
            self.device
                .raw()
                .cmd_begin_rendering(self.raw, &rendering_info);
        }
    }

    pub fn end_rendering(&self) {
        unsafe {
            self.device.raw().cmd_end_rendering(self.raw);
        }
    }

    pub fn bind_vertex_buffer(&self, buffer: &Buffer, binding: u32, offset: u64) {
        // XXX: Map multiple vertex bufffers at once
        unsafe {
            self.device.raw().cmd_bind_vertex_buffers2(
                self.raw,
                binding,
                &[buffer.raw()],
                &[offset],
                None,
                None,
            )
        }
    }

    pub fn bind_index_buffer(&self, buffer: &Buffer, offset: u64) {
        unsafe {
            self.device.raw().cmd_bind_index_buffer(
                self.raw,
                buffer.raw(),
                offset,
                vk::IndexType::UINT16,
            );
        }
    }

    pub fn bind_graphics_pipeline(&self, pipeline: &GraphicsPipeline) {
        unsafe {
            self.device.raw().cmd_bind_pipeline(
                self.raw,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.raw(),
            );
        }
    }

    // XXX: Need to pass in pipeline layout :(, cache it somewhere inside command buffer? Command buffer will have to be mutable!
    pub fn bind_descriptor_set(
        &self,
        descriptor_set: &DescriptorSet,
        raw_pipeline_layout: vk::PipelineLayout,
        set_index: u32,
    ) {
        unsafe {
            self.device.raw().cmd_bind_descriptor_sets(
                self.raw,
                vk::PipelineBindPoint::GRAPHICS,
                raw_pipeline_layout,
                set_index,
                // std::slice::from_ref(&descriptor_set),
                &[descriptor_set.raw()],
                &[],
            );
        }
    }

    pub fn bind_descriptor_sets(
        &self,
        descriptor_sets: &[&DescriptorSet],
        raw_pipeline_layout: vk::PipelineLayout,
        first_set: u32,
    ) {
        let descriptor_sets = descriptor_sets
            .into_iter()
            .map(|set| set.raw())
            .collect::<Vec<_>>();
        unsafe {
            self.device.raw().cmd_bind_descriptor_sets(
                self.raw,
                vk::PipelineBindPoint::GRAPHICS,
                raw_pipeline_layout,
                first_set,
                &descriptor_sets,
                &[],
            );
        }
    }

    pub fn draw(
        &self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        unsafe {
            self.device.raw().cmd_draw(
                self.raw,
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            );
        }
    }

    pub fn draw_indexed(
        &self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) {
        unsafe {
            self.device.raw().cmd_draw_indexed(
                self.raw,
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            );
        }
    }

    pub fn dispatch(&self, group_count_x: u32, group_count_y: u32, group_count_z: u32) {
        unsafe {
            self.device
                .raw()
                .cmd_dispatch(self.raw, group_count_x, group_count_y, group_count_z);
        }
    }

    pub fn copy_buffer(
        &self,
        src: &Buffer,
        dst: &Buffer,
        size: u64,
        src_offset: u64,
        dst_offset: u64,
    ) {
        // XXX: Since BufferCopy2 is used - queue all copy regions and only execute copy once?
        let region = vk::BufferCopy2::builder()
            .size(size)
            .src_offset(src_offset)
            .dst_offset(dst_offset);

        let info = vk::CopyBufferInfo2::builder()
            .src_buffer(src.raw())
            .dst_buffer(dst.raw())
            .regions(std::slice::from_ref(&region));

        unsafe {
            self.device.raw().cmd_copy_buffer2(self.raw, &info);
        }
    }

    pub fn copy_buffer_to_image(&self, buffer: &Buffer, image: &Image, buffer_offset: u64) {
        // XXX: Since BufferToImageCopy2 is used - queue all copy regions and only execute copy once?
        let region = vk::BufferImageCopy2::builder()
            .buffer_offset(buffer_offset)
            .buffer_row_length(0)
            .buffer_image_height(0)
            // XXX: Handle subresource copy properly
            .image_subresource(
                vk::ImageSubresourceLayers::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .mip_level(0)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            )
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(image.extent());

        let info = vk::CopyBufferToImageInfo2::builder()
            .src_buffer(buffer.raw())
            .dst_image(image.raw())
            .dst_image_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .regions(std::slice::from_ref(&region));

        unsafe {
            self.device.raw().cmd_copy_buffer_to_image2(self.raw, &info);
        }
    }

    pub fn upload_data_to_image<T: Copy>(
        &self,
        image: &Image,
        staging_buffer: &Buffer,
        data: &[T],
    ) -> Result<()> {
        // XXX: Remove this!
        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .raw()
                .begin_command_buffer(self.raw, &begin_info)?
        };

        staging_buffer.copy_data_to_buffer(data)?;

        let barriers = Barriers::new().add_image(
            image,
            ResourceState::UNDEFINED,
            ResourceState::COPY_DESTINATION,
        );
        self.pipeline_barrier(barriers);

        self.copy_buffer_to_image(staging_buffer, image, 0);

        // XXX: Cannot transition to SHADER_RESROUCE state if transfer queue is used.
        //      Need to use another different command buffer in this case...
        let barriers = Barriers::new().add_image(
            image,
            ResourceState::COPY_DESTINATION,
            ResourceState::SHADER_RESOURCE,
        );
        self.pipeline_barrier(barriers);

        unsafe { self.device.raw().end_command_buffer(self.raw)? };

        Ok(())
    }

    pub fn pipeline_barrier(&self, barriers: Barriers) {
        let dependency_info =
            vk::DependencyInfo::builder().image_memory_barriers(barriers.image_barriers());

        unsafe {
            self.device
                .raw()
                .cmd_pipeline_barrier2(self.raw, &dependency_info);
        }
    }
}
