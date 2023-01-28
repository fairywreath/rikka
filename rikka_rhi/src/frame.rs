use core::num;
use std::sync::Arc;

use anyhow::Result;
use ash::vk;

use crate::constants;

use crate::{
    command_buffer::CommandPool,
    device::Device,
    query::{PipelineStatsQueryPool, TimestampQueryPool},
};

pub struct ThreadFramePools {
    pub command_pool: CommandPool,
    pub timestamp_query_pool: TimestampQueryPool,
    pub pipeline_stats_query_pool: PipelineStatsQueryPool,
}

pub struct ThreadFramePoolsDesc {
    pub num_threads: u32,
    pub time_queries_per_frame: u32,
    pub graphics_queue_family_index: u32,
}

pub struct ThreadFramePoolsManager {
    device: Arc<Device>,
    thread_frame_pools: Vec<ThreadFramePools>,

    num_threads: u32,
    time_queries_per_frame: u32,
}

impl ThreadFramePoolsManager {
    pub fn new(device: Arc<Device>, desc: ThreadFramePoolsDesc) -> Result<Self> {
        let num_pools = desc.num_threads * constants::MAX_FRAMES;

        let mut thread_frame_pools: Vec<ThreadFramePools> = vec![];
        thread_frame_pools.reserve(num_pools as usize);

        for _ in 0..num_pools {
            // XXX: Use graphics queue only for all command buffers?
            let command_pool = CommandPool::new(device.clone(), desc.graphics_queue_family_index)?;
            let timestamp_query_pool =
                TimestampQueryPool::new(device.clone(), desc.time_queries_per_frame)?;
            let pipeline_stats_query_pool = PipelineStatsQueryPool::new(device.clone())?;

            thread_frame_pools.push(ThreadFramePools {
                command_pool,
                timestamp_query_pool,
                pipeline_stats_query_pool,
            });
        }

        Ok(Self {
            device,
            thread_frame_pools,
            num_threads: desc.num_threads,
            time_queries_per_frame: desc.time_queries_per_frame,
        })
    }
}
