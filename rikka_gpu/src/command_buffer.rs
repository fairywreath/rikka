use std::{
    mem::swap,
    sync::{Arc, Mutex, Weak},
};

use anyhow::{anyhow, Result};
use ash::vk::{self, RenderingAttachmentInfo};

use crate::{
    command_buffer,
    constants::{self, NUM_COMMAND_BUFFERS_PER_THREAD},
    descriptor_set::DescriptorSet,
    device::Device,
    frame::{self, FrameThreadPoolsManager},
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
            unsafe {
                self.device
                    .raw()
                    .reset_command_pool(command_pool.raw(), vk::CommandPoolResetFlags::empty())?;
            }

            let pool_index = self.pool_index_from_indices(frame_index, thread_index) as usize;
            self.num_used_command_buffers[pool_index] = 0;
            self.num_used_secondary_command_buffers[pool_index] = 0;
        }

        Ok(())
    }

    pub fn command_buffer(
        &mut self,
        frame_index: u32,
        thread_index: u32,
    ) -> Result<Weak<CommandBuffer>> {
        let pool_index = self.pool_index_from_indices(frame_index, thread_index);
        let num_used_buffers = self.num_used_command_buffers[pool_index as usize];

        if num_used_buffers > self.num_command_buffers_per_thread {
            return Err(anyhow!(
                "All command buffers in current frame thread are already used!"
            ));
        }

        // XXX: Handle multiple command buffer usage in one single thread.
        // self.num_used_command_buffers[pool_index as usize] += 1;

        let index = (pool_index * self.num_command_buffers_per_thread) + num_used_buffers;

        let command_buffer = Arc::downgrade(&self.command_buffers[index as usize]);
        Ok(command_buffer)
    }

    pub fn secondary_command_buffer(
        &mut self,
        frame_index: u32,
        thread_index: u32,
    ) -> Result<Weak<CommandBuffer>> {
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

        let command_buffer = Arc::downgrade(&self.secondary_command_buffers[index as usize]);

        Ok(command_buffer)
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

    pub(crate) is_recording: bool,
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
            is_recording: false,
            is_secondary,
            meta_data,
        }
    }

    pub fn raw(&self) -> vk::CommandBuffer {
        self.raw
    }

    pub fn begin(&mut self) -> Result<()> {
        if !self.is_recording {
            let begin_info = vk::CommandBufferBeginInfo::builder()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            unsafe {
                self.device
                    .raw()
                    .begin_command_buffer(self.raw, &begin_info)?
            };
            self.is_recording = true;
        } else {
            log::warn!("Called begin to command buffer that is already recording!");
        }

        Ok(())
    }

    pub fn end(&mut self) -> Result<()> {
        if self.is_recording {
            unsafe { self.device.raw().end_command_buffer(self.raw)? };
            self.is_recording = false;
        } else {
            log::warn!("Called end to command buffer that is not recording!");
        }

        Ok(())
    }

    pub fn begin_rendering(&self, rendering_state: RenderingState) {
        let mut color_attachments_info = Vec::<vk::RenderingAttachmentInfo>::with_capacity(
            rendering_state.color_attachments.len(),
        );

        for attachment in rendering_state.color_attachments {
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
            if rendering_state.depth_attachment.is_some() {
                let attachment = rendering_state.depth_attachment.unwrap();
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
    ) {
        unsafe {
            self.device.raw().cmd_bind_descriptor_sets(
                self.raw,
                vk::PipelineBindPoint::GRAPHICS,
                raw_pipeline_layout,
                0,
                // std::slice::from_ref(&descriptor_set),
                &[descriptor_set.raw()],
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

    pub fn test_record_commands(
        &self,
        swapchain: &Swapchain,
        graphics_pipeline: &GraphicsPipeline,
        descriptor_set: &DescriptorSet,
    ) -> Result<()> {
        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .raw()
                .begin_command_buffer(self.raw, &begin_info)?
        };

        let image_memory_barrier = vk::ImageMemoryBarrier::builder()
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .image(swapchain.current_image())
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            )
            .build();

        unsafe {
            self.device.raw().cmd_pipeline_barrier(
                self.raw,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[image_memory_barrier],
            );
        }

        let color_attachment = RenderColorAttachment::new()
            .set_clear_value(vk::ClearColorValue {
                float32: [1.0, 1.0, 1.0, 1.0],
            })
            .set_operation(RenderPassOperation::Clear)
            .set_image_view(swapchain.current_image_view())
            .set_image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        let rendering_state =
            RenderingState::new(swapchain.extent().width, swapchain.extent().height)
                .add_color_attachment(color_attachment);
        self.begin_rendering(rendering_state);

        self.bind_graphics_pipeline(graphics_pipeline);
        self.bind_descriptor_set(&descriptor_set, graphics_pipeline.raw_layout());
        self.draw(6, 1, 0, 0);

        self.end_rendering();

        let image_memory_barrier = vk::ImageMemoryBarrier::builder()
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .image(swapchain.current_image())
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            )
            .build();

        unsafe {
            self.device.raw().cmd_pipeline_barrier(
                self.raw,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[image_memory_barrier],
            );
        }

        unsafe { self.device.raw().end_command_buffer(self.raw)? };

        Ok(())
    }
}
