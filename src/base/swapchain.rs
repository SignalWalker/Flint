use crate::base::surface::*;
use crate::shader::PushConstant;
use ash::extensions::khr::Swapchain;

use crate::command::*;
use crate::renderpass::RenderPassToken;
use crate::shader::DescPoolToken;
use crate::shader::PipeToken;
use crate::shader::ShaderArtifact;
use crate::shader::ShaderStage;
use crate::*;
use ash::version::DeviceV1_0;
use ash::{vk, Device, Instance};
use std::collections::HashMap;
use std::default::Default;

use std::ops::Drop;

fn query_support(
    physical_device: vk::PhysicalDevice,
    surface: &SurfToken,
) -> (
    vk::SurfaceCapabilitiesKHR,
    Vec<vk::SurfaceFormatKHR>,
    Vec<vk::PresentModeKHR>,
) {
    unsafe {
        let capabilities = surface
            .loader
            .get_physical_device_surface_capabilities(physical_device, surface.surface)
            .unwrap();
        let formats = surface
            .loader
            .get_physical_device_surface_formats(physical_device, surface.surface)
            .unwrap();
        let present_modes = surface
            .loader
            .get_physical_device_surface_present_modes(physical_device, surface.surface)
            .unwrap();

        (capabilities, formats, present_modes)
    }
}

fn choose_format(available: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
    available
        .iter()
        .map(|sfmt| match sfmt.format {
            vk::Format::UNDEFINED => vk::SurfaceFormatKHR {
                format: vk::Format::B8G8R8_UNORM,
                color_space: sfmt.color_space,
            },
            _ => *sfmt,
        })
        .nth(0)
        .unwrap()
    // if available.len() == 1 && available[0].format == vk::Format::UNDEFINED {
    //     return vk::SurfaceFormatKHR {
    //         format: vk::Format::B8G8R8A8_UNORM,
    //         color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    //     };
    // }
    //
    // for format in available {
    //     if format.format == vk::Format::B8G8R8A8_UNORM
    //         && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
    //     {
    //         return format.clone();
    //     }
    // }
    //
    // available_formats.first().unwrap().clone()
}

fn choose_mode(available: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    let mut res = vk::PresentModeKHR::FIFO;

    for mode in available {
        if *mode == vk::PresentModeKHR::MAILBOX {
            return *mode;
        } else if *mode == vk::PresentModeKHR::IMMEDIATE {
            res = *mode;
        }
    }

    res
}

fn choose_extent(
    capabilities: &vk::SurfaceCapabilitiesKHR,
    window: &winit::Window,
) -> vk::Extent2D {
    if capabilities.current_extent.width != u32::max_value() {
        capabilities.current_extent
    } else {
        fn clamp(num: u32, n: u32, x: u32) -> u32 {
            use std::cmp::{max, min};
            max(n, min(x, num))
        }
        let window_size = window
            .get_inner_size()
            .expect("Failed to get the size of Window");
        // println!(
        //     "\t\tInner Window Size: ({}, {})",
        //     window_size.width, window_size.height
        // );

        vk::Extent2D {
            width: clamp(
                window_size.width as u32,
                capabilities.min_image_extent.width,
                capabilities.max_image_extent.width,
            ),
            height: clamp(
                window_size.height as u32,
                capabilities.min_image_extent.height,
                capabilities.max_image_extent.height,
            ),
        }
    }
}

pub struct SwapchainBase {
    pub loader: Swapchain,
    pub chain: vk::SwapchainKHR,
    pub format: vk::Format,
    pub extent: vk::Extent2D,
    pub imgs: Vec<vk::Image>,
    pub depth_img: vk::Image,
    pub depth_img_fmt: vk::Format,
    pub depth_img_mem: vk::DeviceMemory,
}

