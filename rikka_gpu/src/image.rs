use std::{
    mem::{align_of, size_of_val, swap},
    ops::Deref,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Error, Result};
use ash::vk;
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc, Allocator},
    MemoryLocation,
};

use crate::{
    barriers::ResourceState,
    command_buffer,
    device::Device,
    frame::{self, FrameThreadPoolsManager},
    pipeline::*,
    sampler::Sampler,
    swapchain::Swapchain,
    types::*,
};

pub struct ImageDesc {
    pub width: u32,
    pub height: u32,
    pub depth: u32,

    pub array_layer_count: u32,
    pub mip_level_count: u32,

    pub format: vk::Format,
    pub image_type: vk::ImageType,
    pub usage_flags: vk::ImageUsageFlags,
    // name: String,
    memory_location: MemoryLocation,
}

impl ImageDesc {
    pub fn new(width: u32, height: u32, depth: u32) -> Self {
        Self {
            width,
            height,
            depth,
            array_layer_count: 1,
            mip_level_count: 1,
            format: vk::Format::UNDEFINED,
            image_type: vk::ImageType::TYPE_2D,
            usage_flags: vk::ImageUsageFlags::empty(),
            memory_location: MemoryLocation::GpuOnly,
        }
    }

    pub fn set_format(mut self, format: vk::Format) -> Self {
        self.format = format;
        self
    }

    pub fn set_usage_flags(mut self, usage_flags: vk::ImageUsageFlags) -> Self {
        self.usage_flags = usage_flags;
        self
    }
}

pub struct ImageViewDesc {
    pub image: vk::Image,
    pub view_type: vk::ImageViewType,
    pub format: vk::Format,
    pub subresource_range: vk::ImageSubresourceRange,
}

fn vulkan_image_type_to_view_type(image_type: vk::ImageType) -> vk::ImageViewType {
    match image_type {
        vk::ImageType::TYPE_2D => vk::ImageViewType::TYPE_2D,
        _ => {
            todo!()
        }
    }
}

fn format_has_depth(format: vk::Format) -> bool {
    match format {
        vk::Format::D32_SFLOAT_S8_UINT
        | vk::Format::D32_SFLOAT
        | vk::Format::D24_UNORM_S8_UINT
        | vk::Format::D16_UNORM_S8_UINT => true,
        _ => false,
    }
}

fn format_has_stencil(format: vk::Format) -> bool {
    match format {
        vk::Format::D32_SFLOAT_S8_UINT
        | vk::Format::D24_UNORM_S8_UINT
        | vk::Format::D16_UNORM_S8_UINT => true,
        _ => false,
    }
}

// XXX: Need a first-class ImageView type as well. Can be useful for example the use cases of different image views for the same image
pub struct Image {
    device: Arc<Device>,

    raw: vk::Image,
    raw_view: vk::ImageView,

    allocator: Option<Arc<Mutex<Allocator>>>,
    allocation: Option<Allocation>,

    resource_state: ResourceState,

    // XXX: This struct contains to much stuff...move/remove some of these?
    format: vk::Format,
    extent: vk::Extent3D,
    mip_levels: u32,
    array_layers: u32,
    image_type: vk::ImageType,

    // Image view info
    subresource_range: vk::ImageSubresourceRange,

    sampler: Option<Arc<Sampler>>,
    owning: bool,
}

impl Image {
    pub(crate) fn new(
        device: Arc<Device>,
        allocator: Arc<Mutex<Allocator>>,
        desc: ImageDesc,
    ) -> Result<Self> {
        let usage_flags = desc.usage_flags
            | vk::ImageUsageFlags::TRANSFER_SRC
            | vk::ImageUsageFlags::TRANSFER_DST;
        let extent = vk::Extent3D {
            width: desc.width,
            height: desc.height,
            depth: desc.depth,
        };

        let create_info = vk::ImageCreateInfo::builder()
            .image_type(desc.image_type)
            .format(desc.format)
            .extent(extent)
            .mip_levels(desc.mip_level_count)
            .array_layers(desc.array_layer_count)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage_flags)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let raw = unsafe {
            device
                .raw()
                .create_image(&create_info, None)
                .context("Failed to create vulkan image")?
        };
        let requirements = unsafe { device.raw().get_image_memory_requirements(raw) };

        // XXX: Always GPU only (and use staging buffer to copy)?
        // let memory_location = MemoryLocation::GpuOnly;

