use anyhow::Result;
use rikka_core::vk;

use crate::factory::DeviceGuard;

pub struct TimestampQueryPool {
    device: DeviceGuard,

    query_pool: vk::QueryPool,

    // Are these needed?
    time_queries_per_frame: u32,
    total_query_count: u32,
}

impl TimestampQueryPool {
    pub fn new(device: DeviceGuard, time_queries_per_frame: u32) -> Result<Self> {
        let pool_info = vk::QueryPoolCreateInfo::builder()
            .query_type(vk::QueryType::TIMESTAMP)
            .query_count(time_queries_per_frame * 2);

        let query_pool = unsafe { device.raw().create_query_pool(&pool_info, None)? };

        Ok(Self {
            device,
            query_pool,
            time_queries_per_frame,
            total_query_count: time_queries_per_frame * 2,
        })
    }
}

impl Drop for TimestampQueryPool {
    fn drop(&mut self) {
        unsafe { self.device.raw().destroy_query_pool(self.query_pool, None) }
    }
}

pub struct PipelineStatsQueryPool {
    device: DeviceGuard,
    query_pool: vk::QueryPool,
}

impl PipelineStatsQueryPool {
    pub fn new(device: DeviceGuard) -> Result<Self> {
        let pipeline_stats_flags = {
            use vk::QueryPipelineStatisticFlags as flags;

            let query_flags = flags::INPUT_ASSEMBLY_VERTICES
                | flags::INPUT_ASSEMBLY_PRIMITIVES
                | flags::VERTEX_SHADER_INVOCATIONS
                | flags::CLIPPING_INVOCATIONS
                | flags::CLIPPING_PRIMITIVES
                | flags::FRAGMENT_SHADER_INVOCATIONS
                | flags::COMPUTE_SHADER_INVOCATIONS;

            query_flags
        };

        let pool_info = vk::QueryPoolCreateInfo::builder()
            .query_type(vk::QueryType::PIPELINE_STATISTICS)
            .query_count(7)
            .pipeline_statistics(pipeline_stats_flags);

        let query_pool = unsafe { device.raw().create_query_pool(&pool_info, None)? };

        Ok(Self { device, query_pool })
    }
}

impl Drop for PipelineStatsQueryPool {
    fn drop(&mut self) {
        unsafe { self.device.raw().destroy_query_pool(self.query_pool, None) }
    }
}
