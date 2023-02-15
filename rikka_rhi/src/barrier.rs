use bitflags::bitflags;

use ash::vk;

bitflags! {
    pub struct ResourceState : u32
    {
        const UNDEFINED = 0x0;
        const VERTEX_AND_UNIFORM_BUFFER = 0x1;
        const INDEX_BUFFER = 0x2;
        const RENDER_TARGET = 0x4;
        const UNORDERED_ACCESS = 0x8;
        const DEPTH_WRITE = 0x10;
        const DEPTH_READ = 0x20;
        const NON_FRAGMENT_SHADER_RESOURCE = 0x40;
        const FRAGMENT_SHADER_RESOURCE = 0x80;
        const STREAM_OUT = 0x100;
        const INDIRECT_ARGUMENT = 0x200;
        const COPY_DESTINATION = 0x400;
        const COPY_SOURCE = 0x800;
        const PRESENT = 0x1000;
        const COMMON = 0x2000;
        const RAY_TRACING_ACCELERATION_STRUCTURE = 0x4000;
        const SHADING_RATE_RESOURCE = 0x8000;

        const SHADER_RESOURCE = Self::NON_FRAGMENT_SHADER_RESOURCE.bits | Self::FRAGMENT_SHADER_RESOURCE.bits;
        const GENERIC_READ = Self::VERTEX_AND_UNIFORM_BUFFER.bits | Self::INDEX_BUFFER.bits | Self::RENDER_TARGET.bits | Self::UNORDERED_ACCESS.bits | Self::INDIRECT_ARGUMENT.bits | Self::COPY_SOURCE.bits;
    }
}

pub struct Barrier {}
