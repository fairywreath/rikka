use std::ffi::{c_void, CStr, CString};

use anyhow::Result;
use raw_window_handle::HasRawDisplayHandle;
use rikka_core::{
    ash::{self, extensions::ext::DebugUtils},
    vk,
};

use crate::{physical_device::PhysicalDevice, surface::Surface};

pub struct Instance {
    instance: ash::Instance,
    debug_utils: DebugUtils,
    debug_utils_messenger: vk::DebugUtilsMessengerEXT,
    entry: ash::Entry,
}

impl Instance {
    pub fn new(display_handle: &dyn HasRawDisplayHandle) -> Result<Self> {
        let entry = unsafe { ash::Entry::load()? };

        // Create vulkan instance.
        let app_name = CString::new("Rikka").unwrap();
        let app_info = vk::ApplicationInfo::builder()
            .application_name(app_name.as_c_str())
            .api_version(vk::API_VERSION_1_3);

        let mut extension_names =
            ash_window::enumerate_required_extensions(display_handle.raw_display_handle())?
                .to_vec();
        extension_names.push(DebugUtils::name().as_ptr());

        let layer_strings = vec![CString::new("VK_LAYER_KHRONOS_validation").unwrap()];
        let layer_names: Vec<*const i8> =
            layer_strings.iter().map(|c_str| c_str.as_ptr()).collect();

        let validation_features = vec![
            // This feature is broken.
            // vk::ValidationFeatureEnableEXT::Gpu_ASSISTED,
            vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
            vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
        ];
        let mut validation_features =
            vk::ValidationFeaturesEXT::builder().enabled_validation_features(&validation_features);

        let instance_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names)
            .enabled_layer_names(&layer_names);
        // .push_next(&mut validation_features);

        let instance = unsafe { entry.create_instance(&instance_info, None)? };

        // Create debug utils messenger
        let debug_utils_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
            .flags(vk::DebugUtilsMessengerCreateFlagsEXT::empty())
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(vulkan_debug_utils_callback));

        let debug_utils = DebugUtils::new(&entry, &instance);
        let debug_utils_messenger =
            unsafe { debug_utils.create_debug_utils_messenger(&debug_utils_info, None)? };

        Ok(Self {
            entry,
            instance,
            debug_utils,
            debug_utils_messenger,
        })
    }

    pub fn raw(&self) -> &ash::Instance {
        &self.instance
    }

    pub fn entry(&self) -> &ash::Entry {
        &self.entry
    }

    pub fn get_physical_devices(&self, surface: &Surface) -> Result<Vec<PhysicalDevice>> {
        let physical_devices = unsafe { self.instance.enumerate_physical_devices()? };

        let physical_devices = physical_devices
            .into_iter()
            .map(|phys_device| {
                PhysicalDevice::new_from_vulkan_handle(&self.instance, &surface, phys_device)
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(physical_devices)
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        log::info!("Instance dropped");
        unsafe {
            self.debug_utils
                .destroy_debug_utils_messenger(self.debug_utils_messenger, None);
            self.instance.destroy_instance(None);
        }
    }
}

pub unsafe extern "system" fn vulkan_debug_utils_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    let severity = match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => "[Verbose]",
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => "[Warning]",
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => "[Error]",
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => "[Info]",
        _ => "[Unknown]",
    };
    let types = match message_type {
        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "[General]",
        vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "[Performance]",
        vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "[Validation]",
        _ => "[Unknown]",
    };
    let message = CStr::from_ptr((*p_callback_data).p_message);
    println!("[VK Debug]{}{}{:?}", severity, types, message);

    vk::FALSE
}