impl SwapchainBase {
    fn make_views(&self, dev: &Device) -> (Vec<vk::ImageView>, vk::ImageView) {
        (
            self.imgs
                .iter()
                .map(|&img| {
                    let create_view_info = vk::ImageViewCreateInfo::builder()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(self.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::R,
                            g: vk::ComponentSwizzle::G,
                            b: vk::ComponentSwizzle::B,
                            a: vk::ComponentSwizzle::A,
                        })
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .image(img);
                    unsafe { dev.create_image_view(&create_view_info, None).unwrap() }
                })
                .collect(),
            {
                let info = vk::ImageViewCreateInfo::builder()
                    .subresource_range(
                        vk::ImageSubresourceRange::builder()
                            .aspect_mask(vk::ImageAspectFlags::DEPTH)
                            .level_count(1)
                            .layer_count(1)
                            .build(),
                    )
                    .image(self.depth_img)
                    .format(self.depth_img_fmt)
                    .view_type(vk::ImageViewType::TYPE_2D);
                unsafe { dev.create_image_view(&info, None).unwrap() }
            },
        )
    }

    unsafe fn new(
        instance: &Instance,
        device: &Device,
        surface: &SurfToken,
        pdevice: vk::PhysicalDevice,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
        window: &winit::Window,
    ) -> Self {
        let (capabilities, formats, present_modes) = query_support(pdevice, surface);
        let format = choose_format(&formats);
        let mode = choose_mode(&present_modes);
        let extent = choose_extent(&capabilities, window);

        let mut desired_image_count = capabilities.min_image_count + 1;
        if capabilities.max_image_count > 0 && desired_image_count > capabilities.max_image_count {
            desired_image_count = capabilities.max_image_count;
        }
        let pre_transform = if capabilities
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            capabilities.current_transform
        };

        let swapchain_loader = Swapchain::new(instance, device);
        let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface.surface)
            .min_image_count(desired_image_count)
            .image_color_space(format.color_space)
            .image_format(format.format)
            .image_extent(extent)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(pre_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(mode)
            .clipped(true)
            .image_array_layers(1);
        let swapchain = swapchain_loader
            .create_swapchain(&swapchain_create_info, None)
            .unwrap();

        //println!("Making depth img");
        let (depth_img, depth_img_fmt, depth_img_mem) = {
            let fmt = vk::Format::D16_UNORM;
            let create_info = vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(fmt)
                .extent(vk::Extent3D {
                    width: extent.width,
                    height: extent.height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            let depth_img = device.create_image(&create_info, None).unwrap();

            let depth_image_memory_req = device.get_image_memory_requirements(depth_img);
            let depth_image_memory_index = find_memorytype_index(
                &depth_image_memory_req,
                mem_prop,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
            .expect("Unable to find suitable memory index for depth image.");

            let depth_image_allocate_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(depth_image_memory_req.size)
                .memory_type_index(depth_image_memory_index);

            let depth_img_mem = device
                .allocate_memory(&depth_image_allocate_info, None)
                .unwrap();

            device
                .bind_image_memory(depth_img, depth_img_mem, 0)
                .unwrap();

            (depth_img, fmt, depth_img_mem)
        };
        //println!("Done making depth img");

        Self {
            chain: swapchain,
            format: format.format,
            extent,
            imgs: swapchain_loader.get_swapchain_images(swapchain).unwrap(),
            loader: swapchain_loader,
            depth_img,
            depth_img_fmt,
            depth_img_mem,
        }
    }

    unsafe fn destroy(&mut self, dev: &Device) {
        //println!("Destroying SwapBase");
        self.loader.destroy_swapchain(self.chain, None);
        dev.free_memory(self.depth_img_mem, None);
        dev.destroy_image(self.depth_img, None);
    }
}

pub struct SwapToken {
    dev: Device,
    // pub surface_format: vk::SurfaceFormatKHR,
    // pub surface_resolution: vk::Extent2D,
    pub base: SwapchainBase,
    pub depth_view: vk::ImageView,
    pub img_views: Vec<vk::ImageView>,

    pub cmd_pool: CmdPool,

    pub renderpass: RenderPassToken,
    pub framebuffers: Vec<vk::Framebuffer>,

    pub viewport: vk::Viewport,
    pub scissors: vk::Rect2D,

    pub pipes: HashMap<String, PipeToken>,
}

impl Drop for SwapToken {
    fn drop(&mut self) {
        unsafe {
            eprintln!("Dropping Swaptoken");
            //self.dev.device_wait_idle().unwrap();
            self.clean();
            //println!("Done Dropping Swaptoken")
        }
    }
}

impl SwapToken {
    unsafe fn clean(&mut self) {
        //println!("Cleaning Swaptoken");
        self.dev.device_wait_idle().unwrap();
        for buffer in self.framebuffers.drain(0..) {
            self.dev.destroy_framebuffer(buffer, None)
        }
        self.dev.destroy_image_view(self.depth_view, None);
        for img in self.img_views.drain(0..) {
            self.dev.destroy_image_view(img, None);
        }
        self.base.destroy(&self.dev);
    }

    pub fn recreate(
        &mut self,
        pdev: vk::PhysicalDevice,
        instance: &Instance,
        surface: &SurfToken,
        window: &winit::Window,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
        present_queue: vk::Queue,
    ) {
        eprintln!("Recreating Swapchain");
        unsafe {
            self.dev.device_wait_idle().unwrap();
            self.clean();

            self.base = SwapchainBase::new(&instance, &self.dev, &surface, pdev, mem_prop, window);
            let (depth_view, img_views, renderpass, framebuffers, viewport, scissors) =
                Self::create(
                    &self.dev,
                    &self.base,
                    self.cmd_pool.buffers[0],
                    present_queue,
                );
            self.depth_view = depth_view;
            self.img_views = img_views;
            self.renderpass = renderpass;
            self.framebuffers = framebuffers;
            self.viewport = viewport;
            self.scissors = scissors;
            for (id, pipe) in self.pipes.iter_mut() {
                eprintln!("Recreating Pipeline: {}", id);
                pipe.recreate(self.renderpass.renderpass, self.viewport, self.scissors)
            }
        }
    }

    pub fn renderpass_begin_info(
        &self,
        present_index: u32,
        clear_values: &[vk::ClearValue],
    ) -> vk::RenderPassBeginInfo {
        vk::RenderPassBeginInfo::builder()
            .render_pass(self.renderpass.renderpass)
            .framebuffer(self.framebuffers[present_index as usize])
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.base.extent,
            })
            .clear_values(clear_values)
            .build()
    }

    pub fn new(
        instance: &Instance,
        dev: Device,
        surface: &SurfToken,
        pdevice: vk::PhysicalDevice,
        queue_fam: u32,
        present_queue: vk::Queue,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
        window: &winit::Window,
    ) -> Self {
        let base =
            unsafe { SwapchainBase::new(instance, &dev, surface, pdevice, mem_prop, window) };

        let cmd_pool = CmdPool::new(dev.clone(), queue_fam, 2);
        let (depth_view, img_views, renderpass, framebuffers, viewport, scissors) =
            Self::create(&dev, &base, cmd_pool.buffers[0], present_queue);
        Self {
            dev,
            base,
            depth_view,
            img_views,
            cmd_pool,
            renderpass,
            framebuffers,
            viewport,
            scissors,
            pipes: HashMap::new(),
        }
    }

    fn create(
        dev: &Device,
        base: &SwapchainBase,
        setup_command_buffer: vk::CommandBuffer,
        present_queue: vk::Queue,
    ) -> (
        vk::ImageView,
        Vec<vk::ImageView>,
        RenderPassToken,
        Vec<vk::Framebuffer>,
        vk::Viewport,
        vk::Rect2D,
    ) {
        let (img_views, depth_view) = base.make_views(&dev);
        let renderpass = RenderPassToken::new(dev.clone(), base.format);
        let framebuffers: Vec<vk::Framebuffer> = img_views
            .iter()
            .map(|present| {
                let attachments = [*present, depth_view];
                let frame_buffer_create_info = vk::FramebufferCreateInfo::builder()
                    .render_pass(renderpass.renderpass)
                    .attachments(&attachments)
                    .width(base.extent.width)
                    .height(base.extent.height)
                    .layers(1);

                unsafe {
                    dev.create_framebuffer(&frame_buffer_create_info, None)
                        .unwrap()
                }
            })
            .collect();
        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: base.extent.width as _,
            height: base.extent.height as _,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let scissors = vk::Rect2D {
            extent: base.extent,
            ..Default::default()
        };

        unsafe {
            crate::command::record_submit_commandbuffer(
                dev,
                setup_command_buffer,
                present_queue,
                &[],
                &[],
                &[],
                |device, setup_command_buffer| {
                    let layout_transition_barriers = vk::ImageMemoryBarrier::builder()
                        .image(base.depth_img)
                        .dst_access_mask(
                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .subresource_range(
                            vk::ImageSubresourceRange::builder()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .layer_count(1)
                                .level_count(1)
                                .build(),
                        );

                    device.cmd_pipeline_barrier(
                        setup_command_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[layout_transition_barriers.build()],
                    );
                },
            );
        }
        (
            depth_view,
            img_views,
            renderpass,
            framebuffers,
            viewport,
            scissors,
        )
    }

    pub fn make_pipeline(
        &mut self,
        id: String,
        pool: DescPoolToken,
        shaders: HashMap<ShaderStage, ShaderArtifact>,
        push_consts: Vec<&PushConstant>,
    ) {
        self.pipes.insert(
            id,
            PipeToken::build(
                self.dev.clone(),
                pool,
                push_consts,
                self.renderpass.renderpass,
                self.viewport,
                self.scissors,
                shaders,
            ),
        );
    }
}
