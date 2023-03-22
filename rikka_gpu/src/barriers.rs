use bitflags::bitflags;

use rikka_core::vk;

use crate::{buffer::Buffer, device::Device, image::Image, queue::QueueType};

bitflags! {
    pub struct ResourceState : u32
    {
        const UNDEFINED = 0x0;
        const VERTEX_AND_UNIFORM_BUFFER = 0x1;
        const INDEX_BUFFER = 0x2;
        const RENDER_TARGET = 0x4;

        // SHADER READ + WRITE, Storage buffers and images
        const SHADER_ACCESS = 0x8;

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

        // Shader READ
        const SHADER_RESOURCE = Self::NON_FRAGMENT_SHADER_RESOURCE.bits | Self::FRAGMENT_SHADER_RESOURCE.bits;

        const GENERIC_READ = Self::VERTEX_AND_UNIFORM_BUFFER.bits | Self::INDEX_BUFFER.bits | Self::RENDER_TARGET.bits | Self::SHADER_ACCESS.bits | Self::INDIRECT_ARGUMENT.bits | Self::COPY_SOURCE.bits;
    }
}

impl From<ResourceState> for vk::AccessFlags2 {
    fn from(resource_state: ResourceState) -> Self {
        let mut flags = vk::AccessFlags2::NONE;

        if resource_state.contains(ResourceState::VERTEX_AND_UNIFORM_BUFFER) {
            flags |= vk::AccessFlags2::VERTEX_ATTRIBUTE_READ | vk::AccessFlags2::UNIFORM_READ;
        }

        if resource_state.contains(ResourceState::INDEX_BUFFER) {
            flags |= vk::AccessFlags2::INDEX_READ;
        }

        if resource_state.contains(ResourceState::RENDER_TARGET) {
            // XXX: Maybe we only need READ here?
            flags |=
                vk::AccessFlags2::COLOR_ATTACHMENT_READ | vk::AccessFlags2::COLOR_ATTACHMENT_WRITE;
        }

        if resource_state.contains(ResourceState::SHADER_ACCESS) {
            flags |= vk::AccessFlags2::SHADER_READ | vk::AccessFlags2::SHADER_WRITE;
        }

        if resource_state.contains(ResourceState::COPY_SOURCE) {
            flags |= vk::AccessFlags2::TRANSFER_READ;
        }

        if resource_state.contains(ResourceState::COPY_DESTINATION) {
            flags |= vk::AccessFlags2::TRANSFER_WRITE;
        }

        if resource_state.contains(ResourceState::DEPTH_WRITE) {
            flags |= vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_READ
                | vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE;
        }

        if resource_state.contains(ResourceState::INDIRECT_ARGUMENT) {
            flags |= vk::AccessFlags2::INDIRECT_COMMAND_READ;
        }

        if resource_state.contains(ResourceState::SHADER_RESOURCE) {
            flags |= vk::AccessFlags2::SHADER_READ;
        }

        if resource_state.contains(ResourceState::RAY_TRACING_ACCELERATION_STRUCTURE) {
            flags |= vk::AccessFlags2::ACCELERATION_STRUCTURE_READ_NV
                | vk::AccessFlags2::ACCELERATION_STRUCTURE_WRITE_NV;
        }

        flags
    }
}

impl From<ResourceState> for vk::ImageLayout {
    fn from(resource_state: ResourceState) -> Self {
        // XXX: Is there a nicer way of doing this? Need to order this correctly and prioritize certain bits

        if resource_state.contains(ResourceState::PRESENT) {
            return vk::ImageLayout::PRESENT_SRC_KHR;
        }
        if resource_state.contains(ResourceState::RENDER_TARGET) {
            return vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
        }
        if resource_state.contains(ResourceState::SHADER_ACCESS) {
            return vk::ImageLayout::GENERAL;
        }
        if resource_state.contains(ResourceState::DEPTH_WRITE) {
            return vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;
        }
        if resource_state.contains(ResourceState::DEPTH_READ) {
            return vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL;
        }
        if resource_state.contains(ResourceState::SHADER_RESOURCE) {
            return vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        }
        if resource_state.contains(ResourceState::COPY_SOURCE) {
            return vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
        }
        if resource_state.contains(ResourceState::COPY_DESTINATION) {
            return vk::ImageLayout::TRANSFER_DST_OPTIMAL;
        }
        if resource_state.contains(ResourceState::COMMON) {
            return vk::ImageLayout::GENERAL;
        }

        vk::ImageLayout::UNDEFINED
    }
}

