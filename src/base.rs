use crate::*;
use ash::extensions::khr::Swapchain;
use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0};
use ash::{vk, Device, Entry, Instance};

use std::default::Default;
use std::ffi::CString;
use std::ops::Drop;

pub mod surface;
pub mod swapchain;
pub use surface::*;
pub use swapchain::*;

pub struct VkData {
    pub entry: Entry,
    pub instance: Instance,
    pub device: Device,

    pub debug_report_loader: DebugReport,
    pub debug_call_back: vk::DebugReportCallbackEXT,

    pub pdevice: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub queue_family_index: u32,
    pub present_queue: vk::Queue,

    pub present_complete_semaphore: vk::Semaphore,
    pub rendering_complete_semaphore: vk::Semaphore,
}

impl VkData {
    pub fn new(name: &str, window: &winit::Window) -> (Self, SurfToken, SwapToken) {
        unsafe {
            let entry = Entry::new().unwrap();
            let app_name = CString::new(name).unwrap();

            let layer_names = [CString::new("VK_LAYER_LUNARG_standard_validation").unwrap()];
            let layers_names_raw: Vec<*const i8> = layer_names
                .iter()
                .map(|raw_name| raw_name.as_ptr())
                .collect();

            let extension_names_raw = extension_names();

            let appinfo = vk::ApplicationInfo::builder()
                .application_name(&app_name)
                .application_version(0)
                .engine_name(&app_name)
                .engine_version(0)
                .api_version(ash::vk_make_version!(1, 1, 100));

            let create_info = vk::InstanceCreateInfo::builder()
                .application_info(&appinfo)
                .enabled_layer_names(&layers_names_raw)
                .enabled_extension_names(&extension_names_raw);

            let instance: Instance = entry
                .create_instance(&create_info, None)
                .expect("Instance creation error");

            let debug_info = vk::DebugReportCallbackCreateInfoEXT::builder()
                .flags(
                    vk::DebugReportFlagsEXT::ERROR
                        | vk::DebugReportFlagsEXT::WARNING
                        | vk::DebugReportFlagsEXT::PERFORMANCE_WARNING
                        //| vk::DebugReportFlagsEXT::INFORMATION,
                )
                .pfn_callback(Some(vulkan_debug_callback));

            let debug_report_loader = DebugReport::new(&entry, &instance);
            let debug_call_back = debug_report_loader
                .create_debug_report_callback(&debug_info, None)
                .unwrap();
            let pdevices = instance
                .enumerate_physical_devices()
                .expect("Physical device error");
            let surface = SurfToken::new(&entry, &instance, &window);
            let (pdevice, queue_family_index) = pdevices
                .iter()
                .map(|pdevice| {
                    instance
                        .get_physical_device_queue_family_properties(*pdevice)
                        .iter()
                        .enumerate()
                        .filter_map(|(index, ref info)| {
                            let supports_graphic_and_surface =
                                info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                    && match surface.loader.get_physical_device_surface_support(
                                        *pdevice,
                                        index as u32,
                                        surface.surface,
                                    ) {
                                        Ok(b) => b,
                                        Err(_) => false,
                                    };
                            if supports_graphic_and_surface {
                                Some((*pdevice, index))
                            } else {
                                None
                            }
                        })
                        .nth(0)
                })
                .filter_map(|v| v)
                .nth(0)
                .expect("Couldn't find suitable device.");
            let queue_family_index = queue_family_index as u32;
            let device_extension_names_raw = [Swapchain::name().as_ptr()];
            let features = vk::PhysicalDeviceFeatures {
                shader_clip_distance: 1,
                ..Default::default()
            };
            let priorities = [1.0];

            let device: Device = {
                let queue_info = [vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(queue_family_index)
                    .queue_priorities(&priorities)
                    .build()];

                let device_create_info = vk::DeviceCreateInfo::builder()
                    .queue_create_infos(&queue_info)
                    .enabled_extension_names(&device_extension_names_raw)
                    .enabled_features(&features);
                instance
                    .create_device(pdevice, &device_create_info, None)
                    .unwrap()
            };
            let present_queue = device.get_device_queue(queue_family_index as u32, 0);

            let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);
            let swapchain = SwapToken::new(
                &instance,
                device.clone(),
                &surface,
                pdevice,
                queue_family_index,
                present_queue,
                &device_memory_properties,
                window,
            );

            let semaphore_create_info = vk::SemaphoreCreateInfo::default();

            let present_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();
            let rendering_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();
            (
                VkData {
                    entry,
                    instance,
                    device,
                    queue_family_index,
                    pdevice,
                    device_memory_properties,
                    present_queue,
                    present_complete_semaphore,
                    rendering_complete_semaphore,
                    debug_call_back,
                    debug_report_loader,
                },
                surface,
                swapchain,
            )
        }
    }
}

impl Drop for VkData {
    fn drop(&mut self) {
        println!("Dropping VkData");
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.device
                .destroy_semaphore(self.present_complete_semaphore, None);
            self.device
                .destroy_semaphore(self.rendering_complete_semaphore, None);
            // self.device.free_memory(self.depth_image_memory, None);
            // self.device.destroy_image_view(self.depth_image_view, None);
            // self.device.destroy_image(self.depth_image, None);
            // for &image_view in self.present_image_views.iter() {
            //     self.device.destroy_image_view(image_view, None);
            // }
            // self.device.destroy_command_pool(self.pool, None);
            // self.swapchain_loader
            //     .destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None);
            //self.surface_loader.destroy_surface(self.surface, None);
            self.debug_report_loader
                .destroy_debug_report_callback(self.debug_call_back, None);
            self.instance.destroy_instance(None);
            //println!("Dropped VkData");
        }
    }
}
