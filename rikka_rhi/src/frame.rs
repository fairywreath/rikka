use core::num;
use std::sync::{Arc, Weak};

use anyhow::Result;
use ash::vk;

use crate::{
    command_buffer::*,
    constants,
    device::Device,
    query::{PipelineStatsQueryPool, TimestampQueryPool},
    queue::*,
    swapchain::*,
    synchronization::*,
};

pub struct FrameThreadPools {
    // Graphics and present command pool.
    pub command_pool: CommandPool,
    pub timestamp_query_pool: TimestampQueryPool,
    pub pipeline_stats_query_pool: PipelineStatsQueryPool,
}

pub struct FrameThreadPoolsDesc {
    pub num_threads: u32,
    pub time_queries_per_frame: u32,
    pub graphics_queue_family_index: u32,
}

pub struct FrameThreadPoolsManager {
    device: Arc<Device>,
    frame_thread_pools: Vec<FrameThreadPools>,

    num_threads: u32,
    time_queries_per_frame: u32,
}

impl FrameThreadPoolsManager {
    pub fn new(device: Arc<Device>, desc: FrameThreadPoolsDesc) -> Result<Self> {
        let num_pools = desc.num_threads * constants::MAX_FRAMES;

        let mut frame_thread_pools: Vec<FrameThreadPools> = vec![];
        frame_thread_pools.reserve(num_pools as usize);

        for _ in 0..num_pools {
            // XXX: Use graphics queue only for all command buffers?
            let command_pool = CommandPool::new(device.clone(), desc.graphics_queue_family_index)?;
            let timestamp_query_pool =
                TimestampQueryPool::new(device.clone(), desc.time_queries_per_frame)?;
            let pipeline_stats_query_pool = PipelineStatsQueryPool::new(device.clone())?;

            frame_thread_pools.push(FrameThreadPools {
                command_pool,
                timestamp_query_pool,
                pipeline_stats_query_pool,
            });
        }

        Ok(Self {
            device,
            frame_thread_pools,
            num_threads: desc.num_threads,
            time_queries_per_frame: desc.time_queries_per_frame,
        })
    }

    pub fn pools_at(&self, frame_index: u32, thread_index: u32) -> &FrameThreadPools {
        assert!(frame_index < constants::MAX_FRAMES);
        assert!(thread_index < self.num_threads);

        let index = (frame_index * self.num_threads) + thread_index;

        &self.frame_thread_pools[index as usize]
    }

    pub fn pools_at_index(&self, index: u32) -> &FrameThreadPools {
        assert!(index < self.frame_thread_pools.len() as u32);
        &self.frame_thread_pools[index as usize]
    }

    pub fn command_pool_at(&self, frame_index: u32, thread_index: u32) -> &CommandPool {
        let pools = self.pools_at(frame_index, thread_index);

        &pools.command_pool
    }

    pub fn num_threads(&self) -> u32 {
        self.num_threads
    }
}

pub struct FrameIndexData {
    pub current: u64,
    pub previous: u64,
    pub absolute: u64,
}

pub struct FrameSynchronizationManager {
    device: Arc<Device>,

    frame_index_data: FrameIndexData,

    // render_complete_semaphores: [Semaphore; constants::MAX_FRAMES as usize],
    render_complete_semaphores: Vec<Semaphore>,
    swapchain_image_acquired_semaphore: Semaphore,
    graphics_work_semaphore: Semaphore,
    compute_work_semaphore: Semaphore,
    // transfer_work_semaphore: Semaphore,
    last_compute_semaphore_value: u64,
    has_async_work: bool,
}

impl FrameSynchronizationManager {
    pub(crate) fn new(device: Arc<Device>) -> Result<Self> {
        let mut render_complete_semaphores =
            Vec::<Semaphore>::with_capacity(constants::MAX_FRAMES as usize);
        for i in 0..constants::MAX_FRAMES as usize {
            render_complete_semaphores.push(Semaphore::new(device.clone(), SemaphoreType::Binary)?);
        }

        let swapchain_image_acquired_semaphore =
            Semaphore::new(device.clone(), SemaphoreType::Binary)?;
        let graphics_work_semaphore = Semaphore::new(device.clone(), SemaphoreType::Timeline)?;
        let compute_work_semaphore = Semaphore::new(device.clone(), SemaphoreType::Timeline)?;

        let frame_index_data = FrameIndexData {
            current: 0,
            previous: 0,
            absolute: 0,
        };

        Ok(Self {
            device,
            render_complete_semaphores,
            frame_index_data,
            swapchain_image_acquired_semaphore,
            graphics_work_semaphore,

            compute_work_semaphore,
            last_compute_semaphore_value: 0,
            has_async_work: false,
        })
    }

    pub fn frame_index_data(&self) -> &FrameIndexData {
        &self.frame_index_data
    }

    pub fn current_render_complete_semaphore(&self) -> &Semaphore {
        &self.render_complete_semaphores[self.frame_index_data.current as usize]
    }

