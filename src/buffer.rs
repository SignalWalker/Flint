use crate::find_memorytype_index;
use ash::{util::Align, version::DeviceV1_0, vk, vk::DeviceMemory, Device};

use std::fmt::Debug;

mod buf_struct;
pub use buf_struct::*;

#[derive(Debug)]
pub enum Buffer {
    Raw(BufToken),
    Struct(BufStruct),
}

impl Buffer {
    pub fn buf_info(&self) -> vk::DescriptorBufferInfo {
        use Buffer::*;
        match self {
            Raw(t) => t.buf_info(),
            Struct(s) => s.buf.buf_info(),
        }
    }
}

pub struct BufToken {
    pub dev: Device,
    pub size: vk::DeviceSize,
    pub buf: vk::Buffer,
    mem: DeviceMemory,
}

impl Debug for BufToken {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("BufToken")
            .field("size", &self.size)
            .field("buf", &self.buf)
            .finish()
    }
}

impl Drop for BufToken {
    fn drop(&mut self) {
        eprintln!("Dropping buffer: {} B", self.size);
        unsafe {
            self.dev.destroy_buffer(self.buf, None);
            self.dev.free_memory(self.mem, None);
        }
    }
}

impl BufToken {
    pub fn buf_info(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo {
            buffer: self.buf,
            offset: 0,
            range: self.size,
        }
    }

    pub fn map_write<F, D>(&self, w: F)
    where
        F: Fn(Align<D>),
    {
        unsafe {
            w(Align::new(
                self.dev
                    .map_memory(self.mem, 0, self.size, vk::MemoryMapFlags::empty())
                    .unwrap(),
                std::mem::align_of::<D>() as u64,
                self.size,
            ));
            self.dev.unmap_memory(self.mem)
        }
    }

    pub fn write<D: Copy>(&self, data: &[D]) {
        self.map_write(|mut align| align.copy_from_slice(data))
    }

    pub fn with_size(
        dev: Device,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
        size: vk::DeviceSize,
    ) -> Self {
        let buf = unsafe {
            dev.create_buffer(
                &vk::BufferCreateInfo {
                    size: size as u64,
                    usage,
                    sharing_mode,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        let req = unsafe { dev.get_buffer_memory_requirements(buf) };
        let mem_type_index =
            find_memorytype_index(&req, &mem_prop, vk::MemoryPropertyFlags::HOST_VISIBLE).unwrap();
        let mem = unsafe {
            dev.allocate_memory(
                &vk::MemoryAllocateInfo {
                    allocation_size: req.size,
                    memory_type_index: mem_type_index,
                    ..Default::default()
                },
                None,
            )
        }
        .unwrap();
        unsafe { dev.bind_buffer_memory(buf, mem, 0) }.unwrap();
        BufToken {
            dev,
            size: req.size,
            buf,
            mem,
        }
    }

    pub fn with_len<D>(
        dev: Device,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
        len: usize,
    ) -> Self {
        Self::with_size(
            dev,
            usage,
            sharing_mode,
            mem_prop,
            (std::mem::size_of::<D>() * len) as _,
        )
    }

    pub fn with_data<D: Copy>(
        dev: Device,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
        data: &[D],
    ) -> Self {
        let res = Self::with_len::<D>(dev, usage, sharing_mode, mem_prop, data.len());
        res.write(data);
        res
    }
}
