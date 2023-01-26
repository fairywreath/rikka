use std::sync::Arc;

use anyhow::Result;
use ash::{extensions::khr, vk};

use crate::{
    device::Device, instance::Instance, physical_device::PhysicalDevice, queue::Queue,
    surface::Surface, swapchain, synchronization::Semaphore,
};

pub struct Swapchain {
    device: Arc<Device>,
    ash_swapchain: khr::Swapchain,
    vulkan_swapchain: vk::SwapchainKHR,

    format: vk::Format,
    extent: vk::Extent2D,
    color_space: vk::ColorSpaceKHR,
    present_mode: vk::PresentModeKHR,
    // images: Vec<vk::Image>,
    // image_views: Vec<vk::ImageView>,
    image_count: u32,
    // Image index obtained from AcquireNextImage.
    vulkan_image_index: u32,
}

pub struct SwapchainDesc {
    pub width: u32,
    pub height: u32,

    pub graphics_queue_family_index: u32,
    pub present_queue_family_index: u32,
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
        }
    }
}

impl Swapchain {
    pub fn new(
        instance: &Instance,
        surface: &Surface,
        physical_device: &PhysicalDevice,
        device: &Arc<Device>,
        swapchain_desc: SwapchainDesc,
    ) -> Result<Self> {
        let surface_format = {
            let formats = unsafe {
                surface.raw().get_physical_device_surface_formats(
                    physical_device.raw_clone(),
                    surface.vulkan(),
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
                    physical_device.raw_clone(),
                    surface.vulkan(),
                )?
            };

            if present_modes.contains(&vk::PresentModeKHR::FIFO) {
                vk::PresentModeKHR::FIFO
            } else {
                vk::PresentModeKHR::FIFO
            }
        };

        // Get surface capabilities.
        let capabilities = unsafe {
            surface.raw().get_physical_device_surface_capabilities(
                physical_device.raw_clone(),
                surface.vulkan(),
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
        log::debug!("Swapchain image count: {}", image_count);

        let queue_family_indices = [
            swapchain_desc.graphics_queue_family_index,
            swapchain_desc.present_queue_family_index,
        ];

        let create_info = {
            let mut info = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface.vulkan())
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
                .present_mode(present_mode);

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

        Ok(Self {
            device: device.clone(),
            ash_swapchain,
            vulkan_swapchain,
            format: surface_format.format,
            color_space: surface_format.color_space,
            present_mode,
            extent,

            image_count,
            vulkan_image_index: 0,
        })
    }

    pub fn acquire_next_image(&mut self, signal_semaphore: &Semaphore) -> Result<bool> {
        let (image_index, is_suboptimal) = unsafe {
            self.ash_swapchain.acquire_next_image(
                self.vulkan_swapchain,
                u64::MAX,
                signal_semaphore.raw_clone(),
                vk::Fence::null(),
            )?
        };

        self.vulkan_image_index = image_index;

        Ok(is_suboptimal)
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
                .queue_present(queue.raw_clone(), &present_info)?
        };

        Ok(result)
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

    // fn create_swapchain(&mut self) -> Result<()> {
    //     let surface_format = {
    //         let formats = unsafe {
    //             self.ash_surface.get_physical_device_surface_formats(
    //                 self.physical_device,
    //                 self.vulkan_surface,
    //             )?
    //         };

    //         if formats.len() == 1 && formats[0].format == vk::Format::UNDEFINED {
    //             vk::SurfaceFormatKHR {
    //                 format: vk::Format::B8G8R8A8_UNORM,
    //                 color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    //             }
    //         } else {
    //             *formats
    //                 .iter()
    //                 .find(|format| {
    //                     format.format == vk::Format::B8G8R8A8_UNORM
    //                         && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
    //                 })
    //                 .unwrap_or(&formats[0])
    //         }
    //     };

    //     let present_mode = {
    //         let present_modes = unsafe {
    //             self.ash_surface.get_physical_device_surface_present_modes(
    //                 self.physical_device,
    //                 self.vulkan_surface,
    //             )?
    //         };

    //         if present_modes.contains(&vk::PresentModeKHR::FIFO) {
    //             vk::PresentModeKHR::FIFO
    //         } else {
    //             vk::PresentModeKHR::FIFO
    //         }
    //     };

    //     // Get surface capabilities.
    //     let capabilities = unsafe {
    //         self.ash_surface.get_physical_device_surface_capabilities(
    //             self.physical_device,
    //             self.vulkan_surface,
    //         )?
    //     };

    //     let extent = {
    //         if capabilities.current_extent.width != std::u32::MAX {
    //             capabilities.current_extent
    //         } else {
    //             let min = capabilities.min_image_extent;
    //             let max = capabilities.max_image_extent;

    //             // Clamp requested extent.
    //             let width = self.swapchain_desc.width.min(max.width).max(min.width);
    //             let height = self.swapchain_desc.height.min(max.height).max(min.height);

    //             vk::Extent2D { width, height }
    //         }
    //     };

    //     let image_count = capabilities
    //         .max_image_count
    //         .min(capabilities.min_image_count + 1);
    //     log::debug!("Swapchain image count: {}", image_count);

    //     let queue_family_indices = [
    //         self.swapchain_desc.graphics_queue_family_index,
    //         self.swapchain_desc.present_queue_family_index,
    //     ];

    //     let create_info = {
    //         let mut info = vk::SwapchainCreateInfoKHR::builder()
    //             .surface(self.vulkan_surface)
    //             .min_image_count(image_count)
    //             .image_format(surface_format.format)
    //             .image_color_space(surface_format.color_space)
    //             .image_extent(extent)
    //             .image_array_layers(1)
    //             .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
    //             .pre_transform(capabilities.current_transform)
    //             .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
    //             .present_mode(present_mode);

    //         if self.swapchain_desc.graphics_queue_family_index
    //             == self.swapchain_desc.present_queue_family_index
    //         {
    //             info.image_sharing_mode(vk::SharingMode::EXCLUSIVE);
    //         } else {
    //             info.image_sharing_mode(vk::SharingMode::CONCURRENT);
    //             info.queue_family_indices(&queue_family_indices);
    //         }

    //         info
    //     };

    //     // Create swapchain.
    //     self.ash_swapchain = khr::Swapchain::new(&self.instance, self.device.raw());
    //     self.vulkan_swapchain = unsafe { self.ash_swapchain.create_swapchain(&create_info, None)? };

    //     Ok(())
    // }

    fn destroy(&mut self) {
        unsafe {
            self.ash_swapchain
                .destroy_swapchain(self.vulkan_swapchain, None);
        }
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        self.destroy();
    }
}