    pub fn render_complete_semaphore(&self, index: usize) -> &Semaphore {
        assert!(index < self.render_complete_semaphores.len());
        &self.render_complete_semaphores[index]
    }

    pub fn swapchain_image_acquired_semaphore(&self) -> &Semaphore {
        &self.swapchain_image_acquired_semaphore
    }

    pub fn graphics_work_semaphore(&self) -> &Semaphore {
        &self.graphics_work_semaphore
    }

    pub fn compute_work_semaphore(&self) -> &Semaphore {
        &self.compute_work_semaphore
    }

    pub fn has_async_work(&self) -> bool {
        self.has_async_work
    }

    pub fn absolute_frame_index(&self) -> u64 {
        self.frame_index_data.absolute
    }

    pub fn current_frame_index(&self) -> u64 {
        self.frame_index_data.current
    }

    pub fn previous_frame_index(&self) -> u64 {
        self.frame_index_data.previous
    }

    pub fn advance_frame_counters(&mut self) {
        self.frame_index_data.previous = self.frame_index_data.current;
        self.frame_index_data.current =
            (self.frame_index_data.current + 1) % (constants::MAX_FRAMES as u64);
        self.frame_index_data.absolute += 1;

        log::error!("NEW CURRENT INDEX: {}", self.frame_index_data.current);
    }

    pub fn wait_graphics_compute_semaphores(&self) -> Result<()> {
        // This if statement is really ugly, since it is satisfied every frame except for the first few
        if self.frame_index_data.absolute >= constants::MAX_FRAMES as u64 {
            let graphics_wait_value = self.graphics_semaphore_wait_value();
            // let graphics_wait_value = self.frame_index_data.absolute;
            // let graphics_wait_value = 0;
            // let compute_wait_value = self.last_compute_semaphore_value;

            log::info!("Waiting on value: {}", graphics_wait_value);

            let current_value = unsafe {
                self.device
                    .raw()
                    .get_semaphore_counter_value(self.graphics_work_semaphore.raw())?
            };

            log::info!(
                "Current GRAPHICS TIMELINE semaphore value: {}",
                current_value
            );

            let wait_values = [
                graphics_wait_value,
                // compute_wait_value
            ];
            let semaphores = [
                self.graphics_work_semaphore.raw(),
                // self.compute_work_semaphore.raw(),
            ];

            let wait_info = vk::SemaphoreWaitInfo::builder()
                .semaphores(&semaphores)
                .values(&wait_values);

            unsafe { self.device.raw().wait_semaphores(&wait_info, u64::MAX)? };
        }

        Ok(())
    }

    // XXX: Put this logic somwhere else?
    pub fn submit_graphics_command_buffers(
        &self,
        command_buffers: &Vec<Weak<CommandBuffer>>,
        queue: &Queue,
    ) -> Result<()> {
        // Wait for a max of 1 image acuired semaphore + graphics + compute = 3 total.
        let mut wait_semaphores = Vec::<SemaphoreSubmitInfo>::with_capacity(3);

        // Wait for image acquired semaphore.
        wait_semaphores.push(SemaphoreSubmitInfo {
            semaphore: &self.swapchain_image_acquired_semaphore,
            stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            value: None,
        });

        // Wait for graphics semaphore.
        // XXX: Do we need these? since we can wait directly in wait_graphics_compute_semaphores()?
        if self.frame_index_data.absolute >= constants::MAX_FRAMES as u64 {
            let graphics_wait_info = SemaphoreSubmitInfo {
                semaphore: &self.graphics_work_semaphore,
                stage_mask: vk::PipelineStageFlags2::TOP_OF_PIPE,
                value: Some(self.graphics_semaphore_wait_value()),
            };

            wait_semaphores.push(graphics_wait_info);
        }

        // Wait for compute semaphore.
        // XXX: Do we need these? since we can wait directly in wait_graphics_compute_semaphores()?
        // if self.has_async_work && self.last_compute_semaphore_value > 0 {
        //     let compute_wait_info = SemaphoreSubmitInfo {
        //         semaphore: &self.compute_work_semaphore,
        //         stage_mask: vk::PipelineStageFlags2::VERTEX_ATTRIBUTE_INPUT,
        //         value: Some(self.last_compute_semaphore_value),
        //     };

        //     wait_semaphores.push(compute_wait_info);
        // }

        // Signal present/render complete semaphore and new graphics timeline value.
        let signal_semaphores = vec![
            SemaphoreSubmitInfo {
                semaphore: self.current_render_complete_semaphore(),
                stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                value: None,
            },
            SemaphoreSubmitInfo {
                semaphore: &self.graphics_work_semaphore,
                stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                value: Some(self.frame_index_data.absolute + 1),
            },
        ];

        log::info!(
            "Signalling graphics semaphore of value: {}",
            signal_semaphores[1].value.unwrap(),
        );

        queue.submit(command_buffers, wait_semaphores, signal_semaphores)?;

        Ok(())
    }

    fn graphics_semaphore_wait_value(&self) -> u64 {
        self.frame_index_data.absolute - (constants::MAX_FRAMES as u64 - 1)
    }
}
