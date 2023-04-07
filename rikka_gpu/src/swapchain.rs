use anyhow::{Context, Result};
use rikka_core::{ash::extensions::khr, vk};

use crate::{
    device::Device, escape::Escape, factory::*, image::Image, instance::Instance,
    physical_device::PhysicalDevice, queue::Queue, surface::Surface, synchronization::Semaphore,
};

pub struct Swapchain {
    device: DeviceGuard,
    ash_swapchain: khr::Swapchain,
    vulkan_swapchain: vk::SwapchainKHR,

    graphics_queue_family_index: u32,
    present_queue_family_index: u32,

    format: vk::Format,
    extent: vk::Extent2D,
    color_space: vk::ColorSpaceKHR,
    present_mode: vk::PresentModeKHR,

    image_count: u32,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    image_handles: Vec<Image>,

    // Image index obtained from AcquireNextImage.
    vulkan_image_index: u32,
}

pub struct SwapchainDesc {
    pub width: u32,
    pub height: u32,

    pub graphics_queue_family_index: u32,
    pub present_queue_family_index: u32,

    pub present_mode: vk::PresentModeKHR,
}

impl SwapchainDesc {
    pub fn new(
        width: u32,
        height: u32,
        graphics_queue_family_index: u32,
        present_queue_family_index: u32,
    ) -> Self {
        SwapchainDesc {
            width,
            height,
            graphics_queue_family_index,
            present_queue_family_index,
            present_mode: vk::PresentModeKHR::FIFO,
        }
    }

    pub fn set_present_mode(mut self, present_mode: vk::PresentModeKHR) -> Self {
        self.present_mode = present_mode;
        self
    }
}

impl Swapchain {
    pub fn new(
        instance: &Instance,
        surface: &Surface,
        physical_device: &PhysicalDevice,
        device: DeviceGuard,
        swapchain_desc: SwapchainDesc,
    ) -> Result<Self> {
        let surface_format = {
            let formats = unsafe {
                surface.raw().get_physical_device_surface_formats(
                    physical_device.raw(),
                    surface.raw_vulkan(),
                )?
            };

            if formats.len() == 1 && formats[0].format == vk::Format::UNDEFINED {
                vk::SurfaceFormatKHR {
                    format: vk::Format::B8G8R8A8_UNORM,
                    color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
                }
            } else {
                *formats
                    .iter()
                    .find(|format| {
                        format.format == vk::Format::B8G8R8A8_UNORM
                            && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                    })
                    .unwrap_or(&formats[0])
            }
        };

        let present_mode = {
            let present_modes = unsafe {
                surface.raw().get_physical_device_surface_present_modes(
                    physical_device.raw(),
                    surface.raw_vulkan(),
                )?
            };

            if present_modes.contains(&swapchain_desc.present_mode) {
                swapchain_desc.present_mode
            } else {
                return Err(anyhow::anyhow!("Present mode not supported"));
            }
        };

        // Get surface capabilities.
        let capabilities = unsafe {
            surface.raw().get_physical_device_surface_capabilities(
                physical_device.raw(),
                surface.raw_vulkan(),
            )?
        };

        let extent = {
            if capabilities.current_extent.width != std::u32::MAX {
                capabilities.current_extent
            } else {
                let min = capabilities.min_image_extent;
                let max = capabilities.max_image_extent;
                // Clamp requested extent.
                let width = swapchain_desc.width.min(max.width).max(min.width);
                let height = swapchain_desc.height.min(max.height).max(min.height);

                vk::Extent2D { width, height }
            }
        };

        let image_count = capabilities
            .max_image_count
            .min(capabilities.min_image_count + 1);

        log::info!("Swapchain image count: {}", image_count);
        log::info!("Swapchain extent: {} X {}", extent.width, extent.height);

        let queue_family_indices = [
            swapchain_desc.graphics_queue_family_index,
            swapchain_desc.present_queue_family_index,
        ];

        let create_info = {
            let mut info = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface.raw_vulkan())
                .min_image_count(image_count)
                .image_format(surface_format.format)
                .image_color_space(surface_format.color_space)
                .image_extent(extent)
                .image_array_layers(1)
                .image_usage(
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::TRANSFER_DST
                        | vk::ImageUsageFlags::TRANSFER_SRC,
                )
                .pre_transform(capabilities.current_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                // XXX: Query supported present modes and make sure requested present mode is supported
                // .present_mode(present_mode);
                .present_mode(vk::PresentModeKHR::FIFO);

            if swapchain_desc.graphics_queue_family_index
                == swapchain_desc.present_queue_family_index
            {
                info = info.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
            } else {
                info = info
                    .image_sharing_mode(vk::SharingMode::CONCURRENT)
                    .queue_family_indices(&queue_family_indices);
            }

            info
        };

        let ash_swapchain = khr::Swapchain::new(instance.raw(), device.raw());
        let vulkan_swapchain = unsafe { ash_swapchain.create_swapchain(&create_info, None)? };

        let mut swapchain = Self {
            device,
            ash_swapchain,
            vulkan_swapchain,

            graphics_queue_family_index: swapchain_desc.graphics_queue_family_index,
            present_queue_family_index: swapchain_desc.present_queue_family_index,

            format: surface_format.format,
            color_space: surface_format.color_space,
            present_mode,
            extent,

            image_count,
            vulkan_image_index: 0,

            images: Vec::with_capacity(image_count as _),
            image_views: Vec::with_capacity(image_count as _),
            image_handles: Vec::with_capacity(image_count as _),
        };

        swapchain
            .init_images()
            .with_context(|| (format!("Failed to initialize swapchain images!")))?;

        Ok(swapchain)
    }

