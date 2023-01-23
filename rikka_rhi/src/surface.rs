use anyhow::Result;
use ash::{extensions::khr, vk};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use crate::instance::Instance;

pub(crate) struct Surface {
    ash_surface: khr::Surface,
    vulkan_surface: vk::SurfaceKHR,
}

impl Surface {
    pub fn new(
        entry: &ash::Entry,
        instance: &Instance,
        window_handle: &dyn HasRawWindowHandle,
        display_handle: &dyn HasRawDisplayHandle,
    ) -> Result<Self> {
        let ash_surface = khr::Surface::new(entry, &instance.instance());
        let vulkan_surface = unsafe {
            ash_window::create_surface(
                entry,
                &instance.instance(),
                display_handle.raw_display_handle(),
                window_handle.raw_window_handle(),
                None,
            )?
        };

        Ok(Self {
            ash_surface,
            vulkan_surface,
        })
    }

    pub fn ash(&self) -> &khr::Surface {
        &self.ash_surface
    }

    pub fn vulkan(&self) -> vk::SurfaceKHR {
        self.vulkan_surface
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            self.ash_surface.destroy_surface(self.vulkan_surface, None);
        }
    }
}