// XXX: Handle mesh shader flags?
//      Maybe a ResourceState -> PipelineFlags conversion would be better (eg. can handle ResourceState::Present)
fn determine_pipeline_flags_from_access_flags(
    access_flags: vk::AccessFlags2,
    queue_type: QueueType,
) -> vk::PipelineStageFlags2 {
    let mut flags = vk::PipelineStageFlags2::empty();

    if access_flags.is_empty() {
        flags = vk::PipelineStageFlags2::TOP_OF_PIPE;
    }
    if access_flags.contains(vk::AccessFlags2::INDIRECT_COMMAND_READ) {
        flags |= vk::PipelineStageFlags2::DRAW_INDIRECT;
    }
    if access_flags.contains(vk::AccessFlags2::TRANSFER_READ | vk::AccessFlags2::TRANSFER_WRITE) {
        flags |= vk::PipelineStageFlags2::TRANSFER;
    }
    if access_flags.contains(vk::AccessFlags2::HOST_READ | vk::AccessFlags2::HOST_WRITE) {
        flags |= vk::PipelineStageFlags2::HOST;
    }

    match queue_type {
        QueueType::Graphics => {
            if access_flags
                .contains(vk::AccessFlags2::VERTEX_ATTRIBUTE_READ | vk::AccessFlags2::INDEX_READ)
            {
                flags |= vk::PipelineStageFlags2::VERTEX_ATTRIBUTE_INPUT;
            }
            if access_flags.contains(vk::AccessFlags2::UNIFORM_READ)
                || access_flags.contains(vk::AccessFlags2::SHADER_READ)
                || access_flags.contains(vk::AccessFlags2::SHADER_WRITE)
            {
                flags |= vk::PipelineStageFlags2::VERTEX_SHADER
                    | vk::PipelineStageFlags2::FRAGMENT_SHADER;

                // XXX: Need compute access as well?
                // flags |= vk::PipelineStageFlags2::COMPUTE_SHADER;

                // XXX: Mesh shaders access?
                // flags |= vk::PipelineStageFlags2::MESH_SHADER_NV | vk::PipelineStageFlags2::TASK_SHADER_NV;

                // XXX: Ray tracing access?
                // flags |= vk::PipelineStageFlags2::RAY_TRACING_SHADER_NV;
            }
            if access_flags.contains(vk::AccessFlags2::INPUT_ATTACHMENT_READ) {
                flags |= vk::PipelineStageFlags2::FRAGMENT_SHADER;
            }
            if access_flags.contains(
                vk::AccessFlags2::COLOR_ATTACHMENT_READ | vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
            ) {
                flags |= vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT;
            }
            if access_flags.contains(
                vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_READ
                    | vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
            ) {
                flags |= vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS;
            }

            // XXX: Only use transfer queue for these
            if access_flags.contains(vk::AccessFlags2::TRANSFER_READ)
                || access_flags.contains(vk::AccessFlags2::TRANSFER_WRITE)
            {
                flags |= vk::PipelineStageFlags2::TRANSFER;
            }
        }
        QueueType::Compute => {
            if access_flags.contains(
                vk::AccessFlags2::UNIFORM_READ
                    | vk::AccessFlags2::SHADER_READ
                    | vk::AccessFlags2::SHADER_WRITE,
            ) {
                flags |= vk::PipelineStageFlags2::RAY_TRACING_SHADER_NV;
            }

            todo!()
        }
        QueueType::Transfer => {
            if access_flags.contains(vk::AccessFlags2::TRANSFER_READ)
                || access_flags.contains(vk::AccessFlags2::TRANSFER_WRITE)
            {
                flags |= vk::PipelineStageFlags2::TRANSFER;
            }
        }
    }

    flags
}

pub struct Barriers {
    image_barriers: Vec<vk::ImageMemoryBarrier2>,
    // XXX: Technically need to hold references to images/buffers to make sure they are still valid when pipelining the barrier?
}

impl Barriers {
    pub fn new() -> Self {
        Self {
            image_barriers: vec![],
        }
    }

    // XXX: Make this accept self and return self
    pub fn add_image(&mut self, image: &Image, old_state: ResourceState, new_state: ResourceState) {
        self.add_image_from_vulkan_parameters(
            old_state.into(),
            determine_pipeline_flags_from_access_flags(old_state.into(), QueueType::Graphics),
            new_state.into(),
            determine_pipeline_flags_from_access_flags(new_state.into(), QueueType::Graphics),
            old_state.into(),
            new_state.into(),
            image.raw(),
            image.subresource_range(),
        )
    }

    pub fn add_image_from_vulkan_parameters(
        &mut self,
        src_access_mask: vk::AccessFlags2,
        src_stage_mask: vk::PipelineStageFlags2,
        dst_acces_mask: vk::AccessFlags2,
        dst_stage_mask: vk::PipelineStageFlags2,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        image: vk::Image,
        subresource_range: vk::ImageSubresourceRange,
    ) {
        let image_barrier = vk::ImageMemoryBarrier2::builder()
            .src_access_mask(src_access_mask)
            .src_stage_mask(src_stage_mask)
            .dst_access_mask(dst_acces_mask)
            .dst_stage_mask(dst_stage_mask)
            .old_layout(old_layout)
            .new_layout(new_layout)
            .image(image)
            .subresource_range(subresource_range)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED);

        self.image_barriers.push(image_barrier.build());
    }

    pub fn image_barriers(&self) -> &[vk::ImageMemoryBarrier2] {
        &self.image_barriers
    }
}
