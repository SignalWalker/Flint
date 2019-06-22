use crate::buffer::*;
use crate::*;
use ash::{version::DeviceV1_0, vk, Device};

pub struct Texture {
    dev: Device,
    pub mem: vk::DeviceMemory,
    pub image: vk::Image,
    pub view: vk::ImageView,
}

impl Drop for Texture {
    fn drop(&mut self) {
        eprintln!("Dropping Texture");
        unsafe {
            self.dev.free_memory(self.mem, None);
            self.dev.destroy_image_view(self.view, None);
            self.dev.destroy_image(self.image, None);
        }
    }
}

impl Texture {
    pub fn from_path(
        dev: Device,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
        cmd_buf: vk::CommandBuffer,
        present_queue: vk::Queue,
        path: &str,
    ) -> Self {
        let image = image::open(path).unwrap().to_rgba();
        let dims = image.dimensions();
        let data = image.into_raw();
        let img_buf = BufToken::with_data(
            dev.clone(),
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::SharingMode::EXCLUSIVE,
            &mem_prop,
            &data,
        );
        Self::from_buffer(dev, img_buf, mem_prop, cmd_buf, present_queue, dims)
    }

    pub fn from_buffer(
        dev: Device,
        image: BufToken,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
        cmd_buf: vk::CommandBuffer,
        present_queue: vk::Queue,
        dims: (u32, u32),
    ) -> Self {
        let format = vk::Format::R8G8B8A8_UNORM;

        let texture_image = unsafe {
            let texture_create_info = vk::ImageCreateInfo {
                image_type: vk::ImageType::TYPE_2D,
                format,
                extent: vk::Extent3D {
                    width: dims.0,
                    height: dims.1,
                    depth: 1,
                },
                mip_levels: 1,
                array_layers: 1,
                samples: vk::SampleCountFlags::TYPE_1,
                tiling: vk::ImageTiling::OPTIMAL,
                usage: vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
                sharing_mode: vk::SharingMode::EXCLUSIVE,
                ..Default::default()
            };
            dev.create_image(&texture_create_info, None).unwrap()
        };
        let texture_memory = unsafe {
            let texture_allocate_info = {
                let texture_memory_req = dev.get_image_memory_requirements(texture_image);
                let texture_memory_index = find_memorytype_index(
                    &texture_memory_req,
                    mem_prop,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                )
                .unwrap();
                vk::MemoryAllocateInfo {
                    allocation_size: texture_memory_req.size,
                    memory_type_index: texture_memory_index,
                    ..Default::default()
                }
            };
            dev.allocate_memory(&texture_allocate_info, None).unwrap()
        };
        unsafe {
            dev.bind_image_memory(texture_image, texture_memory, 0)
                .unwrap();
            crate::command::record_submit_commandbuffer(
                &dev,
                cmd_buf,
                present_queue,
                &[],
                &[],
                &[],
                |device, texture_command_buffer| {
                    let texture_barrier = vk::ImageMemoryBarrier {
                        dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                        new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        image: texture_image,
                        subresource_range: vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            level_count: 1,
                            layer_count: 1,
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    device.cmd_pipeline_barrier(
                        texture_command_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[texture_barrier],
                    );
                    let buffer_copy_regions = vk::BufferImageCopy::builder()
                        .image_subresource(
                            vk::ImageSubresourceLayers::builder()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .layer_count(1)
                                .build(),
                        )
                        .image_extent(vk::Extent3D {
                            width: dims.0,
                            height: dims.1,
                            depth: 1,
                        });

                    device.cmd_copy_buffer_to_image(
                        texture_command_buffer,
                        image.buf,
                        texture_image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[buffer_copy_regions.build()],
                    );
                    let texture_barrier_end = vk::ImageMemoryBarrier {
                        src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                        dst_access_mask: vk::AccessFlags::SHADER_READ,
                        old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        image: texture_image,
                        subresource_range: vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            level_count: 1,
                            layer_count: 1,
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    device.cmd_pipeline_barrier(
                        texture_command_buffer,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[texture_barrier_end],
                    );
                },
            )
        };
        Self {
            mem: texture_memory,
            image: texture_image,
            view: unsafe {
                let tex_image_view_info = vk::ImageViewCreateInfo {
                    view_type: vk::ImageViewType::TYPE_2D,
                    format,
                    components: vk::ComponentMapping {
                        r: vk::ComponentSwizzle::R,
                        g: vk::ComponentSwizzle::G,
                        b: vk::ComponentSwizzle::B,
                        a: vk::ComponentSwizzle::A,
                    },
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        level_count: 1,
                        layer_count: 1,
                        ..Default::default()
                    },
                    image: texture_image,
                    ..Default::default()
                };
                dev.create_image_view(&tex_image_view_info, None).unwrap()
            },
            dev,
        }
    }

    pub fn tex_info(&self, sampler: vk::Sampler) -> vk::DescriptorImageInfo {
        vk::DescriptorImageInfo {
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            image_view: self.view,
            sampler,
        }
    }
}