        let allocation = allocator.lock().unwrap().allocate(&AllocationCreateDesc {
            name: "image",
            requirements,
            location: desc.memory_location,
            linear: true,
        })?;

        unsafe {
            device
                .raw()
                .bind_image_memory(raw, allocation.memory(), allocation.offset())?
        };

        let mut aspect_flags = vk::ImageAspectFlags::empty();
        if format_has_depth(desc.format) {
            aspect_flags |= vk::ImageAspectFlags::DEPTH;
        } else {
            aspect_flags |= vk::ImageAspectFlags::COLOR;
        }

        let subresource_range = vk::ImageSubresourceRange::builder()
            .aspect_mask(aspect_flags)
            .base_mip_level(0)
            .level_count(desc.mip_level_count)
            .base_array_layer(0)
            .layer_count(desc.array_layer_count)
            .build();

        let raw_view = Self::create_vulkan_image_view(
            device.as_ref(),
            ImageViewDesc {
                image: raw,
                view_type: vulkan_image_type_to_view_type(desc.image_type),
                format: desc.format,
                subresource_range,
            },
        )?;

        Ok(Self {
            device,
            raw,
            raw_view,
            allocator: Some(allocator),
            allocation: Some(allocation),
            resource_state: ResourceState::UNDEFINED,
            format: desc.format,
            extent,
            mip_levels: desc.mip_level_count,
            array_layers: desc.array_layer_count,
            subresource_range,
            image_type: desc.image_type,
            sampler: None,
            owning: true,
        })
    }

    pub(crate) fn from_swapchain(
        swapchain: &Swapchain,
        raw: vk::Image,
        raw_view: vk::ImageView,
    ) -> Self {
        // XXX: Create image view here as well?
        Self {
            device: swapchain.device().clone(),
            raw,
            raw_view,
            allocator: None,
            allocation: None,
            resource_state: ResourceState::UNDEFINED,
            format: swapchain.format(),
            extent: vk::Extent3D {
                width: swapchain.extent().width,
                height: swapchain.extent().height,
                depth: 0,
            },
            mip_levels: 1,
            array_layers: 1,
            subresource_range: vk::ImageSubresourceRange::builder()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1)
                .build(),
            image_type: vk::ImageType::TYPE_2D,
            sampler: None,
            owning: false,
        }
    }

    fn create_vulkan_image_view(device: &Device, desc: ImageViewDesc) -> Result<vk::ImageView> {
        let create_info = vk::ImageViewCreateInfo::builder()
            .image(desc.image)
            .view_type(desc.view_type)
            .format(desc.format)
            .subresource_range(desc.subresource_range);

        let image_view = {
            unsafe {
                device
                    .raw()
                    .create_image_view(&create_info, None)
                    .context("Failed to create vulkan image view")?
            }
        };

        Ok(image_view)
    }

    pub fn raw(&self) -> vk::Image {
        self.raw
    }

    pub fn raw_view(&self) -> vk::ImageView {
        self.raw_view
    }

    pub fn has_linked_sampler(&self) -> bool {
        self.sampler.is_some()
    }

    pub fn linked_sampler(&self) -> Option<Arc<Sampler>> {
        self.sampler.clone()
    }

    pub fn set_linked_sampler(&mut self, sampler: Arc<Sampler>) {
        self.sampler = Some(sampler);
    }

    pub fn extent(&self) -> vk::Extent3D {
        self.extent
    }

    pub fn has_depth(&self) -> bool {
        format_has_depth(self.format)
    }

    pub fn subresource_range(&self) -> vk::ImageSubresourceRange {
        self.subresource_range
    }

    pub fn base_mip_level(&self) -> u32 {
        self.subresource_range.base_mip_level
    }

    pub fn mip_levels(&self) -> u32 {
        self.subresource_range.level_count
    }

    pub fn aspect_mask(&self) -> vk::ImageAspectFlags {
        self.subresource_range.aspect_mask
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if self.owning {
            // XXX: tf is this
            self.allocator
                .clone()
                .unwrap()
                .lock()
                .unwrap()
                .free(self.allocation.take().unwrap())
                .unwrap();

            unsafe {
                self.device.raw().destroy_image(self.raw, None);
                self.device.raw().destroy_image_view(self.raw_view, None);
            };
        }
    }
}
