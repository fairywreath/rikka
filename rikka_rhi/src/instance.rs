use std::{
    ffi::{c_void, CStr, CString},
    fmt::Debug,
};

use anyhow::Result;
use ash::{extensions::ext::DebugUtils, vk};
use raw_window_handle::HasRawDisplayHandle;

use crate::{physical_device::PhysicalDevice, surface::Surface};

pub struct Instance {
    instance: ash::Instance,
    debug_utils: DebugUtils,
    debug_utils_messenger: vk::DebugUtilsMessengerEXT,

    physical_devices: Vec<PhysicalDevice>,
}

impl Instance {
    pub fn new(entry: &ash::Entry, display_handle: &dyn HasRawDisplayHandle) -> Result<Self> {
        // Create vulkan instance.
        let app_name = CString::new("Rikka RHIContext").unwrap();
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
            // vk::ValidationFeatureEnableEXT::GPU_ASSISTED,
            vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
            vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
        ];
        let mut validation_features =
            vk::ValidationFeaturesEXT::builder().enabled_validation_features(&validation_features);

        let instance_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names)
            .enabled_layer_names(&layer_names)
            .push_next(&mut validation_features);

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

        let debug_utils = DebugUtils::new(entry, &instance);
        let debug_utils_messenger =
            unsafe { debug_utils.create_debug_utils_messenger(&debug_utils_info, None)? };

        Ok(Self {
            instance,
            debug_utils,
            debug_utils_messenger,
            physical_devices: vec![],
        })
    }

    pub fn raw(&self) -> &ash::Instance {
        &self.instance
    }

    pub fn raw_clone(&self) -> ash::Instance {
        self.instance.clone()
    }

    pub fn get_physical_devices(&mut self, surface: &Surface) -> Result<&Vec<PhysicalDevice>> {
        if self.physical_devices.is_empty() {
            let physical_devices = unsafe { self.instance.enumerate_physical_devices()? };

            let physical_devices = physical_devices
                .into_iter()
                .map(|phys_device| {
                    PhysicalDevice::new_from_vulkan_handle(&self.instance, &surface, phys_device)
                })
                .collect::<Result<Vec<_>>>()?;

            self.physical_devices = physical_devices;
        }

        Ok(&self.physical_devices)
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
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