    pub fn acquire_next_image(&mut self, signal_semaphore: &Semaphore) -> Result<bool> {
        let (image_index, is_suboptimal) = unsafe {
            self.ash_swapchain.acquire_next_image(
                self.vulkan_swapchain,
                u64::MAX,
                signal_semaphore.raw(),
                vk::Fence::null(),
            )?
        };

        self.vulkan_image_index = image_index;

        Ok(!is_suboptimal)
    }

    pub fn queue_present(&self, wait_semaphores: &[&Semaphore], queue: &Queue) -> Result<bool> {
        let swapchains = [self.vulkan_swapchain];
        let image_indices = [self.vulkan_image_index];
        let wait_semaphores = wait_semaphores
            .iter()
            .map(|semaphore| semaphore.raw_clone())
            .collect::<Vec<_>>();

        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        let result = unsafe {
            self.ash_swapchain
                .queue_present(queue.raw(), &present_info)?
        };

        Ok(result)
    }

    pub fn set_present_mode(&mut self, present_mode: vk::PresentModeKHR) {
        self.present_mode = present_mode;
    }

    pub fn vulkan_image_index(&self) -> u32 {
        self.vulkan_image_index
    }

    pub fn extent(&self) -> vk::Extent2D {
        self.extent
    }

    pub fn format(&self) -> vk::Format {
        self.format
    }

    pub fn color_space(&self) -> vk::ColorSpaceKHR {
        self.color_space
    }

    pub fn current_vulkan_image(&self) -> vk::Image {
        self.images[self.vulkan_image_index as usize]
    }

    pub fn current_image_view(&self) -> vk::ImageView {
        self.image_views[self.vulkan_image_index as usize]
    }

    pub fn current_image(&self) -> &Image {
        &self.image_handles[self.vulkan_image_index as usize]
    }

    pub fn device(&self) -> &DeviceGuard {
        &self.device
    }

    fn recreate_from_desc(
        &mut self,
        instance: &Instance,
        surface: &Surface,
        physical_device: &PhysicalDevice,
        device: DeviceGuard,
        swapchain_desc: SwapchainDesc,
    ) -> Result<Self> {
        self.destroy();
        Self::new(instance, surface, physical_device, device, swapchain_desc)
    }

    pub fn recreate_present_mode(
        &mut self,
        instance: &Instance,
        surface: &Surface,
        physical_device: &PhysicalDevice,
        device: DeviceGuard,
        present_mode: vk::PresentModeKHR,
    ) -> Result<Self> {
        let desc = SwapchainDesc::new(
            self.extent.width,
            self.extent.height,
            self.graphics_queue_family_index,
            self.present_queue_family_index,
        )
        .set_present_mode(present_mode);
        self.recreate_from_desc(instance, surface, physical_device, device, desc)
    }

    /// Recreates swapchain with extent information queried from the underlying window surface.
    pub fn recreate(
        &mut self,
        instance: &Instance,
        surface: &Surface,
        physical_device: &PhysicalDevice,
        device: DeviceGuard,
    ) -> Result<Self> {
        let desc = SwapchainDesc::new(
            u32::MAX,
            u32::MIN,
            self.graphics_queue_family_index,
            self.present_queue_family_index,
        )
        .set_present_mode(self.present_mode);
        self.recreate_from_desc(instance, surface, physical_device, device, desc)
    }

    pub fn destroy(&mut self) {
        if !self.image_views.is_empty() {
            unsafe {
                for image_view in self.image_views.drain(..) {
                    self.device.raw().destroy_image_view(image_view, None);
                }

                self.ash_swapchain
                    .destroy_swapchain(self.vulkan_swapchain, None);
            }
        }
    }

    fn init_images(&mut self) -> Result<()> {
        let images = unsafe {
            self.ash_swapchain
                .get_swapchain_images(self.vulkan_swapchain)?
        };

        assert_eq!(self.image_count, images.len() as u32);

        let mut image_views = Vec::with_capacity(images.len());
        let mut image_handles = Vec::with_capacity(images.len());

        for image in &images {
            let image_view_info = vk::ImageViewCreateInfo::builder()
                .image(image.clone())
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(self.format)
                .components(
                    vk::ComponentMapping::builder()
                        .r(vk::ComponentSwizzle::IDENTITY)
                        .g(vk::ComponentSwizzle::IDENTITY)
                        .b(vk::ComponentSwizzle::IDENTITY)
                        .a(vk::ComponentSwizzle::IDENTITY)
                        .build(),
                )
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(1)
                        .build(),
                );

            let image_view = unsafe {
                self.device
                    .raw()
                    .create_image_view(&image_view_info, None)?
            };

            image_views.push(image_view);
            image_handles.push(Image::from_swapchain(&self, image.clone(), image_view));
        }

        self.images = images;
        self.image_views = image_views;
        self.image_handles = image_handles;

        Ok(())
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        self.destroy();
    }
}
