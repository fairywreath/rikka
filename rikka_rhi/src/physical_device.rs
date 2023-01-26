use std::ffi::CStr;

use anyhow::Result;
use ash::vk::{self, QUEUE_FAMILY_EXTERNAL};

use crate::{queue::QueueFamily, surface::Surface};

#[derive(Debug, Clone)]
pub struct PhysicalDevice {
    pub physical_device: vk::PhysicalDevice,

    pub name: String,
    pub device_type: vk::PhysicalDeviceType,
    pub limits: vk::PhysicalDeviceLimits,
    pub properties: vk::PhysicalDeviceProperties,
    pub queue_families: Vec<QueueFamily>,
    pub supported_extensions: Vec<String>,
    pub supported_surface_formats: Vec<vk::SurfaceFormatKHR>,
    pub supported_present_modes: Vec<vk::PresentModeKHR>,
}

impl PhysicalDevice {
    pub fn new_from_vulkan_handle(
        instance: &ash::Instance,
        surface: &Surface,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Self> {
        let properties = unsafe { instance.get_physical_device_properties(physical_device) };
        let name = unsafe {
            CStr::from_ptr(properties.device_name.as_ptr())
                .to_str()
                .unwrap()
                .to_owned()
        };
        let device_type = properties.device_type;
        let limits = properties.limits;

        let queue_family_properties =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        let queue_families = queue_family_properties
            .into_iter()
            .enumerate()
            .map(|(index, prop)| {
                let present_support = unsafe {
                    surface.raw().get_physical_device_surface_support(
                        physical_device,
                        index as _,
                        surface.vulkan(),
                    )?
                };
                Ok(QueueFamily::new(index as _, prop, present_support))
            })
            .collect::<Result<_>>()?;

        let extension_properties =
            unsafe { instance.enumerate_device_extension_properties(physical_device)? };
        let supported_extensions = extension_properties
            .into_iter()
            .map(|prop| {
                let name = unsafe { CStr::from_ptr(prop.extension_name.as_ptr()) };
                name.to_str().unwrap().to_owned()
            })
            .collect();

        let supported_surface_formats = unsafe {
            surface
                .raw()
                .get_physical_device_surface_formats(physical_device, surface.vulkan())?
        };

        let supported_present_modes = unsafe {
            surface
                .raw()
                .get_physical_device_surface_present_modes(physical_device, surface.vulkan())?
        };

        Ok(Self {
            physical_device,
            name,
            device_type,
            limits,
            properties,
            queue_families,
            supported_extensions,
            supported_surface_formats,
            supported_present_modes,
        })
    }

    pub fn supports_extensions(&self, extensions: &[&str]) -> bool {
        let supported_extensions = self
            .supported_extensions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();

        extensions
            .iter()
            .all(|ext| supported_extensions.contains(ext))
    }

    pub fn raw(&self) -> &vk::PhysicalDevice {
        &self.physical_device
    }

    pub fn raw_clone(&self) -> vk::PhysicalDevice {
        self.physical_device.clone()
    }
}
